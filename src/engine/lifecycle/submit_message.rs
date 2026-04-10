//! `QueryEngine::submit_message` -- the main conversation turn pipeline.
//!
//! Phase A: Input Processing
//! Phase B: System Prompt Build
//! Phase C: Pre-Query Setup (SystemInit, local-command fast path)
//! Phase D: Query Loop -- full message dispatch
//! Phase E: Result Generation (SdkResult)

use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use futures::Stream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::engine::input_processing;
use crate::engine::result;
use crate::engine::sdk_types::*;
use crate::engine::system_prompt;
use crate::query::loop_impl;
use crate::session::transcript;
use crate::tools::hooks;
use crate::types::config::{QueryParams, QuerySource};
use crate::types::message::{
    Attachment, Message, MessageContent, QueryYield, StreamEvent, SystemSubtype, Usage,
};

use super::deps::QueryEngineDeps;
use super::types::{AbortReason, UsageTracking};
use super::QueryEngine;

impl QueryEngine {
    /// Submit a user message and return a stream of `SdkMessage` items.
    ///
    /// This is the primary entry point for driving a conversation turn.
    /// The caller should consume the entire stream; every invocation ends
    /// with exactly one `SdkMessage::Result`.
    pub fn submit_message(
        &self,
        prompt: &str,
        query_source: QuerySource,
    ) -> Pin<Box<dyn Stream<Item = SdkMessage> + Send>> {
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

        let state_ref = self.state.clone();
        let aborted_ref = self.aborted.clone();
        let pending_bg_results = self.pending_bg_results.clone();

        let stream = async_stream::stream! {
            let started_at = Instant::now();
            let mut current_message_usage = Usage::default();
            let mut last_stop_reason: Option<String> = None;
            let mut structured_output: Option<serde_json::Value> = None;
            let mut turn_count_this_submit: usize = 0;
            let mut collected_errors: Vec<String> = Vec::new();

            // ================================================================
            // PHASE A-pre: Fire UserPromptSubmit hook
            // ================================================================
            {
                let hooks_map = state_ref.read().app_state.hooks.clone();
                let configs = hooks::load_hook_configs(&hooks_map, "UserPromptSubmit");
                if !configs.is_empty() {
                    let payload = serde_json::json!({
                        "prompt": &prompt,
                    });
                    match hooks::run_event_hooks("UserPromptSubmit", &payload, &configs).await {
                        Ok(output) => {
                            if !output.should_continue {
                                info!("UserPromptSubmit hook blocked prompt");
                                let reason = output.reason
                                    .or(output.stop_reason)
                                    .unwrap_or_else(|| "Blocked by UserPromptSubmit hook".to_string());
                                yield SdkMessage::Result(SdkResult {
                                    subtype: ResultSubtype::Success,
                                    is_error: false,
                                    duration_ms: started_at.elapsed().as_millis() as u64,
                                    duration_api_ms: 0,
                                    num_turns: 0,
                                    result: reason,
                                    stop_reason: None,
                                    session_id: session_id.to_string(),
                                    total_cost_usd: 0.0,
                                    usage: UsageTracking::default(),
                                    permission_denials: vec![],
                                    structured_output: None,
                                    uuid: Uuid::new_v4(),
                                    errors: vec![],
                                });
                                return;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "UserPromptSubmit hook error, continuing");
                        }
                    }
                }
            }

            // ================================================================
            // PHASE A: Input Processing
            // ================================================================

            // A.1: Clear turn-scoped state
            state_ref.write().discovered_skill_names.clear();

            // A.2: Process user input (delegate to input_processing module)
            let current_msgs_snapshot = state_ref.read().messages.clone();
            let processed = input_processing::process_user_input(
                &prompt,
                &current_msgs_snapshot,
                &config.cwd,
            );

            // A.3: Push processed messages into mutable_messages
            {
                let mut s = state_ref.write();
                for m in &processed.messages {
                    s.messages.push(m.clone());
                }
            }

            // A.4: Persist user message to transcript (fire-and-forget)
            if !processed.messages.is_empty() {
                let _ = transcript::record_transcript(
                    session_id.as_str(),
                    &processed.messages,
                );
            }

            // ================================================================
            // PHASE B: System Prompt Build
            // ================================================================

            let (tools_snapshot, model_name) = {
                let s = state_ref.read();
                let tools = s.tools.clone();
                let model = config
                    .user_specified_model
                    .clone()
                    .unwrap_or_else(|| s.app_state.main_loop_model.clone());
                (tools, model)
            };

            let (system_prompt_parts, user_context, system_context) =
                system_prompt::build_system_prompt(
                    config.custom_system_prompt.as_deref(),
                    config.append_system_prompt.as_deref(),
                    &tools_snapshot,
                    &model_name,
                    &config.cwd,
                );

            // Fire InstructionsLoaded hook if CLAUDE.md context was injected
            {
                let content_length: usize = system_prompt_parts.iter().map(|p| p.len()).sum();
                if content_length > 0 {
                    let hooks_map = state_ref.read().app_state.hooks.clone();
                    let configs = hooks::load_hook_configs(&hooks_map, "InstructionsLoaded");
                    if !configs.is_empty() {
                        let payload = serde_json::json!({
                            "source": "system_prompt",
                            "content_length": content_length,
                            "cwd": &config.cwd,
                        });
                        let _ = hooks::run_event_hooks("InstructionsLoaded", &payload, &configs).await;
                    }
                }
            }

            // ================================================================
            // PHASE C: Pre-Query Setup
            // ================================================================

            // C.1: Yield SystemInit message
            let perm_mode = state_ref
                .read()
                .app_state
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
                session_id: session_id.to_string(),
                uuid: Uuid::new_v4(),
            });

            // C.2: If this is a local command, yield result and return immediately.
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
                    session_id: session_id.to_string(),
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

            let current_messages = state_ref.read().messages.clone();

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

            // Create API client via full auth resolution chain
            let api_client: Option<Arc<crate::api::client::ApiClient>> =
                crate::api::client::ApiClient::from_auth().map(Arc::new);

            // Create deps for the inner query loop
            let permission_callback = state_ref.read().permission_callback.clone();
            let bg_agent_tx = state_ref.read().bg_agent_tx.clone();
            let deps = Arc::new(QueryEngineDeps {
                aborted: aborted_ref.clone(),
                state: state_ref.clone(),
                api_client,
                agent_context: config.agent_context.clone(),
                permission_callback,
                bg_agent_tx,
                pending_bg_results: pending_bg_results.clone(),
            });

            // Run the query loop
            let inner_stream = loop_impl::query(params, deps);

            use futures::StreamExt;
            let mut inner_stream = std::pin::pin!(inner_stream);

            let api_started_at = Instant::now();
            let replay_user_messages = query_source == QuerySource::Sdk;

            while let Some(item) = inner_stream.next().await {
                match item {
                    // --------------------------------------------------------
                    // D.1: Assistant message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Assistant(ref assistant_msg)) => {
                        if let Some(ref sr) = assistant_msg.stop_reason {
                            last_stop_reason = Some(sr.clone());
                        }

                        {
                            let mut s = state_ref.write();
                            s.messages.push(Message::Assistant(assistant_msg.clone()));
                            if let Some(ref msg_usage) = assistant_msg.usage {
                                s.usage.add_usage(msg_usage, assistant_msg.cost_usd);
                            }
                        }

                        yield SdkMessage::Assistant(SdkAssistantMessage {
                            message: assistant_msg.clone(),
                            session_id: session_id.to_string(),
                            parent_tool_use_id: None,
                        });

                        let _ = transcript::record_transcript(
                            session_id.as_str(),
                            &[Message::Assistant(assistant_msg.clone())],
                        );

                        if config.auto_save_session {
                            let all_msgs = state_ref.read().messages.clone();
                            let _ = crate::session::storage::save_session(
                                session_id.as_str(),
                                &all_msgs,
                                &config.cwd,
                            );
                        }
                    }

                    // --------------------------------------------------------
                    // D.2: User message (tool results, continuation messages)
                    // --------------------------------------------------------
                    QueryYield::Message(Message::User(ref user_msg)) => {
                        turn_count_this_submit += 1;

                        {
                            let mut s = state_ref.write();
                            s.total_turn_count += 1;
                            s.messages.push(Message::User(user_msg.clone()));
                        }

                        if replay_user_messages {
                            let (content_text, content_blocks) = match &user_msg.content {
                                MessageContent::Text(t) => (t.clone(), None),
                                MessageContent::Blocks(blocks) => (
                                    format!("[{} content blocks]", blocks.len()),
                                    Some(blocks.clone()),
                                ),
                            };
                            yield SdkMessage::UserReplay(SdkUserReplay {
                                content: content_text,
                                session_id: session_id.to_string(),
                                uuid: user_msg.uuid,
                                timestamp: user_msg.timestamp,
                                is_replay: true,
                                is_synthetic: user_msg.is_meta,
                                content_blocks,
                            });
                        }

                        let _ = transcript::record_transcript(
                            session_id.as_str(),
                            &[Message::User(user_msg.clone())],
                        );
                    }

                    // --------------------------------------------------------
                    // D.3: Progress message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Progress(ref progress_msg)) => {
                        state_ref.write().messages.push(Message::Progress(progress_msg.clone()));

                        let _ = transcript::record_transcript(
                            session_id.as_str(),
                            &[Message::Progress(progress_msg.clone())],
                        );
                    }

                    // --------------------------------------------------------
                    // D.4: System message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::System(ref system_msg)) => {
                        match &system_msg.subtype {
                            SystemSubtype::CompactBoundary {
                                compact_metadata,
                            } => {
                                state_ref.write().messages.push(Message::System(
                                    system_msg.clone(),
                                ));

                                yield SdkMessage::CompactBoundary(
                                    SdkCompactBoundary {
                                        session_id: session_id.to_string(),
                                        uuid: system_msg.uuid,
                                        compact_metadata: compact_metadata
                                            .clone(),
                                    },
                                );
                            }

                            SystemSubtype::ApiError {
                                retry_attempt,
                                max_retries,
                                retry_in_ms,
                                error,
                            } => {
                                state_ref.write().messages.push(Message::System(
                                    system_msg.clone(),
                                ));

                                collected_errors.push(error.message.clone());

                                yield SdkMessage::ApiRetry(SdkApiRetry {
                                    attempt: *retry_attempt,
                                    max_retries: *max_retries,
                                    retry_delay_ms: *retry_in_ms,
                                    error_status: error.status,
                                    error: error.message.clone(),
                                    session_id: session_id.to_string(),
                                    uuid: system_msg.uuid,
                                });
                            }

                            _ => {
                                state_ref.write().messages.push(Message::System(system_msg.clone()));
                            }
                        }
                    }

                    // --------------------------------------------------------
                    // D.5: Attachment message
                    // --------------------------------------------------------
                    QueryYield::Message(Message::Attachment(ref attachment_msg)) => {
                        state_ref.write().messages.push(Message::Attachment(
                            attachment_msg.clone(),
                        ));

                        match &attachment_msg.attachment {
                            Attachment::MaxTurnsReached {
                                max_turns,
                                turn_count,
                            } => {
                                let (usage_snap, denials_snap) = {
                                    let s = state_ref.read();
                                    (s.usage.clone(), s.permission_denials.clone())
                                };

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
                                    session_id: session_id.to_string(),
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

                            Attachment::StructuredOutput { data } => {
                                structured_output = Some(data.clone());
                            }

                            Attachment::QueuedCommand {
                                prompt: cmd_prompt,
                                source_uuid,
                            } => {
                                let _ = source_uuid;
                                if replay_user_messages {
                                    yield SdkMessage::UserReplay(
                                        SdkUserReplay {
                                            content: cmd_prompt.clone(),
                                            session_id: session_id.to_string(),
                                            uuid: attachment_msg.uuid,
                                            timestamp: attachment_msg.timestamp,
                                            is_replay: false,
                                            is_synthetic: true,
                                            content_blocks: None,
                                        },
                                    );
                                }
                            }

                            Attachment::SkillDiscovery { skills } => {
                                let mut s = state_ref.write();
                                for skill in skills {
                                    s.discovered_skill_names.insert(skill.clone());
                                }
                            }

                            Attachment::NestedMemory { path, .. } => {
                                state_ref.write()
                                    .loaded_nested_memory_paths
                                    .insert(path.clone());
                            }

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
                                current_message_usage = msg_usage.clone();
                            }
                            StreamEvent::MessageDelta {
                                delta,
                                usage: delta_usage,
                            } => {
                                if let Some(du) = delta_usage {
                                    current_message_usage.output_tokens +=
                                        du.output_tokens;
                                }
                                if let Some(ref sr) = delta.stop_reason {
                                    last_stop_reason = Some(sr.clone());
                                }
                            }
                            StreamEvent::MessageStop => {}
                            _ => {}
                        }

                        yield SdkMessage::StreamEvent(SdkStreamEvent {
                            event: event.clone(),
                            session_id: session_id.to_string(),
                            uuid: Uuid::new_v4(),
                        });
                    }

                    // --------------------------------------------------------
                    // D.7: RequestStart
                    // --------------------------------------------------------
                    QueryYield::RequestStart(_) => {
                        debug!("request_start signal received");
                    }

                    // --------------------------------------------------------
                    // D.8: Tombstone
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
                            session_id: session_id.to_string(),
                            uuid: summary_msg.uuid,
                        });
                    }
                }

                // ============================================================
                // After EACH item: budget checks
                // ============================================================
                if let Some(max_budget) = config.max_budget_usd {
                    let current_cost = state_ref.read().usage.total_cost_usd;
                    if current_cost >= max_budget {
                        info!(
                            spent = current_cost,
                            limit = max_budget,
                            "max budget exceeded"
                        );

                        state_ref.write().abort_reason =
                            Some(AbortReason::MaxBudget {
                                spent_usd: current_cost,
                                limit_usd: max_budget,
                            });

                        let (usage_snap, denials_snap) = {
                            let s = state_ref.read();
                            (s.usage.clone(), s.permission_denials.clone())
                        };

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
                            session_id: session_id.to_string(),
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

            let final_messages = state_ref.read().messages.clone();

            let terminal_msg =
                result::find_terminal_message(&final_messages);
            let is_success = result::is_result_successful(
                terminal_msg,
                last_stop_reason.as_deref(),
            );
            let (text_result, is_api_error) =
                result::extract_text_result(&final_messages);

            let (usage_snap, denials_snap) = {
                let s = state_ref.read();
                (s.usage.clone(), s.permission_denials.clone())
            };

            let subtype = if is_success {
                ResultSubtype::Success
            } else {
                ResultSubtype::ErrorDuringExecution
            };

            let mut errors = collected_errors;
            if is_api_error {
                errors.push(text_result.clone());
            }

            // Record API duration in global ProcessState
            let api_duration_ms = api_started_at.elapsed().as_millis() as u64;
            crate::bootstrap::PROCESS_STATE
                .read()
                .api_duration.record(api_duration_ms);

            yield SdkMessage::Result(SdkResult {
                subtype,
                is_error: !is_success,
                duration_ms: started_at.elapsed().as_millis() as u64,
                duration_api_ms: api_started_at.elapsed().as_millis() as u64,
                num_turns: turn_count_this_submit,
                result: text_result,
                stop_reason: last_stop_reason,
                session_id: session_id.to_string(),
                total_cost_usd: usage_snap.total_cost_usd,
                usage: usage_snap,
                permission_denials: denials_snap,
                structured_output,
                uuid: Uuid::new_v4(),
                errors,
            });
        };
        Box::pin(stream)
    }
}
