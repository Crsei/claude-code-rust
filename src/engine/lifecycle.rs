//! QueryEngine -- full session lifecycle implementation.
//!
//! Corresponds to TypeScript: QueryEngine.ts
//!
//! Owns a single conversation session. Implements the complete message dispatch
//! pipeline as described in QUERY_ENGINE_SESSION_LIFECYCLE.md:
//!
//!   Phase A: Input Processing
//!   Phase B: System Prompt Build
//!   Phase C: Pre-Query Setup (SystemInit, local-command fast path)
//!   Phase D: Query Loop -- full message dispatch (assistant, user, progress,
//!            system, attachment, stream, request_start, tombstone, tool_use_summary)
//!   Phase E: Result Generation (SdkResult)
//!
//! The stream returned by `submit_message` yields `SdkMessage` items.

#![allow(unused)]

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::Result;
use futures::Stream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::engine::input_processing;
use crate::engine::result;
use crate::engine::sdk_types::*;
use crate::engine::system_prompt;
use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::query::loop_impl;
use crate::session::transcript;
use crate::types::app_state::AppState;
use crate::types::config::{QueryEngineConfig, QueryParams, QuerySource};
use crate::types::message::{
    Attachment, AttachmentMessage, Message, MessageContent, QueryYield, StreamEvent,
    SystemMessage, SystemSubtype, Usage, UserMessage,
};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Usage tracking -- accumulated across all API calls in a session.
#[derive(Debug, Clone, Default)]
pub struct UsageTracking {
    /// Total input tokens consumed.
    pub total_input_tokens: u64,
    /// Total output tokens produced.
    pub total_output_tokens: u64,
    /// Total cache-read tokens.
    pub total_cache_read_tokens: u64,
    /// Total cache-creation tokens.
    pub total_cache_creation_tokens: u64,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Number of API calls made.
    pub api_call_count: u64,
}

impl UsageTracking {
    /// Accumulate a single API call's usage.
    pub fn add_usage(&mut self, usage: &Usage, cost_usd: f64) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
        self.total_cache_read_tokens += usage.cache_read_input_tokens;
        self.total_cache_creation_tokens += usage.cache_creation_input_tokens;
        self.total_cost_usd += cost_usd;
        self.api_call_count += 1;
    }
}

/// A record of a permission denial.
#[derive(Debug, Clone)]
pub struct PermissionDenial {
    pub tool_name: String,
    pub tool_use_id: String,
    pub reason: String,
    pub timestamp: i64,
}

/// Reason for aborting a query.
#[derive(Debug, Clone)]
pub enum AbortReason {
    /// User pressed Ctrl-C or called abort().
    UserAbort,
    /// Max budget exceeded.
    MaxBudget { spent_usd: f64, limit_usd: f64 },
    /// Max turns exceeded.
    MaxTurns { turns: usize, limit: usize },
    /// Unrecoverable API error.
    ApiError { message: String },
}

// ---------------------------------------------------------------------------
// QueryEngine
// ---------------------------------------------------------------------------

/// QueryEngine -- owns the full lifecycle of a single conversation session.
///
/// Each session creates exactly one `QueryEngine`. It wraps the inner
/// `query::loop_impl::query()` generator, intercepting every yielded item to
/// maintain cross-turn state and produce `SdkMessage` items for the caller.
pub struct QueryEngine {
    /// Session identifier (UUID v4).
    pub session_id: String,
    /// Immutable configuration snapshot.
    config: QueryEngineConfig,

    // -- Cross-turn persistent state (mutable via Arc wrappers) --------------

    /// Conversation message history.
    mutable_messages: Arc<RwLock<Vec<Message>>>,
    /// Abort reason (if aborted).
    abort_reason: Arc<Mutex<Option<AbortReason>>>,
    /// Atomic abort flag (fast path for the query loop).
    aborted: Arc<AtomicBool>,
    /// Accumulated usage across all API calls.
    usage: Arc<Mutex<UsageTracking>>,
    /// History of permission denials.
    permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    /// Total turn count across all `submit_message` invocations.
    total_turn_count: Arc<Mutex<usize>>,
    /// Application-wide state (shared with deps).
    app_state: Arc<RwLock<AppState>>,
    /// Current tool registry (shared with deps).
    tools: Arc<RwLock<Tools>>,

    // -- Session-level dedup / tracking --------------------------------------

    /// Skills discovered during this session (dedup).
    discovered_skill_names: Arc<Mutex<HashSet<String>>>,
    /// Nested memory paths already loaded (dedup).
    loaded_nested_memory_paths: Arc<Mutex<HashSet<String>>>,
    /// Whether we have handled the orphaned-permission edge case.
    has_handled_orphaned_permission: Arc<AtomicBool>,
}

impl QueryEngine {
    // -- Construction --------------------------------------------------------

    /// Create a new QueryEngine with the given configuration.
    pub fn new(config: QueryEngineConfig) -> Self {
        let initial_messages = config.initial_messages.clone().unwrap_or_default();
        let tools = config.tools.clone();

        Self {
            session_id: Uuid::new_v4().to_string(),
            config,
            mutable_messages: Arc::new(RwLock::new(initial_messages)),
            abort_reason: Arc::new(Mutex::new(None)),
            aborted: Arc::new(AtomicBool::new(false)),
            usage: Arc::new(Mutex::new(UsageTracking::default())),
            permission_denials: Arc::new(Mutex::new(Vec::new())),
            total_turn_count: Arc::new(Mutex::new(0)),
            app_state: Arc::new(RwLock::new(AppState::default())),
            tools: Arc::new(RwLock::new(tools)),
            discovered_skill_names: Arc::new(Mutex::new(HashSet::new())),
            loaded_nested_memory_paths: Arc::new(Mutex::new(HashSet::new())),
            has_handled_orphaned_permission: Arc::new(AtomicBool::new(false)),
        }
    }

    // -- Main entry point ----------------------------------------------------

    /// Submit a user message and return a stream of `SdkMessage` items.
    ///
    /// This is the primary entry point for driving a conversation turn.
    /// The caller should consume the entire stream; every invocation ends
    /// with exactly one `SdkMessage::Result`.
    ///
    /// # Arguments
    /// * `prompt` -- the raw user input text
    /// * `query_source` -- where this query originated (REPL, SDK, Agent, etc.)
    pub fn submit_message(
        &self,
        prompt: &str,
        query_source: QuerySource,
    ) -> impl Stream<Item = SdkMessage> + '_ {
        info!(
            prompt_len = prompt.len(),
            source = ?query_source,
            session = %self.session_id,
            "submit_message: starting"
        );

        // Capture owned/cloned references for the async stream closure.
        let session_id = self.session_id.clone();
        let config = self.config.clone();
        let prompt = prompt.to_string();

        let messages_ref = self.mutable_messages.clone();
        let abort_reason_ref = self.abort_reason.clone();
        let aborted_ref = self.aborted.clone();
        let usage_ref = self.usage.clone();
        let permission_denials_ref = self.permission_denials.clone();
        let turn_count_ref = self.total_turn_count.clone();
        let app_state_ref = self.app_state.clone();
        let tools_ref = self.tools.clone();
        let discovered_skills_ref = self.discovered_skill_names.clone();
        let loaded_memory_ref = self.loaded_nested_memory_paths.clone();

        async_stream::stream! {
            let started_at = Instant::now();
            let mut current_message_usage = Usage::default();
            let mut last_stop_reason: Option<String> = None;
            let mut structured_output: Option<serde_json::Value> = None;
            let mut turn_count_this_submit: usize = 0;
            let mut collected_errors: Vec<String> = Vec::new();

            // ================================================================
            // PHASE A: Input Processing
            // ================================================================

            // A.1: Clear turn-scoped state
            discovered_skills_ref.lock().unwrap().clear();

            // A.2: Process user input (delegate to input_processing module)
            let current_msgs_snapshot =
                messages_ref.read().unwrap().clone();
            let processed = input_processing::process_user_input(
                &prompt,
                &current_msgs_snapshot,
                &config.cwd,
            );

            // A.3: Push processed messages into mutable_messages
            {
                let mut msgs = messages_ref.write().unwrap();
                for m in &processed.messages {
                    msgs.push(m.clone());
                }
            }

            // A.4: Persist user message to transcript (fire-and-forget)
            if !processed.messages.is_empty() {
                let _ = transcript::record_transcript(
                    &session_id,
                    &processed.messages,
                );
            }

            // ================================================================
            // PHASE B: System Prompt Build
            // ================================================================

            let tools_snapshot = tools_ref.read().unwrap().clone();
            let model_name = config
                .user_specified_model
                .clone()
                .unwrap_or_else(|| {
                    app_state_ref
                        .read()
                        .unwrap()
                        .main_loop_model
                        .clone()
                });

            let (system_prompt_parts, user_context, system_context) =
                system_prompt::build_system_prompt(
                    config.custom_system_prompt.as_deref(),
                    config.append_system_prompt.as_deref(),
                    &tools_snapshot,
                    &model_name,
                    &config.cwd,
                );

            // ================================================================
            // PHASE C: Pre-Query Setup
            // ================================================================

            // C.1: Yield SystemInit message
            let perm_mode = app_state_ref
                .read()
                .unwrap()
                .tool_permission_context
                .mode
                .clone();

            yield SdkMessage::SystemInit(SystemInitMessage {
                tools: tools_snapshot
                    .iter()
                    .map(|t| t.name().to_string())
                    .collect(),
                model: model_name.clone(),
                permission_mode: format!("{:?}", perm_mode),
                session_id: session_id.clone(),
                uuid: Uuid::new_v4(),
            });

            // C.2: If this is a local command (should_query == false), yield
            //      local result then SdkResult(success) and return immediately.
            if !processed.should_query {
                let local_text = processed
                    .result_text
                    .clone()
                    .unwrap_or_default();

                yield SdkMessage::Result(SdkResult {
                    subtype: ResultSubtype::Success,
                    is_error: false,
                    duration_ms: started_at.elapsed().as_millis() as u64,
                    duration_api_ms: 0,
                    num_turns: 0,
                    result: local_text,
                    stop_reason: None,
                    session_id: session_id.clone(),
                    total_cost_usd: 0.0,
                    usage: UsageTracking::default(),
                    permission_denials: vec![],
                    structured_output: None,
                    uuid: Uuid::new_v4(),
                    errors: vec![],
                });
                return;
            }

            // ================================================================
            // PHASE D: Query Loop -- full message dispatch
            // ================================================================

            // Build QueryParams from accumulated state
            let current_messages = messages_ref.read().unwrap().clone();

            let params = QueryParams {
                messages: current_messages,
                system_prompt: system_prompt_parts,
                user_context,
                system_context,
                fallback_model: config.fallback_model.clone(),
                query_source: query_source.clone(),
                max_output_tokens_override: None,
                max_turns: config.max_turns,
                skip_cache_write: None,
                task_budget: config.task_budget.clone(),
            };

            // Create API client from environment if available
            let api_client: Option<Arc<crate::api::client::ApiClient>> = {
                if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                    let base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
                    let client_config = crate::api::client::ApiClientConfig {
                        provider: crate::api::client::ApiProvider::Anthropic {
                            api_key,
                            base_url,
                        },
                        default_model: model_name.clone(),
                        max_retries: 3,
                        timeout_secs: 120,
                    };
                    Some(Arc::new(crate::api::client::ApiClient::new(client_config)))
                } else {
                    None
                }
            };

            // Create deps for the inner query loop
            let deps = Arc::new(QueryEngineDeps {
                aborted: aborted_ref.clone(),
                app_state: app_state_ref.clone(),
                tools: tools_ref.clone(),
                api_client,
            });

            // Run the query loop
            let inner_stream = loop_impl::query(params, deps);

            use futures::StreamExt;
            let mut inner_stream = std::pin::pin!(inner_stream);

            let api_started_at = Instant::now();
            let include_partial_messages = query_source == QuerySource::Sdk;
            let replay_user_messages = query_source == QuerySource::Sdk;

            while let Some(item) = inner_stream.next().await {
                match item {
                    // --------------------------------------------------------
                    // D.1: Assistant message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Assistant(ref assistant_msg)) => {
                        // Capture stop_reason
                        if let Some(ref sr) = assistant_msg.stop_reason {
                            last_stop_reason = Some(sr.clone());
                        }

                        // Push to mutable_messages
                        {
                            let mut msgs = messages_ref.write().unwrap();
                            msgs.push(Message::Assistant(assistant_msg.clone()));
                        }

                        // Update usage from this message
                        if let Some(ref msg_usage) = assistant_msg.usage {
                            usage_ref
                                .lock()
                                .unwrap()
                                .add_usage(msg_usage, assistant_msg.cost_usd);
                        }

                        // Yield SdkMessage::Assistant
                        yield SdkMessage::Assistant(SdkAssistantMessage {
                            message: assistant_msg.clone(),
                            session_id: session_id.clone(),
                            parent_tool_use_id: None,
                        });

                        // Fire-and-forget transcript
                        let _ = transcript::record_transcript(
                            &session_id,
                            &[Message::Assistant(assistant_msg.clone())],
                        );
                    }

                    // --------------------------------------------------------
                    // D.2: User message (tool results, continuation messages)
                    // --------------------------------------------------------
                    QueryYield::Message(Message::User(ref user_msg)) => {
                        turn_count_this_submit += 1;
                        *turn_count_ref.lock().unwrap() += 1;

                        // Push to mutable_messages
                        {
                            let mut msgs = messages_ref.write().unwrap();
                            msgs.push(Message::User(user_msg.clone()));
                        }

                        // Yield UserReplay if caller wants replays
                        if replay_user_messages {
                            let content_text = match &user_msg.content {
                                MessageContent::Text(t) => t.clone(),
                                MessageContent::Blocks(blocks) => {
                                    format!("[{} content blocks]", blocks.len())
                                }
                            };
                            yield SdkMessage::UserReplay(SdkUserReplay {
                                content: content_text,
                                session_id: session_id.clone(),
                                uuid: user_msg.uuid,
                                timestamp: user_msg.timestamp,
                                is_replay: true,
                                is_synthetic: user_msg.is_meta,
                            });
                        }

                        // Await transcript (blocking for user messages to
                        // preserve ordering guarantees)
                        let _ = transcript::record_transcript(
                            &session_id,
                            &[Message::User(user_msg.clone())],
                        );
                    }

                    // --------------------------------------------------------
                    // D.3: Progress message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Progress(ref progress_msg)) => {
                        // Push to mutable_messages
                        {
                            let mut msgs = messages_ref.write().unwrap();
                            msgs.push(Message::Progress(progress_msg.clone()));
                        }

                        // Fire-and-forget transcript
                        let _ = transcript::record_transcript(
                            &session_id,
                            &[Message::Progress(progress_msg.clone())],
                        );
                    }

                    // --------------------------------------------------------
                    // D.4: System message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::System(ref system_msg)) => {
                        match &system_msg.subtype {
                            // Compact boundary: GC old messages, yield marker
                            SystemSubtype::CompactBoundary {
                                compact_metadata,
                            } => {
                                // In a full implementation we would drain
                                // mutable_messages before the boundary to GC.
                                // For now, record and yield the boundary.
                                {
                                    let mut msgs = messages_ref.write().unwrap();
                                    msgs.push(Message::System(
                                        system_msg.clone(),
                                    ));
                                }

                                yield SdkMessage::CompactBoundary(
                                    SdkCompactBoundary {
                                        session_id: session_id.clone(),
                                        uuid: system_msg.uuid,
                                        compact_metadata: compact_metadata
                                            .clone(),
                                    },
                                );
                            }

                            // API error with retry: yield ApiRetry
                            SystemSubtype::ApiError {
                                retry_attempt,
                                max_retries,
                                retry_in_ms,
                                error,
                            } => {
                                {
                                    let mut msgs = messages_ref.write().unwrap();
                                    msgs.push(Message::System(
                                        system_msg.clone(),
                                    ));
                                }

                                collected_errors.push(error.message.clone());

                                yield SdkMessage::ApiRetry(SdkApiRetry {
                                    attempt: *retry_attempt,
                                    max_retries: *max_retries,
                                    retry_delay_ms: *retry_in_ms,
                                    error_status: error.status,
                                    error: error.message.clone(),
                                    session_id: session_id.clone(),
                                    uuid: system_msg.uuid,
                                });
                            }

                            // Other system messages: push silently (headless)
                            _ => {
                                let mut msgs = messages_ref.write().unwrap();
                                msgs.push(Message::System(system_msg.clone()));
                                // No yield -- system info/warning is silent
                            }
                        }
                    }

                    // --------------------------------------------------------
                    // D.5: Attachment message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Attachment(ref attachment_msg)) => {
                        // Push to mutable_messages
                        {
                            let mut msgs = messages_ref.write().unwrap();
                            msgs.push(Message::Attachment(
                                attachment_msg.clone(),
                            ));
                        }

                        match &attachment_msg.attachment {
                            // Max turns reached: yield error result and return
                            Attachment::MaxTurnsReached {
                                max_turns,
                                turn_count,
                            } => {
                                let usage_snap =
                                    usage_ref.lock().unwrap().clone();
                                let denials_snap =
                                    permission_denials_ref.lock().unwrap().clone();

                                yield SdkMessage::Result(SdkResult {
                                    subtype: ResultSubtype::ErrorMaxTurns,
                                    is_error: true,
                                    duration_ms: started_at
                                        .elapsed()
                                        .as_millis()
                                        as u64,
                                    duration_api_ms: api_started_at
                                        .elapsed()
                                        .as_millis()
                                        as u64,
                                    num_turns: *turn_count,
                                    result: format!(
                                        "Reached maximum of {} turns",
                                        max_turns
                                    ),
                                    stop_reason: last_stop_reason.clone(),
                                    session_id: session_id.clone(),
                                    total_cost_usd: usage_snap.total_cost_usd,
                                    usage: usage_snap,
                                    permission_denials: denials_snap,
                                    structured_output: structured_output
                                        .clone(),
                                    uuid: Uuid::new_v4(),
                                    errors: collected_errors.clone(),
                                });
                                return;
                            }

                            // Structured output: capture for final result
                            Attachment::StructuredOutput { data } => {
                                structured_output = Some(data.clone());
                            }

                            // Queued command: yield UserReplay so SDK sees it
                            Attachment::QueuedCommand {
                                prompt: cmd_prompt,
                                source_uuid,
                            } => {
                                if replay_user_messages {
                                    yield SdkMessage::UserReplay(
                                        SdkUserReplay {
                                            content: cmd_prompt.clone(),
                                            session_id: session_id.clone(),
                                            uuid: attachment_msg.uuid,
                                            timestamp: attachment_msg.timestamp,
                                            is_replay: false,
                                            is_synthetic: true,
                                        },
                                    );
                                }
                            }

                            // Skill discovery: dedup tracking
                            Attachment::SkillDiscovery { skills } => {
                                let mut set =
                                    discovered_skills_ref.lock().unwrap();
                                for skill in skills {
                                    set.insert(skill.clone());
                                }
                            }

                            // Nested memory: dedup tracking
                            Attachment::NestedMemory { path, .. } => {
                                loaded_memory_ref
                                    .lock()
                                    .unwrap()
                                    .insert(path.clone());
                            }

                            // Other attachments: no special SDK handling
                            _ => {}
                        }
                    }

                    // --------------------------------------------------------
                    // D.6: Stream event (partial messages)
                    // --------------------------------------------------------
                    QueryYield::Stream(ref event) => {
                        match event {
                            StreamEvent::MessageStart {
                                usage: msg_usage,
                            } => {
                                // Reset per-message usage accumulator
                                current_message_usage = msg_usage.clone();
                            }
                            StreamEvent::MessageDelta {
                                delta,
                                usage: delta_usage,
                            } => {
                                // Accumulate delta usage
                                if let Some(du) = delta_usage {
                                    current_message_usage.output_tokens +=
                                        du.output_tokens;
                                }
                                // Capture stop_reason from delta
                                if let Some(ref sr) = delta.stop_reason {
                                    last_stop_reason = Some(sr.clone());
                                }
                            }
                            StreamEvent::MessageStop => {
                                // The main usage update happens when we
                                // receive the full AssistantMessage in D.1.
                            }
                            _ => {}
                        }

                        // Forward stream events if partial messages enabled
                        if include_partial_messages {
                            yield SdkMessage::StreamEvent(SdkStreamEvent {
                                event: event.clone(),
                                session_id: session_id.clone(),
                                uuid: Uuid::new_v4(),
                            });
                        }
                    }

                    // --------------------------------------------------------
                    // D.7: RequestStart (internal signal -- do not yield)
                    // --------------------------------------------------------
                    QueryYield::RequestStart(_) => {
                        debug!("request_start signal received");
                    }

                    // --------------------------------------------------------
                    // D.8: Tombstone (model fallback retry -- skip)
                    // --------------------------------------------------------
                    QueryYield::Tombstone(_) => {
                        debug!("tombstone received (model fallback retry)");
                    }

                    // --------------------------------------------------------
                    // D.9: ToolUseSummary
                    // --------------------------------------------------------
                    QueryYield::ToolUseSummary(ref summary_msg) => {
                        yield SdkMessage::ToolUseSummary(SdkToolUseSummary {
                            summary: summary_msg.summary.clone(),
                            preceding_tool_use_ids: summary_msg
                                .preceding_tool_use_ids
                                .clone(),
                            session_id: session_id.clone(),
                            uuid: summary_msg.uuid,
                        });
                    }
                }

                // ============================================================
                // After EACH item: budget checks
                // ============================================================
                if let Some(max_budget) = config.max_budget_usd {
                    let current_cost =
                        usage_ref.lock().unwrap().total_cost_usd;
                    if current_cost >= max_budget {
                        info!(
                            spent = current_cost,
                            limit = max_budget,
                            "max budget exceeded"
                        );

                        *abort_reason_ref.lock().unwrap() =
                            Some(AbortReason::MaxBudget {
                                spent_usd: current_cost,
                                limit_usd: max_budget,
                            });

                        let usage_snap =
                            usage_ref.lock().unwrap().clone();
                        let denials_snap =
                            permission_denials_ref.lock().unwrap().clone();

                        yield SdkMessage::Result(SdkResult {
                            subtype: ResultSubtype::ErrorMaxBudgetUsd,
                            is_error: true,
                            duration_ms: started_at
                                .elapsed()
                                .as_millis()
                                as u64,
                            duration_api_ms: api_started_at
                                .elapsed()
                                .as_millis()
                                as u64,
                            num_turns: turn_count_this_submit,
                            result: format!(
                                "Stopped: cost ${:.4} exceeded budget ${:.4}",
                                current_cost, max_budget
                            ),
                            stop_reason: last_stop_reason.clone(),
                            session_id: session_id.clone(),
                            total_cost_usd: current_cost,
                            usage: usage_snap,
                            permission_denials: denials_snap,
                            structured_output: structured_output.clone(),
                            uuid: Uuid::new_v4(),
                            errors: collected_errors.clone(),
                        });
                        return;
                    }
                }
            } // end while let Some(item)

            // ================================================================
            // PHASE E: Result Generation
            // ================================================================

            let final_messages = messages_ref.read().unwrap().clone();

            // Find the terminal message and check success
            let terminal_msg =
                result::find_terminal_message(&final_messages);
            let is_success = result::is_result_successful(
                terminal_msg,
                last_stop_reason.as_deref(),
            );
            let (text_result, is_api_error) =
                result::extract_text_result(&final_messages);

            let usage_snap = usage_ref.lock().unwrap().clone();
            let denials_snap =
                permission_denials_ref.lock().unwrap().clone();

            let subtype = if is_success {
                ResultSubtype::Success
            } else {
                ResultSubtype::ErrorDuringExecution
            };

            let mut errors = collected_errors;
            if is_api_error {
                errors.push(text_result.clone());
            }

            yield SdkMessage::Result(SdkResult {
                subtype,
                is_error: !is_success,
                duration_ms: started_at.elapsed().as_millis() as u64,
                duration_api_ms: api_started_at.elapsed().as_millis() as u64,
                num_turns: turn_count_this_submit,
                result: text_result,
                stop_reason: last_stop_reason,
                session_id: session_id.clone(),
                total_cost_usd: usage_snap.total_cost_usd,
                usage: usage_snap,
                permission_denials: denials_snap,
                structured_output,
                uuid: Uuid::new_v4(),
                errors,
            });
        }
    }

    // -- Abort control -------------------------------------------------------

    /// Abort the currently running query.
    pub fn abort(&self) {
        info!("aborting query engine");
        self.aborted.store(true, Ordering::SeqCst);
        *self.abort_reason.lock().unwrap() = Some(AbortReason::UserAbort);
    }

    /// Reset the abort flag before starting a new `submit_message` call.
    pub fn reset_abort(&self) {
        self.aborted.store(false, Ordering::SeqCst);
        *self.abort_reason.lock().unwrap() = None;
    }

    /// Check whether the engine has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    /// Get the abort reason (if any).
    pub fn abort_reason(&self) -> Option<AbortReason> {
        self.abort_reason.lock().unwrap().clone()
    }

    // -- Accessors -----------------------------------------------------------

    /// Get a snapshot of the current message history.
    pub fn messages(&self) -> Vec<Message> {
        self.mutable_messages.read().unwrap().clone()
    }

    /// Get a snapshot of usage tracking.
    pub fn usage(&self) -> UsageTracking {
        self.usage.lock().unwrap().clone()
    }

    /// Get a snapshot of permission denials.
    pub fn permission_denials(&self) -> Vec<PermissionDenial> {
        self.permission_denials.lock().unwrap().clone()
    }

    /// Record a permission denial.
    pub fn record_permission_denial(&self, denial: PermissionDenial) {
        self.permission_denials.lock().unwrap().push(denial);
    }

    /// Get the total turn count (across all submit_message calls).
    pub fn total_turn_count(&self) -> usize {
        *self.total_turn_count.lock().unwrap()
    }

    /// Get a snapshot of the application state.
    pub fn app_state(&self) -> AppState {
        self.app_state.read().unwrap().clone()
    }

    /// Update the application state with a closure.
    pub fn update_app_state<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        let mut state = self.app_state.write().unwrap();
        updater(&mut state);
    }

    /// Replace the tool registry.
    pub fn set_tools(&self, tools: Tools) {
        *self.tools.write().unwrap() = tools;
    }

    /// Get discovered skill names from the current turn.
    pub fn discovered_skill_names(&self) -> HashSet<String> {
        self.discovered_skill_names.lock().unwrap().clone()
    }

    /// Get loaded nested memory paths.
    pub fn loaded_nested_memory_paths(&self) -> HashSet<String> {
        self.loaded_nested_memory_paths.lock().unwrap().clone()
    }
}


// ---------------------------------------------------------------------------
// QueryEngineDeps -- QueryDeps implementation for QueryEngine
// ---------------------------------------------------------------------------

/// Dependency injection bridge: provides the query loop with access to the
/// engine's shared state (abort flag, app state, tools) and, optionally, a
/// real `ApiClient` for making Anthropic API calls.
struct QueryEngineDeps {
    aborted: Arc<AtomicBool>,
    app_state: Arc<RwLock<AppState>>,
    tools: Arc<RwLock<Tools>>,
    /// When `Some`, the deps will use this client for `call_model` /
    /// `call_model_streaming`. When `None`, those methods bail with a
    /// descriptive error.
    api_client: Option<Arc<crate::api::client::ApiClient>>,
}

#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
    async fn call_model(
        &self,
        params: ModelCallParams,
    ) -> Result<ModelResponse> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model: no API client configured -- \
                 set ANTHROPIC_API_KEY or provide a mock in tests"
            )
        })?;

        let request = build_messages_request(&params);
        let stream = client.messages_stream(request).await?;
        let mut stream = std::pin::pin!(stream);

        let mut accumulator =
            crate::api::streaming::StreamAccumulator::new();
        let mut stream_events = Vec::new();

        use futures::StreamExt;
        while let Some(event_result) = stream.next().await {
            let event = event_result?;
            accumulator.process_event(&event);
            stream_events.push(event);
        }

        let usage = accumulator.usage.clone();
        let assistant_message = accumulator.build();

        Ok(ModelResponse {
            assistant_message,
            stream_events,
            usage,
        })
    }

    async fn call_model_streaming(
        &self,
        params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model_streaming: no API client configured -- \
                 set ANTHROPIC_API_KEY or provide a mock in tests"
            )
        })?;

        let request = build_messages_request(&params);
        client.messages_stream(request).await
    }

    async fn microcompact(
        &self,
        messages: Vec<Message>,
    ) -> Result<Vec<Message>> {
        Ok(messages)
    }

    async fn autocompact(
        &self,
        _messages: Vec<Message>,
        _tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        Ok(None)
    }

    async fn reactive_compact(
        &self,
        _messages: Vec<Message>,
    ) -> Result<Option<CompactionResult>> {
        Ok(None)
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &crate::types::message::AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool_name)
            .ok_or_else(|| {
                anyhow::anyhow!("tool not found: {}", request.tool_name)
            })?;

        let ctx = crate::types::tool::ToolUseContext {
            options: crate::types::tool::ToolUseOptions {
                debug: false,
                main_loop_model: self
                    .app_state
                    .read()
                    .unwrap()
                    .main_loop_model
                    .clone(),
                verbose: self.app_state.read().unwrap().verbose,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: {
                let (tx, rx) = tokio::sync::watch::channel(false);
                if self.aborted.load(Ordering::Relaxed) {
                    let _ = tx.send(true);
                }
                rx
            },
            read_file_state: crate::types::tool::FileStateCache::default(),
            get_app_state: {
                let state = self.app_state.clone();
                Arc::new(move || state.read().unwrap().clone())
            },
            set_app_state: {
                let state = self.app_state.clone();
                Arc::new(
                    move |updater: Box<
                        dyn FnOnce(AppState) -> AppState,
                    >| {
                        let mut s = state.write().unwrap();
                        let old = s.clone();
                        *s = updater(old);
                    },
                )
            },
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        };

        match tool.call(request.input, &ctx, parent_message, None).await {
            Ok(result) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result,
                is_error: false,
            }),
            Err(e) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: crate::types::tool::ToolResult {
                    data: serde_json::json!(format!("Error: {}", e)),
                    new_messages: vec![],
                },
                is_error: true,
            }),
        }
    }

    fn get_app_state(&self) -> AppState {
        self.app_state.read().unwrap().clone()
    }

    fn uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        self.tools.read().unwrap().clone()
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        Ok(self.tools.read().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------
// Helper: convert ModelCallParams into a MessagesRequest
// ---------------------------------------------------------------------------

/// Build a `MessagesRequest` from the generic `ModelCallParams`.
///
/// This translates the engine's internal representation into the wire format
/// expected by `api::client::ApiClient`.
fn build_messages_request(
    params: &ModelCallParams,
) -> crate::api::client::MessagesRequest {
    use crate::types::message::{Message, MessageContent};

    // Convert Message list to API JSON format
    let api_messages: Vec<serde_json::Value> = params
        .messages
        .iter()
        .filter_map(|msg| match msg {
            Message::User(u) => {
                let content = match &u.content {
                    MessageContent::Text(t) => serde_json::json!(t),
                    MessageContent::Blocks(blocks) => {
                        serde_json::to_value(blocks).unwrap_or_default()
                    }
                };
                Some(serde_json::json!({
                    "role": "user",
                    "content": content,
                }))
            }
            Message::Assistant(a) => {
                let content =
                    serde_json::to_value(&a.content).unwrap_or_default();
                Some(serde_json::json!({
                    "role": "assistant",
                    "content": content,
                }))
            }
            // System, Progress, Attachment messages are not sent to the API
            _ => None,
        })
        .collect();

    // Convert system prompt parts into API format
    let system = if params.system_prompt.is_empty() {
        None
    } else {
        Some(
            params
                .system_prompt
                .iter()
                .map(|s| serde_json::json!({"type": "text", "text": s}))
                .collect(),
        )
    };

    // Convert tools to API JSON format.
    // Each tool is rendered as {"name": ..., "input_schema": ...} which is
    // the shape the Anthropic Messages API expects.
    let tools: Option<Vec<serde_json::Value>> = if params.tools.is_empty() {
        None
    } else {
        Some(
            params
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name(),
                        "description": "", // description is async; callers should pre-resolve
                        "input_schema": t.input_json_schema(),
                    })
                })
                .collect(),
        )
    };

    // Build thinking config
    let thinking = params.thinking_enabled.and_then(|enabled| {
        if enabled {
            Some(serde_json::json!({
                "type": "enabled",
                "budget_tokens": params.max_output_tokens.unwrap_or(16384)
            }))
        } else {
            None
        }
    });

    crate::api::client::MessagesRequest {
        model: params
            .model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        messages: api_messages,
        system,
        max_tokens: params.max_output_tokens.unwrap_or(16384),
        tools,
        stream: true,
        thinking,
        tool_choice: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{AssistantMessage, ContentBlock};
    use std::sync::atomic::Ordering;

    fn make_config() -> QueryEngineConfig {
        QueryEngineConfig {
            cwd: "/tmp".to_string(),
            tools: vec![],
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: None,
            fallback_model: None,
            max_turns: None,
            max_budget_usd: None,
            task_budget: None,
            verbose: false,
            initial_messages: None,
            commands: vec![],
            thinking_config: None,
            json_schema: None,
            replay_user_messages: false,
            include_partial_messages: false,
            persist_session: false,
        }
    }

    #[test]
    fn test_query_engine_creation() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.messages().len(), 0);
        assert_eq!(engine.total_turn_count(), 0);
        assert!(engine.usage().total_cost_usd == 0.0);
        assert!(!engine.session_id.is_empty());
    }

    #[test]
    fn test_query_engine_abort() {
        let engine = QueryEngine::new(make_config());
        assert!(!engine.is_aborted());
        assert!(engine.abort_reason().is_none());

        engine.abort();
        assert!(engine.is_aborted());
        assert!(matches!(
            engine.abort_reason(),
            Some(AbortReason::UserAbort)
        ));

        engine.reset_abort();
        assert!(!engine.is_aborted());
        assert!(engine.abort_reason().is_none());
    }

    #[test]
    fn test_query_engine_app_state() {
        let engine = QueryEngine::new(make_config());
        let state = engine.app_state();
        assert!(!state.verbose);

        engine.update_app_state(|s| {
            s.verbose = true;
        });

        let state = engine.app_state();
        assert!(state.verbose);
    }

    #[test]
    fn test_query_engine_permission_denial() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.permission_denials().len(), 0);

        engine.record_permission_denial(PermissionDenial {
            tool_name: "Bash".to_string(),
            tool_use_id: "tu_1".to_string(),
            reason: "user denied".to_string(),
            timestamp: 0,
        });

        assert_eq!(engine.permission_denials().len(), 1);
        assert_eq!(engine.permission_denials()[0].tool_name, "Bash");
    }

    #[test]
    fn test_usage_tracking() {
        let mut usage = UsageTracking::default();
        let api_usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 5,
        };
        usage.add_usage(&api_usage, 0.001);
        assert_eq!(usage.total_input_tokens, 100);
        assert_eq!(usage.total_output_tokens, 50);
        assert_eq!(usage.total_cache_read_tokens, 10);
        assert_eq!(usage.total_cache_creation_tokens, 5);
        assert!((usage.total_cost_usd - 0.001).abs() < f64::EPSILON);
        assert_eq!(usage.api_call_count, 1);

        // Second call accumulates
        usage.add_usage(&api_usage, 0.002);
        assert_eq!(usage.total_input_tokens, 200);
        assert_eq!(usage.api_call_count, 2);
    }

    #[test]
    fn test_discovered_skill_names() {
        let engine = QueryEngine::new(make_config());
        assert!(engine.discovered_skill_names().is_empty());

        engine
            .discovered_skill_names
            .lock()
            .unwrap()
            .insert("test_skill".to_string());
        assert_eq!(engine.discovered_skill_names().len(), 1);
    }

    #[test]
    fn test_loaded_nested_memory_paths() {
        let engine = QueryEngine::new(make_config());
        assert!(engine.loaded_nested_memory_paths().is_empty());
    }

    #[test]
    fn test_set_tools() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.tools.read().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_submit_local_command() {
        use futures::StreamExt;

        let engine = QueryEngine::new(make_config());
        let stream = engine.submit_message("/clear", QuerySource::Sdk);
        let mut stream = std::pin::pin!(stream);

        let mut items: Vec<SdkMessage> = Vec::new();
        while let Some(msg) = stream.next().await {
            items.push(msg);
        }

        // Should yield SystemInit + Result
        assert!(
            items.len() >= 2,
            "expected at least 2 items, got {}",
            items.len()
        );

        // First should be SystemInit
        assert!(
            matches!(items[0], SdkMessage::SystemInit(_)),
            "first item should be SystemInit"
        );

        // Last should be Result with success
        let last = items.last().unwrap();
        match last {
            SdkMessage::Result(ref result) => {
                assert_eq!(result.subtype, ResultSubtype::Success);
                assert!(!result.is_error);
                // The local command result text comes from input_processing,
                // which produces the command name (e.g. "/clear").
                assert!(result.result.contains("clear"));
            }
            other => panic!("expected SdkMessage::Result, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_submit_message_yields_system_init() {
        use futures::StreamExt;

        let engine = QueryEngine::new(make_config());
        let stream =
            engine.submit_message("hello", QuerySource::ReplMainThread);
        let mut stream = std::pin::pin!(stream);

        // The first item should always be SystemInit
        if let Some(msg) = stream.next().await {
            match msg {
                SdkMessage::SystemInit(init) => {
                    assert_eq!(init.session_id, engine.session_id);
                    assert!(!init.model.is_empty());
                }
                other => panic!("expected SystemInit, got {:?}", other),
            }
        } else {
            panic!("stream was empty");
        }
    }
}
