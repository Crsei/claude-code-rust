//! `QueryEngine::submit_message` -- the main conversation turn pipeline.
//!
//! Phase A: Input Processing
//! Phase B: System Prompt Build
//! Phase C: Pre-Query Setup (SystemInit, local-command fast path)
//! Phase D: Query Loop -- full message dispatch
//! Phase E: Result Generation (SdkResult)

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::Stream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::codex_exec;
use crate::command_runtime::{CommandContext, CommandResult};
use crate::input_processing;
use crate::result;
use crate::session::transcript;
use crate::system_prompt;
use crate::types::config::{QueryParams, QuerySource};
use crate::types::message::{
    AssistantMessage, Attachment, ContentBlock, Message, MessageContent, QueryYield, StreamEvent,
    SystemSubtype,
};
use cc_engine::query::loop_impl;
use cc_types::sdk::*;

use super::deps::QueryEngineDeps;
use super::types::{AbortReason, UsageTrackingExt};
use super::QueryEngine;

fn model_assisted_memory_recall_enabled() -> bool {
    std::env::var("CC_RUST_MODEL_ASSISTED_MEMORY_RECALL")
        .or_else(|_| std::env::var("CC_RUST_MODEL_MEMORY_RECALL"))
        .map(|value| is_truthy_model_assisted_memory_recall_value(&value))
        .unwrap_or(false)
}

fn is_truthy_model_assisted_memory_recall_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn model_assisted_memory_recall_timeout() -> Duration {
    let millis = std::env::var("CC_RUST_MODEL_ASSISTED_MEMORY_RECALL_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .unwrap_or(1500);
    Duration::from_millis(millis)
}

fn deterministic_memory_context(
    cwd: &str,
    include_auto_memory: bool,
    memory_query_text: &str,
    recent_tool_names: &[String],
    already_surfaced_memory_keys: &std::collections::HashSet<String>,
) -> (Option<String>, Vec<String>) {
    match cc_session::memdir::build_relevant_memory_context_with(
        std::path::Path::new(cwd),
        include_auto_memory,
        memory_query_text,
        recent_tool_names,
        already_surfaced_memory_keys,
        cc_session::memdir::MODEL_ASSISTED_RECALL_MAX_RESULTS,
    ) {
        Ok((context, surfaced)) => (Some(context), surfaced),
        Err(error) => {
            debug!(
                error = %error,
                "failed to build deterministic memory context; falling back to full memory context"
            );
            (None, Vec::new())
        }
    }
}

async fn build_model_assisted_memory_context(
    cwd: &str,
    include_auto_memory: bool,
    memory_query_text: &str,
    recent_tool_names: &[String],
    already_surfaced_memory_keys: &std::collections::HashSet<String>,
    backend_name: &str,
    model_name: &str,
) -> anyhow::Result<Option<(String, Vec<String>)>> {
    let candidates = cc_session::memdir::recall_relevant_memories(
        std::path::Path::new(cwd),
        include_auto_memory,
        memory_query_text,
        recent_tool_names,
        already_surfaced_memory_keys,
        cc_session::memdir::MODEL_ASSISTED_RECALL_CANDIDATE_LIMIT,
    )?;
    if candidates.is_empty() {
        return Ok(Some((String::new(), Vec::new())));
    }

    let api_client = match cc_api::api::client::ApiClient::from_backend(Some(backend_name)) {
        Some(client) => client,
        None => return Ok(None),
    };
    let prompt = cc_session::memdir::build_model_assisted_recall_prompt(
        memory_query_text,
        &candidates,
        cc_session::memdir::MODEL_ASSISTED_RECALL_MAX_RESULTS,
    );
    let request = cc_api::api::client::MessagesRequest {
        model: model_name.to_string(),
        messages: vec![serde_json::json!({
            "role": "user",
            "content": prompt,
        })],
        system: Some(vec![serde_json::json!({
            "type": "text",
            "text": "Rank memory candidates for relevance. Return only valid JSON.",
        })]),
        max_tokens: 512,
        tools: None,
        stream: true,
        metadata: None,
        service_tier: None,
        stop_sequences: None,
        temperature: None,
        top_p: None,
        top_k: None,
        context_management: None,
        thinking: None,
        tool_choice: None,
        advisor_model: None,
    };

    let response = tokio::time::timeout(
        model_assisted_memory_recall_timeout(),
        api_client.messages(request),
    )
    .await
    .map_err(|_| anyhow::anyhow!("model-assisted memory recall timed out"))??;
    let response_text = assistant_message_text(&response);
    let identities = cc_session::memdir::parse_model_assisted_recall_selection(
        &response_text,
        &candidates,
        cc_session::memdir::MODEL_ASSISTED_RECALL_MAX_RESULTS,
    );

    if identities.is_empty() {
        if response_text.contains("[]") {
            return Ok(Some((String::new(), Vec::new())));
        }
        anyhow::bail!("model-assisted memory recall returned no recognized memory identities");
    }

    let selected = cc_session::memdir::select_relevant_memories_by_identity(
        &candidates,
        &identities,
        cc_session::memdir::MODEL_ASSISTED_RECALL_MAX_RESULTS,
    );
    if selected.is_empty() {
        anyhow::bail!("model-assisted memory recall selected no usable candidates");
    }

    let surfaced = selected
        .iter()
        .map(|memory| memory.identity.clone())
        .collect::<Vec<_>>();
    Ok(Some((
        cc_session::memdir::format_relevant_memory_context(&selected),
        surfaced,
    )))
}

fn assistant_message_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct SubmitTurnState {
    started_at: Instant,
    last_stop_reason: Option<String>,
    structured_output: Option<serde_json::Value>,
    turn_count_this_submit: usize,
    collected_errors: Vec<String>,
}

impl SubmitTurnState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            last_stop_reason: None,
            structured_output: None,
            turn_count_this_submit: 0,
            collected_errors: Vec::new(),
        }
    }

    fn duration_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

struct LocalCommandOutcome {
    is_error: bool,
    session_id: crate::bootstrap::SessionId,
}

impl LocalCommandOutcome {
    fn new(session_id: crate::bootstrap::SessionId) -> Self {
        Self {
            is_error: false,
            session_id,
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "local command execution needs submit state, session state, and command adapters"
)]
async fn handle_parsed_command(
    processed: &mut input_processing::ProcessedInput,
    current_messages: &[Message],
    config: &crate::types::config::QueryEngineConfig,
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    active_session_id_ref: &Arc<parking_lot::RwLock<crate::bootstrap::SessionId>>,
    session_id: &crate::bootstrap::SessionId,
    command_dispatcher: &dyn cc_types::commands::CommandDispatcher,
    command_executor: &dyn crate::command_runtime::CommandExecutor,
) -> LocalCommandOutcome {
    let mut outcome = LocalCommandOutcome::new(session_id.clone());
    let Some(parsed_command) = processed.parsed_command.take() else {
        return outcome;
    };

    let command_name = command_dispatcher
        .command_name(parsed_command.index)
        .unwrap_or_else(|| format!("#{}", parsed_command.index));

    let mut ctx = CommandContext {
        messages: current_messages.to_vec(),
        cwd: std::path::PathBuf::from(&config.cwd),
        app_state: state_ref.read().app_state.clone(),
        session_id: session_id.clone(),
    };

    match command_executor
        .execute(parsed_command, command_name.clone(), &mut ctx)
        .await
    {
        Ok(CommandResult::Output(text)) => {
            apply_command_state(state_ref, ctx);
            processed.result_text = Some(text);
            processed.should_query = false;
            processed.messages.clear();
        }
        Ok(CommandResult::Query(messages)) => {
            apply_command_state(state_ref, ctx);
            processed.messages = messages;
            processed.should_query = true;
            processed.result_text = None;
        }
        Ok(CommandResult::SwitchSession {
            session_id,
            messages,
            notice,
        }) => {
            switch_command_session(
                state_ref,
                active_session_id_ref,
                session_id.clone(),
                messages,
                ctx,
            );
            outcome.session_id = session_id;
            processed.result_text = Some(notice);
            processed.should_query = false;
            processed.messages.clear();
        }
        Ok(CommandResult::Clear) => {
            outcome.session_id =
                clear_command_session(state_ref, active_session_id_ref, config, ctx);
            processed.result_text = Some("Conversation cleared.".to_string());
            processed.should_query = false;
            processed.messages.clear();
        }
        Ok(CommandResult::Exit(text)) => {
            apply_command_state(state_ref, ctx);
            processed.result_text = Some(text);
            processed.should_query = false;
            processed.messages.clear();
        }
        Ok(CommandResult::None) => {
            apply_command_state(state_ref, ctx);
            processed.result_text = Some(String::new());
            processed.should_query = false;
            processed.messages.clear();
        }
        Err(err) => {
            outcome.is_error = true;
            processed.result_text = Some(format!("Command /{} failed: {}", command_name, err));
            processed.should_query = false;
            processed.messages.clear();
        }
    }

    outcome
}

fn apply_command_state(
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    ctx: CommandContext,
) {
    let mut state = state_ref.write();
    state.messages = ctx.messages;
    state.app_state = ctx.app_state;
}

fn switch_command_session(
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    active_session_id_ref: &Arc<parking_lot::RwLock<crate::bootstrap::SessionId>>,
    session_id: crate::bootstrap::SessionId,
    messages: Vec<Message>,
    ctx: CommandContext,
) {
    {
        let mut state = state_ref.write();
        state.messages = messages;
        state.app_state = ctx.app_state;
    }
    *active_session_id_ref.write() = session_id.clone();
    crate::bootstrap::PROCESS_STATE.write().session_id = session_id;
}

fn clear_command_session(
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    active_session_id_ref: &Arc<parking_lot::RwLock<crate::bootstrap::SessionId>>,
    config: &crate::types::config::QueryEngineConfig,
    ctx: CommandContext,
) -> crate::bootstrap::SessionId {
    let previous_id = active_session_id_ref.read().clone();
    if config.auto_save_session && !ctx.messages.is_empty() {
        if let Err(err) =
            crate::session::storage::save_session(previous_id.as_str(), &ctx.messages, &config.cwd)
        {
            warn!(
                error = %err,
                session = %previous_id,
                "failed to save previous session before command clear"
            );
        }
    }

    let new_session_id = crate::bootstrap::SessionId::new();
    {
        let mut state = state_ref.write();
        state.messages.clear();
        state.usage = UsageTracking::default();
        state.permission_denials.clear();
        state.total_turn_count = 0;
        state.app_state = ctx.app_state;
    }
    *active_session_id_ref.write() = new_session_id.clone();
    crate::bootstrap::PROCESS_STATE.write().session_id = new_session_id.clone();
    new_session_id
}

struct SubmitSystemPrompt {
    system_prompt_parts: Vec<String>,
    user_context: std::collections::HashMap<String, String>,
    system_context: std::collections::HashMap<String, String>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "prompt assembly combines config, live state, hooks, tools, and model metadata"
)]
async fn build_submit_system_prompt(
    prompt: &str,
    config: &crate::types::config::QueryEngineConfig,
    session_id: &crate::bootstrap::SessionId,
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    hook_runner: &Arc<dyn cc_types::hooks::HookRunner>,
    tools_snapshot: &crate::types::tool::Tools,
    model_name: &str,
    backend_name: &str,
) -> SubmitSystemPrompt {
    // Pull live language/output_style off AppState so /config set takes effect
    // on the next submit without restarting the engine.
    let (
        cfg_language,
        cfg_output_style,
        include_auto_memory,
        session_memory_context,
        memory_query_text,
        recent_tool_names,
        already_surfaced_memory_keys,
        model_assisted_memory_recall,
    ) = {
        let state = state_ref.read();
        (
            state.app_state.settings.language.clone(),
            state.app_state.settings.output_style.clone(),
            state
                .app_state
                .settings
                .auto_memory_enabled
                .unwrap_or(false),
            state
                .session_memory
                .format_memory_context_for_workspace_excluding_session(
                    5,
                    Some(std::path::Path::new(&config.cwd)),
                    Some(session_id.as_str()),
                ),
            latest_user_query_text(&state.messages).unwrap_or_else(|| prompt.to_string()),
            recent_tool_names(&state.messages, 8),
            state.app_state.surfaced_memory_keys.clone(),
            model_assisted_memory_recall_enabled(),
        )
    };

    let ignore_memory = cc_session::memdir::query_requests_memory_ignore(&memory_query_text);
    let session_memory_context = if ignore_memory {
        None
    } else {
        session_memory_context
    };
    let (memory_context_override, newly_surfaced_memory_keys) = resolve_memory_context_override(
        config,
        include_auto_memory,
        &memory_query_text,
        &recent_tool_names,
        &already_surfaced_memory_keys,
        backend_name,
        model_name,
        model_assisted_memory_recall,
        ignore_memory,
    )
    .await;

    if !newly_surfaced_memory_keys.is_empty() {
        state_ref
            .write()
            .app_state
            .surfaced_memory_keys
            .extend(newly_surfaced_memory_keys);
    }

    let (system_prompt_parts, user_context, system_context) =
        system_prompt::build_system_prompt_with_memory_contexts(
            config.custom_system_prompt.as_deref(),
            config.append_system_prompt.as_deref(),
            tools_snapshot,
            model_name,
            &config.cwd,
            cfg_language.as_deref(),
            cfg_output_style.as_deref(),
            include_auto_memory,
            memory_context_override.as_deref(),
            session_memory_context.as_deref(),
        );

    fire_instructions_loaded_hook(state_ref, hook_runner, &system_prompt_parts, &config.cwd).await;

    SubmitSystemPrompt {
        system_prompt_parts,
        user_context,
        system_context,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "memory recall selection keeps the independent policy inputs explicit"
)]
async fn resolve_memory_context_override(
    config: &crate::types::config::QueryEngineConfig,
    include_auto_memory: bool,
    memory_query_text: &str,
    recent_tool_names: &[String],
    already_surfaced_memory_keys: &std::collections::HashSet<String>,
    backend_name: &str,
    model_name: &str,
    model_assisted_memory_recall: bool,
    ignore_memory: bool,
) -> (Option<String>, Vec<String>) {
    if ignore_memory {
        debug!("memory recall skipped because the user requested memory ignore");
        return (None, Vec::new());
    }

    if !model_assisted_memory_recall {
        debug!("using deterministic memory recall");
        return deterministic_memory_context(
            &config.cwd,
            include_auto_memory,
            memory_query_text,
            recent_tool_names,
            already_surfaced_memory_keys,
        );
    }

    match build_model_assisted_memory_context(
        &config.cwd,
        include_auto_memory,
        memory_query_text,
        recent_tool_names,
        already_surfaced_memory_keys,
        backend_name,
        model_name,
    )
    .await
    {
        Ok(Some((context, surfaced))) => {
            debug!(
                surfaced_count = surfaced.len(),
                "model-assisted memory recall completed"
            );
            (Some(context), surfaced)
        }
        Ok(None) => {
            debug!("model-assisted memory recall unavailable; using deterministic recall");
            deterministic_memory_context(
                &config.cwd,
                include_auto_memory,
                memory_query_text,
                recent_tool_names,
                already_surfaced_memory_keys,
            )
        }
        Err(error) => {
            debug!(
                error = %error,
                "model-assisted memory recall failed; using deterministic recall"
            );
            deterministic_memory_context(
                &config.cwd,
                include_auto_memory,
                memory_query_text,
                recent_tool_names,
                already_surfaced_memory_keys,
            )
        }
    }
}

async fn fire_instructions_loaded_hook(
    state_ref: &Arc<parking_lot::RwLock<super::QueryEngineState>>,
    hook_runner: &Arc<dyn cc_types::hooks::HookRunner>,
    system_prompt_parts: &[String],
    cwd: &str,
) {
    let content_length: usize = system_prompt_parts.iter().map(|part| part.len()).sum();
    if content_length == 0 {
        return;
    }

    let hooks_map = state_ref.read().app_state.hooks.clone();
    let configs = hook_runner.load_hook_configs(&hooks_map, "InstructionsLoaded");
    if !configs.is_empty() {
        let payload = serde_json::json!({
            "source": "system_prompt",
            "content_length": content_length,
            "cwd": cwd,
        });
        let _ = hook_runner
            .run_event_hooks("InstructionsLoaded", &payload, &configs)
            .await;
    }
}

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
        let session_id = self.current_session_id();
        info!(
            prompt_len = prompt.len(),
            source = ?query_source,
            session = %session_id,
            "submit_message: starting"
        );

        // Capture owned/cloned references for the async stream closure.
        let config = self.config.clone();
        let prompt = prompt.to_string();

        let state_ref = self.state.clone();
        let active_session_id_ref = self.active_session_id.clone();
        let aborted_ref = self.aborted.clone();
        let pending_bg_results = self.pending_bg_results.clone();
        let hook_runner = self.hook_runner.clone();
        let command_dispatcher = self.command_dispatcher.clone();
        let command_executor = self.command_executor.clone();
        let auto_classifier_fn = self.auto_classifier_fn.clone();

        let stream = async_stream::stream! {
            let mut submit_turn = SubmitTurnState::new();

            // Emit submit.received audit event
            {
                use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                let ctx = state_ref.read().audit_ctx.with_submit();
                ctx.emit(
                    EventKind::SubmitReceived,
                    Stage::Submit,
                    AuditLevel::Info,
                    Outcome::Started,
                    None,
                    Some(serde_json::json!({
                        "prompt_len": prompt.len(),
                        "source": format!("{:?}", query_source),
                    })),
                );
            }

            // ================================================================
            // PHASE A-pre: Fire UserPromptSubmit hook
            // ================================================================
            {
                let hooks_map = state_ref.read().app_state.hooks.clone();
                let configs = hook_runner.load_hook_configs(&hooks_map, "UserPromptSubmit");
                if !configs.is_empty() {
                    let payload = serde_json::json!({
                        "prompt": &prompt,
                    });
                    match hook_runner
                        .run_event_hooks("UserPromptSubmit", &payload, &configs)
                        .await
                    {
                        Ok(output) => {
                            if !output.should_continue {
                                info!("UserPromptSubmit hook blocked prompt");
                                let reason = output.reason
                                    .or(output.stop_reason)
                                    .unwrap_or_else(|| "Blocked by UserPromptSubmit hook".to_string());
                                yield SdkMessage::Result(SdkResult {
                                    subtype: ResultSubtype::Success,
                                    is_error: false,
                                    duration_ms: submit_turn.duration_ms(),
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
            let mut processed = input_processing::process_user_input(
                &prompt,
                &current_msgs_snapshot,
                &config.cwd,
                command_dispatcher.as_ref(),
            );

            let local_command = handle_parsed_command(
                &mut processed,
                &current_msgs_snapshot,
                &config,
                &state_ref,
                &active_session_id_ref,
                &session_id,
                command_dispatcher.as_ref(),
                command_executor.as_ref(),
            )
            .await;

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

            let (tools_snapshot, model_name, backend_name) = {
                let s = state_ref.read();
                let tools = s.tools.clone();
                let model = config
                    .user_specified_model
                    .clone()
                    .unwrap_or_else(|| s.app_state.main_loop_model.clone());
                let backend = s.app_state.main_loop_backend.clone();
                (tools, model, backend)
            };

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
                session_id: local_command.session_id.to_string(),
                uuid: Uuid::new_v4(),
            });

            // C.2: If this is a local command, yield result and return immediately.
            if !processed.should_query {
                let local_text = processed
                    .result_text
                    .clone()
                    .unwrap_or_default();

                yield SdkMessage::Result(SdkResult {
                    subtype: if local_command.is_error {
                        ResultSubtype::ErrorDuringExecution
                    } else {
                        ResultSubtype::Success
                    },
                    is_error: local_command.is_error,
                    duration_ms: submit_turn.duration_ms(),
                    duration_api_ms: 0,
                    num_turns: 0,
                    result: local_text.clone(),
                    stop_reason: None,
                    session_id: local_command.session_id.to_string(),
                    total_cost_usd: 0.0,
                    usage: UsageTracking::default(),
                    permission_denials: vec![],
                    structured_output: None,
                    uuid: Uuid::new_v4(),
                    errors: if local_command.is_error {
                        vec![local_text.clone()]
                    } else {
                        vec![]
                    },
                });
                return;
            }

            // ================================================================
            // PHASE B: System Prompt Build
            // ================================================================

            let prompt_build = build_submit_system_prompt(
                &prompt,
                &config,
                &session_id,
                &state_ref,
                &hook_runner,
                &tools_snapshot,
                &model_name,
                &backend_name,
            )
            .await;

            // ================================================================
            // PHASE D: Query Loop -- full message dispatch
            // ================================================================

            let current_messages = state_ref.read().messages.clone();

            let params = QueryParams {
                messages: current_messages,
                system_prompt: prompt_build.system_prompt_parts,
                user_context: prompt_build.user_context,
                system_context: prompt_build.system_context,
                fallback_model: config.fallback_model.clone(),
                query_source: query_source.clone(),
                max_output_tokens_override: None,
                max_turns: config.max_turns,
                skip_cache_write: None,
                task_budget: config.task_budget.clone(),
                gates: crate::types::config::QueryGates::from_env(
                    state_ref.read().app_state.fast_mode,
                ),
            };

            // Create API client for the selected backend.
            let mut submit_langfuse_trace = None;
            let api_client: Option<Arc<cc_api::api::client::ApiClient>> =
                cc_api::api::client::ApiClient::from_backend(Some(&backend_name)).map(Arc::new);
            if api_client.is_none() {
                let result = if codex_exec::is_codex_backend(&backend_name) {
                    format!(
                        "Codex backend requires {}. Optionally set {} and {}.",
                        cc_api::api::client::OPENAI_CODEX_TOKEN_ENV,
                        cc_api::api::client::OPENAI_CODEX_BASE_URL_ENV,
                        cc_api::api::client::OPENAI_CODEX_MODEL_ENV
                    )
                } else {
                    "No API provider configured. Set an API key in environment or use /login."
                        .to_string()
                };

                yield SdkMessage::Result(SdkResult {
                    subtype: ResultSubtype::ErrorDuringExecution,
                    is_error: true,
                    duration_ms: submit_turn.duration_ms(),
                    duration_api_ms: 0,
                    num_turns: 0,
                    result: result.clone(),
                    stop_reason: Some("api_error".to_string()),
                    session_id: session_id.to_string(),
                    total_cost_usd: 0.0,
                    usage: UsageTracking::default(),
                    permission_denials: vec![],
                    structured_output: submit_turn.structured_output.clone(),
                    uuid: Uuid::new_v4(),
                    errors: vec![result],
                });
                return;
            }

            if let Some(ref api_client) = api_client {
                let provider = api_client.langfuse_provider_name().to_string();
                submit_langfuse_trace = if let Some(agent_context) = config.agent_context.as_ref() {
                    crate::services::langfuse::create_subagent_trace(
                        &agent_context.langfuse_session_id,
                        agent_context
                            .agent_type
                            .as_deref()
                            .unwrap_or("general-purpose"),
                        &agent_context.agent_id,
                        &model_name,
                        &provider,
                        &prompt,
                    )
                } else {
                    let query_source_label = query_source.as_label();
                    crate::services::langfuse::create_trace(
                        session_id.as_str(),
                        &model_name,
                        &provider,
                        &prompt,
                        Some(query_source_label.as_str()),
                    )
                };
            }

            // Create deps for the inner query loop
            let permission_callback = state_ref.read().permission_callback.clone();
            let bg_agent_tx = state_ref.read().bg_agent_tx.clone();
            let tool_progress_callback = state_ref.read().tool_progress_callback.clone();
            let submit_audit_ctx = state_ref.read().audit_ctx.with_submit();
            let deps = Arc::new(QueryEngineDeps {
                aborted: aborted_ref.clone(),
                state: state_ref.clone(),
                cwd: config.cwd.clone(),
                session_id: session_id.to_string(),
                audit_ctx: submit_audit_ctx,
                langfuse_trace: submit_langfuse_trace.clone(),
                api_client,
                agent_context: config.agent_context.clone(),
                permission_callback,
                bg_agent_tx,
                tool_progress_callback,
                pending_bg_results: pending_bg_results.clone(),
                hook_runner: hook_runner.clone(),
                command_dispatcher: command_dispatcher.clone(),
                auto_classifier_fn: auto_classifier_fn.clone(),
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
                            submit_turn.last_stop_reason = Some(sr.clone());
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
                        submit_turn.turn_count_this_submit += 1;

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
                                tool_use_result: user_msg.tool_use_result.clone(),
                                source_tool_assistant_uuid: user_msg.source_tool_assistant_uuid,
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

                                submit_turn.collected_errors.push(error.message.clone());

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
                                let result_text =
                                    format!("Reached maximum of {} turns", max_turns);
                                let (usage_snap, denials_snap) = {
                                    let s = state_ref.read();
                                    (s.usage.clone(), s.permission_denials.clone())
                                };
                                crate::services::langfuse::end_trace(
                                    submit_langfuse_trace.take(),
                                    Some(&result_text),
                                    Some(crate::services::langfuse::TraceStatus::Error),
                                );

                                yield SdkMessage::Result(SdkResult {
                                    subtype: ResultSubtype::ErrorMaxTurns,
                                    is_error: true,
                                    duration_ms: submit_turn.duration_ms(),
                                    duration_api_ms: api_started_at
                                        .elapsed()
                                        .as_millis()
                                        as u64,
                                    num_turns: *turn_count,
                                    result: result_text,
                                    stop_reason: submit_turn.last_stop_reason.clone(),
                                    session_id: session_id.to_string(),
                                    total_cost_usd: usage_snap.total_cost_usd,
                                    usage: usage_snap,
                                    permission_denials: denials_snap,
                                    structured_output: submit_turn.structured_output
                                        .clone(),
                                    uuid: Uuid::new_v4(),
                                    errors: submit_turn.collected_errors.clone(),
                                });
                                return;
                            }

                            Attachment::StructuredOutput { data } => {
                                submit_turn.structured_output = Some(data.clone());
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
                                            tool_use_result: None,
                                            source_tool_assistant_uuid: None,
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
                                let _ = msg_usage;
                            }
                            StreamEvent::MessageDelta {
                                delta,
                                usage: _delta_usage,
                            } => {
                                if let Some(ref sr) = delta.stop_reason {
                                    submit_turn.last_stop_reason = Some(sr.clone());
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
                    QueryYield::Tombstone(ref tombstone) => {
                        debug!(
                            assistant_uuid = %tombstone.message.uuid,
                            "tombstone received (model fallback retry)"
                        );
                        {
                            let mut s = state_ref.write();
                            s.messages.retain(|message| {
                                !matches!(
                                    message,
                                    Message::Assistant(assistant)
                                        if assistant.uuid == tombstone.message.uuid
                                )
                            });
                        }

                        yield SdkMessage::Tombstone(SdkTombstone {
                            message: tombstone.message.clone(),
                            session_id: session_id.to_string(),
                            uuid: Uuid::new_v4(),
                        });

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
                        let result_text = format!(
                            "Stopped: cost ${:.4} exceeded budget ${:.4}",
                            current_cost, max_budget
                        );
                        crate::services::langfuse::end_trace(
                            submit_langfuse_trace.take(),
                            Some(&result_text),
                            Some(crate::services::langfuse::TraceStatus::Error),
                        );

                        yield SdkMessage::Result(SdkResult {
                            subtype: ResultSubtype::ErrorMaxBudgetUsd,
                            is_error: true,
                            duration_ms: submit_turn.duration_ms(),
                            duration_api_ms: api_started_at
                                .elapsed()
                                .as_millis()
                                as u64,
                            num_turns: submit_turn.turn_count_this_submit,
                            result: result_text,
                            stop_reason: submit_turn.last_stop_reason.clone(),
                            session_id: session_id.to_string(),
                            total_cost_usd: current_cost,
                            usage: usage_snap,
                            permission_denials: denials_snap,
                            structured_output: submit_turn.structured_output.clone(),
                            uuid: Uuid::new_v4(),
                            errors: submit_turn.collected_errors.clone(),
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
                submit_turn.last_stop_reason.as_deref(),
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

            let mut errors = std::mem::take(&mut submit_turn.collected_errors);
            if is_api_error {
                errors.push(text_result.clone());
            }

            // Record API duration in global ProcessState
            let api_duration_ms = api_started_at.elapsed().as_millis() as u64;
            crate::bootstrap::PROCESS_STATE
                .read()
                .api_duration.record(api_duration_ms);

            // Emit submit.completed audit event
            {
                use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                let ctx = state_ref.read().audit_ctx.clone();
                let outcome = if is_success { Outcome::Completed } else { Outcome::Failed };
                ctx.emit(
                    EventKind::SubmitCompleted,
                    Stage::Submit,
                    AuditLevel::Info,
                    outcome,
                    Some(submit_turn.duration_ms()),
                    Some(serde_json::json!({
                        "num_turns": submit_turn.turn_count_this_submit,
                        "cost_usd": usage_snap.total_cost_usd,
                        "is_error": !is_success,
                    })),
                );
                ctx.flush();
            }

            crate::services::langfuse::end_trace(
                submit_langfuse_trace.take(),
                Some(&text_result),
                if is_success {
                    None
                } else {
                    Some(crate::services::langfuse::TraceStatus::Error)
                },
            );

            yield SdkMessage::Result(SdkResult {
                subtype,
                is_error: !is_success,
                duration_ms: submit_turn.duration_ms(),
                duration_api_ms: api_started_at.elapsed().as_millis() as u64,
                num_turns: submit_turn.turn_count_this_submit,
                result: text_result,
                stop_reason: submit_turn.last_stop_reason,
                session_id: session_id.to_string(),
                total_cost_usd: usage_snap.total_cost_usd,
                usage: usage_snap,
                permission_denials: denials_snap,
                structured_output: submit_turn.structured_output,
                uuid: Uuid::new_v4(),
                errors,
            });
        };
        Box::pin(stream)
    }
}

fn latest_user_query_text(messages: &[Message]) -> Option<String> {
    messages.iter().rev().find_map(|message| {
        let Message::User(user) = message else {
            return None;
        };
        if user.is_meta {
            return None;
        }
        user_message_text(&user.content).filter(|text| !text.trim().is_empty())
    })
}

fn user_message_text(content: &MessageContent) -> Option<String> {
    match content {
        MessageContent::Text(text) => Some(text.clone()),
        MessageContent::Blocks(blocks) => {
            let text = blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
    }
}

fn recent_tool_names(messages: &[Message], limit: usize) -> Vec<String> {
    let mut names = Vec::new();
    for message in messages.iter().rev() {
        let Message::Assistant(assistant) = message else {
            continue;
        };
        for block in assistant.content.iter().rev() {
            let name = match block {
                ContentBlock::ToolUse { name, .. } | ContentBlock::ServerToolUse { name, .. } => {
                    name
                }
                _ => continue,
            };
            if !names.iter().any(|existing| existing == name) {
                names.push(name.clone());
                if names.len() >= limit {
                    return names;
                }
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// submit_preprocessed_input — centralized submit entry point
// ---------------------------------------------------------------------------

/// Submit a pre-processed input directly into the conversation.
///
/// This is the centralized submit entry point for both TUI and headless
/// frontends. It handles:
///
/// 1. Building attachment content blocks from `ProcessedInput.attachments`
/// 2. Emitting a user prompt progress message during submission
/// 3. Injecting the permission mode into the submit context
/// 4. Calling the existing submit pipeline
///
/// This function consumes the `ProcessedInput` and converts it into messages
/// ready for the main submit_message pipeline.
#[allow(dead_code)]
pub fn build_attachment_content_blocks(
    attachments: &[crate::input_processing::AttachmentInfo],
) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();

    for attachment in attachments {
        if let Some(block) = &attachment.content_block {
            blocks.push(block.clone());
        } else if let Some(path) = &attachment.file_path {
            // Attempt to load the file and create a content block
            if let Ok(content) = std::fs::read_to_string(path) {
                blocks.push(ContentBlock::Text { text: content });
            }
        }
    }

    blocks
}

/// Create a user prompt progress message to show during submission.
#[allow(dead_code)]
pub fn build_submit_progress_message(prompt_len: usize) -> Message {
    Message::Progress(crate::types::message::ProgressMessage {
        uuid: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        tool_use_id: "submit".to_string(),
        data: serde_json::json!({
            "event": "user_prompt_submit",
            "prompt_len": prompt_len,
        }),
    })
}

/// Submit a pre-processed input into the conversation.
///
/// This is the centralized submit entry point for both TUI and headless
/// frontends. It takes a `ProcessedInput` and converts attachments into
/// content blocks before passing the messages to the main pipeline.
///
/// Returns the modified `ProcessedInput` with built attachment content blocks.
#[allow(dead_code)]
pub fn submit_preprocessed_input(
    mut preprocessed: crate::input_processing::ProcessedInput,
) -> crate::input_processing::ProcessedInput {
    // Build attachment content blocks
    if !preprocessed.attachments.is_empty() {
        let blocks = build_attachment_content_blocks(&preprocessed.attachments);

        // If there are content blocks, update the first message
        if let Some(Message::User(ref mut user)) = preprocessed.messages.first_mut() {
            let mut all_blocks = Vec::new();

            // Add existing text content
            match &user.content {
                MessageContent::Text(text) if !text.trim().is_empty() => {
                    all_blocks.push(ContentBlock::Text {
                        text: text.clone(),
                    });
                }
                MessageContent::Blocks(existing) => {
                    all_blocks.extend(existing.clone());
                }
                _ => {}
            }

            // Add attachment blocks
            all_blocks.extend(blocks);

            if !all_blocks.is_empty() {
                user.content = MessageContent::Blocks(all_blocks);
            }
        }
    }

    preprocessed
}

#[cfg(test)]
mod model_assisted_memory_recall_tests {
    use super::*;

    #[test]
    fn truthy_model_assisted_memory_recall_values_are_explicit() {
        for value in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(is_truthy_model_assisted_memory_recall_value(value));
        }

        for value in ["", "0", "false", "off", "enabled"] {
            assert!(!is_truthy_model_assisted_memory_recall_value(value));
        }
    }
}
