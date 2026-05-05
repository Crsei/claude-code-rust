//! QueryEngineDeps -- QueryDeps implementation for the QueryEngine.
//!
//! Provides the query loop with access to the engine's shared state
//! (abort flag, app state, tools) and, optionally, a real `ApiClient`
//! for making Anthropic API calls.

use parking_lot::RwLock;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use uuid::Uuid;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::tools::execution::{
    enforce_result_size, find_tool, is_plan_mode_plan_file_write, sandbox_allowed_command_applies,
    security_validate, ToolExecutionResult,
};
use crate::types::app_state::AppState;
use crate::types::message::{Message, StreamEvent};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{PermissionMode, ToolProgress, Tools, ValidationResult};

use super::helpers::{build_messages_request, format_conversation_for_summary};
use super::QueryEngineState;

/// Dependency injection bridge: provides the query loop with access to the
/// engine's shared state (abort flag, app state, tools) and, optionally, a
/// real `ApiClient` for making Anthropic API calls.
pub(crate) struct QueryEngineDeps {
    pub(crate) aborted: Arc<AtomicBool>,
    pub(crate) state: Arc<RwLock<QueryEngineState>>,
    /// Audit context for this submit — carries correlation IDs.
    pub(crate) audit_ctx: crate::observability::AuditContext,
    pub(crate) langfuse_trace: Option<crate::services::langfuse::LangfuseTrace>,
    /// When `Some`, the deps will use this client for `call_model` /
    /// `call_model_streaming`. When `None`, those methods bail with a
    /// descriptive error.
    pub(crate) api_client: Option<Arc<crate::api::client::ApiClient>>,
    /// Sub-agent context -- propagated into `ToolUseContext` so that
    /// nested Agent tool calls can enforce recursion depth limits.
    pub(crate) agent_context: Option<crate::types::config::AgentContext>,
    /// Async callback for interactive permission prompts.
    /// Propagated into `ToolUseContext` for headless/TUI permission flow.
    pub(crate) permission_callback: Option<crate::types::tool::PermissionCallback>,
    /// Background agent sender — forwarded into ToolUseContext.
    pub(crate) bg_agent_tx: Option<crate::ipc::agent_channel::AgentSender>,
    /// Optional callback that receives every `ToolProgress` emitted by a
    /// tool. The query loop pulls this via `tool_progress_callback()` and
    /// hands it to `execute_tool_calls`. Tests can leave it unset; in
    /// headless mode the engine installs a closure that forwards each
    /// progress event to `FrontendSink`.
    pub(crate) tool_progress_callback: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    /// Shared buffer of completed background agents.
    pub(crate) pending_bg_results: cc_types::background_agents::PendingBackgroundResults,
    /// Hook runner — used via the `HookRunner` trait from `cc-types::hooks` so
    /// the engine has no direct dependency on `crate::tools::hooks`.
    pub(crate) hook_runner: Arc<dyn cc_types::hooks::HookRunner>,
    /// Command dispatcher — forwarded into `ToolUseContext` for tools that
    /// spawn child engines (e.g. Agent).
    pub(crate) command_dispatcher: Arc<dyn cc_types::commands::CommandDispatcher>,
}

fn central_permission_result_for_tool(
    tool_name: &str,
    input: &mut serde_json::Value,
    app_state: &AppState,
    hook_decision: Option<&crate::permissions::decision::HookPermissionDecision>,
) -> crate::types::tool::PermissionResult {
    use crate::permissions::decision::{
        self, PermissionBehavior, PermissionDecision, PermissionDecisionReason,
    };

    let plan_file_write_allowed = app_state.tool_permission_context.mode == PermissionMode::Plan
        && is_plan_mode_plan_file_write(tool_name, input);

    let mut decision = if plan_file_write_allowed {
        PermissionDecision {
            behavior: PermissionBehavior::Allow,
            updated_input: None,
            message: None,
            reason: PermissionDecisionReason::Mode {
                mode: "plan_file".to_string(),
            },
        }
    } else {
        decision::has_permissions_to_use_tool_with_hook(
            tool_name,
            input,
            &app_state.tool_permission_context,
            hook_decision,
            None,
        )
    };
    if matches!(&decision.behavior, PermissionBehavior::Ask)
        && matches!(&decision.reason, PermissionDecisionReason::Mode { .. })
        && app_state.tool_permission_context.mode != PermissionMode::Plan
        && sandbox_allowed_command_applies(tool_name, input, app_state)
    {
        decision = PermissionDecision {
            behavior: PermissionBehavior::Allow,
            updated_input: None,
            message: None,
            reason: PermissionDecisionReason::Mode {
                mode: "sandbox_allowed_command".to_string(),
            },
        };
    }
    let behavior = decision.behavior;
    let message = decision.message;
    if let Some(updated_input) = decision.updated_input {
        *input = updated_input;
    }

    match behavior {
        PermissionBehavior::Allow => crate::types::tool::PermissionResult::Allow {
            updated_input: input.clone(),
        },
        PermissionBehavior::Deny => crate::types::tool::PermissionResult::Deny {
            message: message.unwrap_or_else(|| "Permission blocked by policy.".to_string()),
        },
        PermissionBehavior::Ask => crate::types::tool::PermissionResult::Ask {
            message: message.unwrap_or_else(|| format!("Allow tool '{}'?", tool_name)),
        },
    }
}

fn tool_execution_result_to_exec_result(result: ToolExecutionResult) -> ToolExecResult {
    let mut tool_result = result.result;
    if !result.new_messages.is_empty() {
        tool_result.new_messages = result.new_messages;
    }

    ToolExecResult {
        tool_use_id: result.tool_use_id,
        tool_name: result.tool_name,
        result: tool_result,
        is_error: result.is_error,
    }
}

#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
    fn tool_progress_callback(&self) -> Option<Arc<dyn Fn(ToolProgress) + Send + Sync>> {
        self.tool_progress_callback.clone()
    }

    async fn call_model(&self, mut params: ModelCallParams) -> Result<ModelResponse> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model: no API client configured -- \
                 set a provider key (ANTHROPIC_API_KEY / OPENAI_API_KEY / OPENAI_CODEX_AUTH_TOKEN), \
                 use /login for Anthropic, or provide a mock in tests"
            )
        })?;

        // Fill model: AppState (user/config/env) > provider default
        if params.model.is_none() {
            let app_model = self.state.read().app_state.main_loop_model.clone();
            params.model = Some(if app_model.is_empty() {
                client.config().default_model.clone()
            } else {
                app_model
            });
        }

        // Strip advisor_model for providers that don't support it (issue #33).
        if !crate::api::client::provider_supports_advisor(&client.config().provider)
            && params.advisor_model.is_some()
        {
            tracing::debug!(
                provider = client.langfuse_provider_name(),
                "dropping advisor_model — provider does not support it"
            );
            params.advisor_model = None;
        }

        let request = build_messages_request(&params);
        let stream = client.messages_stream(request).await?;
        let mut stream = std::pin::pin!(stream);

        let mut accumulator = crate::api::streaming::StreamAccumulator::new();
        let mut stream_events = Vec::new();

        use futures::StreamExt;
        while let Some(event_result) = stream.next().await {
            let event = event_result?;
            accumulator.process_event(&event);
            stream_events.push(event);
        }

        let usage = accumulator.usage.clone();
        let model_id = params.model.as_deref().unwrap_or("unknown");
        let assistant_message = accumulator.build(model_id);

        Ok(ModelResponse {
            assistant_message,
            stream_events,
            usage,
        })
    }

    async fn call_model_streaming(
        &self,
        mut params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model_streaming: no API client configured -- \
                 set a provider key (ANTHROPIC_API_KEY / OPENAI_API_KEY / OPENAI_CODEX_AUTH_TOKEN), \
                 use /login for Anthropic, or provide a mock in tests"
            )
        })?;

        // Fill model: AppState (user/config/env) > provider default
        if params.model.is_none() {
            let app_model = self.state.read().app_state.main_loop_model.clone();
            params.model = Some(if app_model.is_empty() {
                client.config().default_model.clone()
            } else {
                app_model
            });
        }

        // Strip advisor_model for providers that don't support it (issue #33).
        if !crate::api::client::provider_supports_advisor(&client.config().provider)
            && params.advisor_model.is_some()
        {
            tracing::debug!(
                provider = client.langfuse_provider_name(),
                "dropping advisor_model — provider does not support it"
            );
            params.advisor_model = None;
        }

        let request = build_messages_request(&params);
        client.messages_stream(request).await
    }

    async fn microcompact(&self, messages: Vec<Message>) -> Result<Vec<Message>> {
        let result = crate::compact::microcompact::microcompact_messages(messages);
        if result.tokens_freed > 0 {
            tracing::debug!(
                tokens_freed = result.tokens_freed,
                "microcompact: trimmed old tool results"
            );
        }
        Ok(result.messages)
    }

    async fn autocompact(
        &self,
        messages: Vec<Message>,
        tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        let model = {
            let app = &self.state.read().app_state;
            if app.main_loop_model.is_empty() {
                self.api_client
                    .as_ref()
                    .map(|c| c.config().default_model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
            } else {
                app.main_loop_model.clone()
            }
        };

        // Run the local context pipeline (budget -> snip -> microcompact -> auto-compact check)
        let pipeline_result = crate::compact::pipeline::run_context_pipeline(
            messages.clone(),
            tracking.clone(),
            &model,
        )
        .await;

        // If auto-compact was triggered AND we have an API client, generate a model summary
        if let Some(ref updated_tracking) = pipeline_result.tracking {
            if updated_tracking.compacted {
                // Try model-based summarization if API client is available
                if let Some(ref _client) = self.api_client {
                    let summary_prompt = crate::compact::compaction::build_compaction_prompt();
                    let pre_tokens = crate::utils::tokens::estimate_messages_tokens(&messages);

                    // Build a summarization request
                    let summary_messages =
                        vec![Message::User(crate::types::message::UserMessage {
                            uuid: Uuid::new_v4(),
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            role: "user".into(),
                            content: crate::types::message::MessageContent::Text(
                                format_conversation_for_summary(&messages),
                            ),
                            is_meta: true,
                            tool_use_result: None,
                            source_tool_assistant_uuid: None,
                        })];

                    let summary_params = ModelCallParams {
                        messages: summary_messages,
                        system_prompt: vec![summary_prompt],
                        tools: vec![],
                        model: Some(model.clone()),
                        max_output_tokens: Some(20_000),
                        skip_cache_write: Some(true),
                        thinking_enabled: None,
                        effort_value: None,
                        advisor_model: None,
                    };

                    match self.call_model(summary_params).await {
                        Ok(response) => {
                            // Extract summary text from assistant response
                            let summary_text = response
                                .assistant_message
                                .content
                                .iter()
                                .filter_map(|b| match b {
                                    crate::types::message::ContentBlock::Text { text } => {
                                        Some(text.as_str())
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");

                            let config = crate::compact::compaction::CompactionConfig {
                                model: model.clone(),
                                session_id: String::new(),
                                query_source: "compact".into(),
                            };

                            let post_messages =
                                crate::compact::compaction::build_post_compact_messages(
                                    &summary_text,
                                    &messages,
                                    &config,
                                );

                            let post_tokens =
                                crate::utils::tokens::estimate_messages_tokens(&post_messages);

                            tracing::info!(
                                pre_tokens = pre_tokens,
                                post_tokens = post_tokens,
                                "autocompact: model-based summary complete"
                            );

                            let new_tracking = crate::compact::compaction::tracking_on_success(
                                tracking.as_ref(),
                                &Uuid::new_v4().to_string(),
                            );

                            return Ok(Some(CompactionResult {
                                messages: post_messages,
                                tracking: new_tracking,
                            }));
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "autocompact: model summary failed, using local pipeline");
                            let new_tracking =
                                crate::compact::compaction::tracking_on_failure(tracking.as_ref());
                            // Fall through to local-only result
                            return Ok(Some(CompactionResult {
                                messages: pipeline_result.messages,
                                tracking: new_tracking,
                            }));
                        }
                    }
                }

                // No API client -- return local pipeline result
                return Ok(Some(CompactionResult {
                    messages: pipeline_result.messages,
                    tracking: updated_tracking.clone(),
                }));
            }
        }

        // Pipeline ran but auto-compact was not triggered -- return local compacted messages
        if pipeline_result.compacted {
            return Ok(Some(CompactionResult {
                messages: pipeline_result.messages,
                tracking: tracking.unwrap_or(AutoCompactTracking {
                    compacted: false,
                    turn_counter: 0,
                    turn_id: String::new(),
                    consecutive_failures: 0,
                }),
            }));
        }

        Ok(None)
    }

    async fn reactive_compact(&self, messages: Vec<Message>) -> Result<Option<CompactionResult>> {
        let model = {
            let app = &self.state.read().app_state;
            if app.main_loop_model.is_empty() {
                self.api_client
                    .as_ref()
                    .map(|c| c.config().default_model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
            } else {
                app.main_loop_model.clone()
            }
        };

        match crate::compact::pipeline::try_reactive_compact(messages, &model).await {
            Some(result) => {
                tracing::info!(
                    tokens_freed = result.tokens_freed,
                    "reactive compact: freed tokens via emergency pipeline"
                );
                Ok(Some(CompactionResult {
                    messages: result.messages,
                    tracking: result.tracking,
                }))
            }
            None => Ok(None),
        }
    }

    async fn collapse_drain(
        &self,
        messages: Vec<Message>,
        tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        let model = {
            let app = &self.state.read().app_state;
            if app.main_loop_model.is_empty() {
                self.api_client
                    .as_ref()
                    .map(|c| c.config().default_model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
            } else {
                app.main_loop_model.clone()
            }
        };

        let pipeline_result =
            crate::compact::pipeline::run_context_pipeline(messages, tracking.clone(), &model)
                .await;

        if !pipeline_result.compacted {
            return Ok(None);
        }

        tracing::info!(
            snip_tokens_freed = pipeline_result.snip_tokens_freed,
            microcompact_tokens_freed = pipeline_result.microcompact_tokens_freed,
            context_collapse_tokens_freed = pipeline_result.context_collapse_tokens_freed,
            total_tokens_freed = pipeline_result.total_tokens_freed,
            "collapse drain: committed local context pipeline result"
        );

        Ok(Some(CompactionResult {
            messages: pipeline_result.messages,
            tracking: pipeline_result.tracking.unwrap_or_else(|| {
                tracking.unwrap_or(AutoCompactTracking {
                    compacted: false,
                    turn_counter: 0,
                    turn_id: String::new(),
                    consecutive_failures: 0,
                })
            }),
        }))
    }

    // Production implementation of the canonical query-loop tool execution
    // boundary declared in `QueryDeps`. Stage 2 keeps main-loop and future
    // stream-time scheduling routed here while folding in the remaining
    // validation/security/result-size stages from `tools::execution::run_tool_use`.
    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &crate::types::message::AssistantMessage,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        use crate::types::tool::PermissionResult;
        use cc_types::hooks::{PermissionOverride, PostToolHookResult, PreToolHookResult};

        // Hook dispatcher trait object — decouples the engine from the concrete
        // `crate::tools::hooks` impl (see issue #74, Phase 5b).
        let hooks = self.hook_runner.as_ref();

        let tool = find_tool(&request.tool_name, tools)
            .ok_or_else(|| anyhow::anyhow!("tool not found: {}", request.tool_name))?;

        let ctx = crate::types::tool::ToolUseContext {
            options: crate::types::tool::ToolUseOptions {
                debug: false,
                main_loop_model: self.state.read().app_state.main_loop_model.clone(),
                verbose: self.state.read().app_state.verbose,
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
            read_file_state: self.state.read().file_state_cache.clone(),
            get_app_state: {
                let state = self.state.clone();
                Arc::new(move || state.read().app_state.clone())
            },
            set_app_state: {
                let state = self.state.clone();
                Arc::new(move |updater: Box<dyn FnOnce(AppState) -> AppState>| {
                    let mut s = state.write();
                    let old = s.app_state.clone();
                    s.app_state = updater(old);
                })
            },
            session_id: self.audit_ctx.session_id.clone(),
            langfuse_session_id: self
                .langfuse_trace
                .as_ref()
                .map(|trace| trace.session_id.clone())
                .unwrap_or_else(|| self.audit_ctx.session_id.clone()),
            messages: vec![],
            agent_id: self.agent_context.as_ref().map(|ac| ac.agent_id.clone()),
            agent_type: self
                .agent_context
                .as_ref()
                .and_then(|ac| ac.agent_type.clone()),
            query_tracking: self
                .agent_context
                .as_ref()
                .map(|ac| ac.query_tracking.clone()),
            permission_callback: self.permission_callback.clone(),
            ask_user_callback: self.state.read().ask_user_callback.clone(),
            bg_agent_tx: self.bg_agent_tx.clone(),
            hook_runner: self.hook_runner.clone(),
            command_dispatcher: self.command_dispatcher.clone(),
        };

        // Load hook configs from AppState.
        let hooks_map = self.state.read().app_state.hooks.clone();
        let pre_configs = hooks.load_hook_configs(&hooks_map, "PreToolUse");
        let post_configs = hooks.load_hook_configs(&hooks_map, "PostToolUse");
        let failure_configs = hooks.load_hook_configs(&hooks_map, "PostToolUseFailure");

        // Pre-tool hooks.
        let execution_started = std::time::Instant::now();

        match tool.validate_input(&request.input, &ctx).await {
            ValidationResult::Ok => {}
            ValidationResult::Error { message, .. } => {
                return Ok(ToolExecResult {
                    tool_use_id: request.tool_use_id,
                    tool_name: request.tool_name,
                    result: crate::types::tool::ToolResult {
                        data: serde_json::json!(format!(
                            "Input validation error: {}. The schema was not sent - please check the tool's input requirements.",
                            message
                        )),
                        new_messages: vec![],
                        ..Default::default()
                    },
                    is_error: true,
                });
            }
        }

        let mut sanitized_input = request.input.clone();
        if let Some(obj) = sanitized_input.as_object_mut() {
            obj.remove("_simulatedSedEdit");
        }

        if let Some(result) = security_validate(
            &request.tool_use_id,
            &request.tool_name,
            &sanitized_input,
            tool.as_ref(),
            &ctx,
            execution_started,
        ) {
            return Ok(tool_execution_result_to_exec_result(result));
        }

        let (mut effective_input, permission_override) = match hooks
            .run_pre_tool_hooks(&request.tool_name, &sanitized_input, &pre_configs)
            .await
        {
            Ok(PreToolHookResult::Continue {
                updated_input,
                permission_override,
            }) => (
                updated_input.unwrap_or_else(|| sanitized_input.clone()),
                permission_override,
            ),
            Ok(PreToolHookResult::Stop { message }) => {
                return Ok(ToolExecResult {
                    tool_use_id: request.tool_use_id,
                    tool_name: request.tool_name,
                    result: crate::types::tool::ToolResult {
                        data: serde_json::json!(format!("Pre-tool hook stopped: {}", message)),
                        new_messages: vec![],
                        ..Default::default()
                    },
                    is_error: true,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "pre-tool hook error, continuing");
                (sanitized_input, None)
            }
        };

        // Permission check (tool-local checks first, then central rules/mode).
        let hook_decision = match permission_override.as_ref() {
            Some(PermissionOverride::Allow) => {
                tracing::debug!(
                    tool = %request.tool_name,
                    "Permission allow requested by hook override"
                );
                Some(crate::permissions::decision::HookPermissionDecision {
                    allow: true,
                    source: Some("PreToolUse".to_string()),
                    ..Default::default()
                })
            }
            Some(PermissionOverride::Deny { .. }) | None => None,
        };

        if let Some(PermissionOverride::Deny { reason }) = permission_override.as_ref() {
            // Fire PermissionDenied hook
            let deny_configs = hooks.load_hook_configs(&hooks_map, "PermissionDenied");
            if !deny_configs.is_empty() {
                let payload = serde_json::json!({
                    "tool_name": request.tool_name,
                    "tool_input": effective_input.clone(),
                    "reason": format!("Permission denied by hook: {}", reason),
                });
                let _ = hooks
                    .run_event_hooks("PermissionDenied", &payload, &deny_configs)
                    .await;
            }

            return Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: crate::types::tool::ToolResult {
                    data: serde_json::json!(format!("Permission denied by hook: {}", reason)),
                    new_messages: vec![],
                    ..Default::default()
                },
                is_error: true,
            });
        }

        {
            // Normal permission check via tool-local checks and the central rule engine
            let perm_audit_ctx = self.audit_ctx.with_tool_use(&request.tool_use_id);
            let perm_result = match tool.check_permissions(&effective_input, &ctx).await {
                PermissionResult::Allow { updated_input } => {
                    effective_input = updated_input;
                    let app_state = (ctx.get_app_state)();
                    central_permission_result_for_tool(
                        &request.tool_name,
                        &mut effective_input,
                        &app_state,
                        hook_decision.as_ref(),
                    )
                }
                other => other,
            };
            match perm_result {
                PermissionResult::Allow { updated_input } => {
                    effective_input = updated_input;
                }
                PermissionResult::Deny { message } => {
                    // Emit permission.resolved(denied) audit event
                    {
                        use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                        perm_audit_ctx.emit(
                            EventKind::PermissionResolved,
                            Stage::Permission,
                            AuditLevel::Warn,
                            Outcome::Denied,
                            None,
                            Some(serde_json::json!({
                                "tool_name": request.tool_name,
                                "decision": "deny",
                                "reason": message,
                            })),
                        );
                    }
                    // Fire PermissionDenied hook
                    let deny_configs = hooks.load_hook_configs(&hooks_map, "PermissionDenied");
                    if !deny_configs.is_empty() {
                        let payload = serde_json::json!({
                            "tool_name": request.tool_name,
                            "tool_input": effective_input.clone(),
                            "reason": format!("Permission denied: {}", message),
                        });
                        let _ = hooks
                            .run_event_hooks("PermissionDenied", &payload, &deny_configs)
                            .await;
                    }

                    return Ok(ToolExecResult {
                        tool_use_id: request.tool_use_id,
                        tool_name: request.tool_name,
                        result: crate::types::tool::ToolResult {
                            data: serde_json::json!(format!("Permission denied: {}", message)),
                            new_messages: vec![],
                            ..Default::default()
                        },
                        is_error: true,
                    });
                }
                PermissionResult::Ask { message } => {
                    // Emit permission.requested audit event
                    {
                        use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                        perm_audit_ctx.emit(
                            EventKind::PermissionRequested,
                            Stage::Permission,
                            AuditLevel::Info,
                            Outcome::Info,
                            None,
                            Some(serde_json::json!({
                                "tool_name": request.tool_name,
                                "message": message,
                            })),
                        );
                    }

                    // Fire PermissionRequest hook before interactive prompt
                    let mut hook_allowed = false;
                    let perm_req_configs = hooks.load_hook_configs(&hooks_map, "PermissionRequest");
                    if !perm_req_configs.is_empty() {
                        let payload = serde_json::json!({
                            "tool_name": request.tool_name,
                            "tool_input": effective_input.clone(),
                            "message": message,
                        });
                        if let Ok(output) = hooks
                            .run_event_hooks("PermissionRequest", &payload, &perm_req_configs)
                            .await
                        {
                            // If hook provides a permission decision, use it
                            if let Some(ref decision) = output.permission_decision {
                                match decision.as_str() {
                                    "allow" => {
                                        // Skip the interactive prompt, proceed to execution
                                        tracing::debug!(
                                            tool = %request.tool_name,
                                            "PermissionRequest hook allowed tool execution"
                                        );
                                        hook_allowed = true;
                                    }
                                    "deny" => {
                                        // Fire PermissionDenied hook
                                        let deny_configs =
                                            hooks.load_hook_configs(&hooks_map, "PermissionDenied");
                                        if !deny_configs.is_empty() {
                                            let deny_payload = serde_json::json!({
                                                "tool_name": request.tool_name,
                                                "tool_input": effective_input.clone(),
                                                "reason": "Permission denied by PermissionRequest hook",
                                            });
                                            let _ = hooks
                                                .run_event_hooks(
                                                    "PermissionDenied",
                                                    &deny_payload,
                                                    &deny_configs,
                                                )
                                                .await;
                                        }

                                        return Ok(ToolExecResult {
                                            tool_use_id: request.tool_use_id,
                                            tool_name: request.tool_name,
                                            result: crate::types::tool::ToolResult {
                                                data: serde_json::json!(
                                                    "Permission denied by hook"
                                                ),
                                                new_messages: vec![],
                                                ..Default::default()
                                            },
                                            is_error: true,
                                        });
                                    }
                                    _ => {} // unknown decision, continue with normal prompt
                                }
                            }
                        }
                    }

                    if !hook_allowed {
                        if let Some(ref callback) = ctx.permission_callback {
                            let description = format!("{}: {}", request.tool_name, message);
                            let options = vec![
                                "Allow".to_string(),
                                "Deny".to_string(),
                                "Always Allow".to_string(),
                            ];
                            let decision = callback(
                                request.tool_use_id.clone(),
                                request.tool_name.clone(),
                                description,
                                options,
                            )
                            .await;

                            match decision.to_lowercase().as_str() {
                                "allow" => {
                                    // Emit permission.resolved(allow) audit event
                                    use crate::observability::{
                                        AuditLevel, EventKind, Outcome, Stage,
                                    };
                                    perm_audit_ctx.emit(
                                        EventKind::PermissionResolved,
                                        Stage::Permission,
                                        AuditLevel::Info,
                                        Outcome::Completed,
                                        None,
                                        Some(serde_json::json!({
                                            "tool_name": request.tool_name,
                                            "decision": "allow",
                                        })),
                                    );
                                }
                                "always_allow" => {
                                    // Record a session-level grant so subsequent
                                    // calls to this tool don't re-prompt.
                                    self.state
                                        .write()
                                        .app_state
                                        .tool_permission_context
                                        .grant_session_allow(&request.tool_name);
                                    tracing::debug!(
                                        tool = %request.tool_name,
                                        "session-level always_allow grant recorded"
                                    );
                                }
                                _ => {
                                    // Emit permission.resolved(denied) audit event
                                    {
                                        use crate::observability::{
                                            AuditLevel, EventKind, Outcome, Stage,
                                        };
                                        perm_audit_ctx.emit(
                                            EventKind::PermissionResolved,
                                            Stage::Permission,
                                            AuditLevel::Warn,
                                            Outcome::Denied,
                                            None,
                                            Some(serde_json::json!({
                                                "tool_name": request.tool_name,
                                                "decision": "deny",
                                                "source": "user",
                                            })),
                                        );
                                    }

                                    // Fire PermissionDenied hook (user chose deny)
                                    let deny_configs =
                                        hooks.load_hook_configs(&hooks_map, "PermissionDenied");
                                    if !deny_configs.is_empty() {
                                        let payload = serde_json::json!({
                                            "tool_name": request.tool_name,
                                            "tool_input": effective_input.clone(),
                                            "reason": "Permission denied by user",
                                        });
                                        let _ = hooks
                                            .run_event_hooks(
                                                "PermissionDenied",
                                                &payload,
                                                &deny_configs,
                                            )
                                            .await;
                                    }

                                    return Ok(ToolExecResult {
                                        tool_use_id: request.tool_use_id,
                                        tool_name: request.tool_name,
                                        result: crate::types::tool::ToolResult {
                                            data: serde_json::json!("Permission denied by user."),
                                            new_messages: vec![],
                                            ..Default::default()
                                        },
                                        is_error: true,
                                    });
                                }
                            }
                        } else {
                            // Fire PermissionDenied hook (no callback available)
                            let deny_configs =
                                hooks.load_hook_configs(&hooks_map, "PermissionDenied");
                            if !deny_configs.is_empty() {
                                let payload = serde_json::json!({
                                    "tool_name": request.tool_name,
                                    "tool_input": effective_input.clone(),
                                    "reason": format!("Permission required (no callback): {}", message),
                                });
                                let _ = hooks
                                    .run_event_hooks("PermissionDenied", &payload, &deny_configs)
                                    .await;
                            }

                            return Ok(ToolExecResult {
                                tool_use_id: request.tool_use_id,
                                tool_name: request.tool_name,
                                result: crate::types::tool::ToolResult {
                                    data: serde_json::json!(format!(
                                        "Permission required: {}",
                                        message
                                    )),
                                    new_messages: vec![],
                                    ..Default::default()
                                },
                                is_error: true,
                            });
                        }
                    } // if !hook_allowed
                }
            }
        }

        // Tool execution with post-hooks.

        // Emit tool.start audit event
        let tool_audit_ctx = self.audit_ctx.with_tool_use(&request.tool_use_id);
        let tool_langfuse_span = self.langfuse_trace.as_ref().and_then(|trace| {
            crate::services::langfuse::create_tool_span(
                trace,
                &request.tool_name,
                &request.tool_use_id,
                &effective_input,
                request.langfuse_batch_span.as_ref(),
            )
        });
        {
            use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
            tool_audit_ctx.emit(
                EventKind::ToolStart,
                Stage::ToolExecution,
                AuditLevel::Info,
                Outcome::Started,
                None,
                Some(serde_json::json!({
                    "tool_name": request.tool_name,
                })),
            );
        }
        let tool_start = std::time::Instant::now();

        // Adapt the `Arc` from `QueryDeps::execute_tool` into the `Box`
        // the `Tool::call` contract expects. The wrapper also stamps the
        // current `request.tool_use_id` onto each `ToolProgress` so
        // downstream tools don't need to know it themselves.
        let tool_use_id_for_progress = request.tool_use_id.clone();
        let boxed_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>> =
            on_progress.as_ref().map(|arc| {
                let arc = arc.clone();
                let tool_use_id = tool_use_id_for_progress.clone();
                Box::new(move |mut p: ToolProgress| {
                    if p.tool_use_id.is_empty() {
                        p.tool_use_id = tool_use_id.clone();
                    }
                    arc(p)
                }) as Box<dyn Fn(ToolProgress) + Send + Sync>
            });

        match tool
            .call(
                effective_input.clone(),
                &ctx,
                parent_message,
                boxed_progress,
            )
            .await
        {
            Ok(mut result) => {
                let result_preview =
                    result
                        .display_preview
                        .clone()
                        .unwrap_or_else(|| match &result.data {
                            serde_json::Value::String(value) => value.clone(),
                            other => {
                                serde_json::to_string(other).unwrap_or_else(|_| "null".to_string())
                            }
                        });
                crate::services::langfuse::finish_tool_span(
                    tool_langfuse_span,
                    &request.tool_name,
                    &result_preview,
                    false,
                );
                // Emit tool.finish audit event
                {
                    use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                    tool_audit_ctx.emit(
                        EventKind::ToolFinish,
                        Stage::ToolExecution,
                        AuditLevel::Info,
                        Outcome::Completed,
                        Some(tool_start.elapsed().as_millis() as u64),
                        Some(serde_json::json!({
                            "tool_name": request.tool_name,
                        })),
                    );
                }

                // Run post-tool hooks on success
                if !post_configs.is_empty() {
                    if let Ok(PostToolHookResult::StopContinuation { message }) = hooks
                        .run_post_tool_hooks(
                            &request.tool_name,
                            &effective_input,
                            &result.data,
                            &post_configs,
                        )
                        .await
                    {
                        tracing::debug!(
                            message = %message,
                            "post-tool hook stopped continuation"
                        );
                    }
                }

                result.data = enforce_result_size(result.data, tool.max_result_size_chars());

                Ok(ToolExecResult {
                    tool_use_id: request.tool_use_id,
                    tool_name: request.tool_name,
                    result,
                    is_error: false,
                })
            }
            Err(e) => {
                crate::services::langfuse::finish_tool_span(
                    tool_langfuse_span,
                    &request.tool_name,
                    &e.to_string(),
                    true,
                );
                // Emit tool.error audit event
                {
                    use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                    tool_audit_ctx.emit(
                        EventKind::ToolError,
                        Stage::ToolExecution,
                        AuditLevel::Error,
                        Outcome::Failed,
                        Some(tool_start.elapsed().as_millis() as u64),
                        Some(serde_json::json!({
                            "tool_name": request.tool_name,
                            "error": e.to_string(),
                        })),
                    );
                }

                // Run post-failure hooks on error
                if !failure_configs.is_empty() {
                    let _ = hooks
                        .run_post_tool_failure_hooks(
                            &request.tool_name,
                            &effective_input,
                            &e.to_string(),
                            &failure_configs,
                        )
                        .await;
                }

                Ok(ToolExecResult {
                    tool_use_id: request.tool_use_id,
                    tool_name: request.tool_name,
                    result: crate::types::tool::ToolResult {
                        data: serde_json::json!(format!("Error: {}", e)),
                        new_messages: vec![],
                        ..Default::default()
                    },
                    is_error: true,
                })
            }
        }
    }

    fn get_app_state(&self) -> AppState {
        self.state.read().app_state.clone()
    }

    fn uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        self.state.read().tools.clone()
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        Ok(self.state.read().tools.clone())
    }

    fn drain_background_results(
        &self,
    ) -> Vec<cc_types::background_agents::CompletedBackgroundAgent> {
        self.pending_bg_results.drain_all()
    }

    fn hook_runner(&self) -> Arc<dyn cc_types::hooks::HookRunner> {
        self.hook_runner.clone()
    }

    fn audit_context(&self) -> crate::observability::AuditContext {
        self.audit_ctx.clone()
    }

    fn langfuse_trace(&self) -> Option<crate::services::langfuse::LangfuseTrace> {
        self.langfuse_trace.clone()
    }

    fn langfuse_provider_name(&self) -> Option<String> {
        self.api_client
            .as_ref()
            .map(|client| client.langfuse_provider_name().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::lifecycle::QueryEngine;
    use crate::types::config::QueryEngineConfig;
    use crate::types::message::{AssistantMessage, ToolResultContent};
    use crate::types::tool::{
        PermissionCallback, PermissionMode, PermissionResult, Tool, ToolResult, ToolUseContext,
    };
    use serde_json::{json, Value};

    struct CanonicalTool {
        name: &'static str,
        validation_error: Option<&'static str>,
        permission: Option<PermissionResult>,
        result: ToolResult,
        max_result_size_chars: usize,
        seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
        progress_payload: Option<Value>,
    }

    #[async_trait::async_trait]
    impl Tool for CanonicalTool {
        fn name(&self) -> &str {
            self.name
        }

        async fn description(&self, _input: &Value) -> String {
            String::new()
        }

        fn input_json_schema(&self) -> Value {
            json!({})
        }

        async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
            if let Some(message) = self.validation_error {
                ValidationResult::Error {
                    message: message.to_string(),
                    error_code: 1,
                }
            } else {
                ValidationResult::Ok
            }
        }

        async fn check_permissions(
            &self,
            input: &Value,
            _ctx: &ToolUseContext,
        ) -> PermissionResult {
            self.permission
                .clone()
                .unwrap_or_else(|| PermissionResult::Allow {
                    updated_input: input.clone(),
                })
        }

        async fn call(
            &self,
            input: Value,
            _ctx: &ToolUseContext,
            _parent_message: &AssistantMessage,
            on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> Result<ToolResult> {
            *self.seen_input.lock() = Some(input);
            if let (Some(callback), Some(payload)) = (on_progress, self.progress_payload.clone()) {
                callback(ToolProgress {
                    tool_use_id: String::new(),
                    data: payload,
                });
            }
            Ok(self.result.clone())
        }

        async fn prompt(&self) -> String {
            String::new()
        }

        fn max_result_size_chars(&self) -> usize {
            self.max_result_size_chars
        }
    }

    fn make_config(tools: Tools) -> QueryEngineConfig {
        QueryEngineConfig {
            cwd: ".".to_string(),
            tools,
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
            persist_session: false,
            resolved_model: None,
            auto_save_session: false,
            agent_context: None,
        }
    }

    fn make_deps(tools: Tools, mode: PermissionMode) -> QueryEngineDeps {
        let engine = QueryEngine::new(make_config(tools));
        engine.state.write().app_state.tool_permission_context.mode = mode;

        QueryEngineDeps {
            aborted: engine.aborted.clone(),
            state: engine.state.clone(),
            audit_ctx: crate::observability::AuditContext::noop("test"),
            langfuse_trace: None,
            api_client: None,
            agent_context: None,
            permission_callback: None,
            bg_agent_tx: None,
            tool_progress_callback: None,
            pending_bg_results: cc_types::background_agents::PendingBackgroundResults::new(),
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        }
    }

    fn parent_message() -> AssistantMessage {
        AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }

    fn tool_request(tool_name: &str, input: Value) -> ToolExecRequest {
        ToolExecRequest {
            tool_use_id: "tu_test".to_string(),
            tool_name: tool_name.to_string(),
            input,
            langfuse_batch_span: None,
        }
    }

    fn canonical_tool(
        name: &'static str,
        seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
    ) -> Arc<CanonicalTool> {
        Arc::new(CanonicalTool {
            name,
            validation_error: None,
            permission: None,
            result: ToolResult {
                data: json!("ok"),
                new_messages: vec![],
                ..Default::default()
            },
            max_result_size_chars: 100_000,
            seen_input,
            progress_payload: None,
        })
    }

    #[test]
    fn central_permission_default_mode_asks_for_bash_after_tool_allow() {
        let app_state = AppState::default();
        let mut input = json!({"command": "rm -rf F:/temp/gomoku_subagent/*"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None);

        assert!(
            matches!(result, PermissionResult::Ask { .. }),
            "default mode must ask even when the tool-local check allowed the command"
        );
    }

    #[test]
    fn central_permission_allow_rule_still_allows_matching_bash_prefix() {
        let mut app_state = AppState::default();
        app_state
            .tool_permission_context
            .always_allow_rules
            .insert("test".into(), vec!["Bash(prefix:git)".into()]);
        let mut input = json!({"command": "git status"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None);

        assert!(matches!(result, PermissionResult::Allow { .. }));
    }

    #[test]
    fn central_permission_sandbox_allowed_command_allows_workspace_bash() {
        let mut app_state = AppState::default();
        app_state.settings.sandbox.enabled = Some(true);
        app_state.settings.sandbox.mode = Some("workspace".into());
        app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
        let mut input = json!({"command": "cargo test --all"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None);

        assert!(matches!(result, PermissionResult::Allow { .. }));
    }

    #[test]
    fn central_permission_sandbox_allowed_command_does_not_override_ask_rule() {
        let mut app_state = AppState::default();
        app_state.settings.sandbox.enabled = Some(true);
        app_state.settings.sandbox.mode = Some("workspace".into());
        app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
        app_state
            .tool_permission_context
            .always_ask_rules
            .insert("test".into(), vec!["Bash".into()]);
        let mut input = json!({"command": "cargo test --all"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None);

        assert!(matches!(result, PermissionResult::Ask { .. }));
    }

    #[test]
    fn central_permission_sandbox_allowed_command_does_not_override_plan_mode() {
        let mut app_state = AppState::default();
        app_state.tool_permission_context.mode = PermissionMode::Plan;
        app_state.settings.sandbox.enabled = Some(true);
        app_state.settings.sandbox.mode = Some("workspace".into());
        app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
        let mut input = json!({"command": "cargo test --all"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None);

        assert!(matches!(result, PermissionResult::Ask { .. }));
    }

    #[test]
    fn central_permission_ask_rule_overrides_hook_allow() {
        let mut app_state = AppState::default();
        app_state
            .tool_permission_context
            .always_ask_rules
            .insert("test".into(), vec!["Bash".into()]);
        let mut input = json!({"command": "git status"});
        let hook_decision = crate::permissions::decision::HookPermissionDecision {
            allow: true,
            source: Some("PreToolUse:test".into()),
            ..Default::default()
        };

        let result = central_permission_result_for_tool(
            "Bash",
            &mut input,
            &app_state,
            Some(&hook_decision),
        );

        assert!(matches!(result, PermissionResult::Ask { .. }));
    }

    #[tokio::test]
    async fn execute_tool_rejects_validation_error_before_call() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = Arc::new(CanonicalTool {
            name: "ValidateMe",
            validation_error: Some("missing required field"),
            permission: None,
            result: ToolResult::default(),
            max_result_size_chars: 100_000,
            seen_input: seen_input.clone(),
            progress_payload: None,
        });
        let deps = make_deps(vec![tool], PermissionMode::Bypass);

        let result = deps
            .execute_tool(
                tool_request("ValidateMe", json!({})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result
            .result
            .data
            .as_str()
            .is_some_and(|text| text.contains("Input validation error")));
        assert!(
            seen_input.lock().is_none(),
            "validation failure must stop before Tool::call"
        );
    }

    #[tokio::test]
    async fn execute_tool_sanitizes_input_and_enforces_result_size() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = Arc::new(CanonicalTool {
            name: "SanitizeMe",
            validation_error: None,
            permission: None,
            result: ToolResult {
                data: json!("x".repeat(200)),
                model_content: Some(ToolResultContent::Text("model content".to_string())),
                display_preview: Some("preview".to_string()),
                new_messages: vec![],
            },
            max_result_size_chars: 40,
            seen_input: seen_input.clone(),
            progress_payload: None,
        });
        let deps = make_deps(vec![tool], PermissionMode::Bypass);

        let result = deps
            .execute_tool(
                tool_request(
                    "SanitizeMe",
                    json!({"keep": true, "_simulatedSedEdit": "remove me"}),
                ),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let input = seen_input.lock().clone().expect("tool should be called");
        assert_eq!(input.get("keep"), Some(&json!(true)));
        assert!(
            input.get("_simulatedSedEdit").is_none(),
            "canonical path should strip simulated edit marker"
        );
        assert!(
            result
                .result
                .data
                .as_str()
                .is_some_and(|text| text.contains("characters omitted")),
            "canonical path should enforce max_result_size_chars"
        );
        assert_eq!(result.result.display_preview.as_deref(), Some("preview"));
        assert!(matches!(
            result.result.model_content,
            Some(ToolResultContent::Text(ref text)) if text == "model content"
        ));
    }

    #[tokio::test]
    async fn execute_tool_blocks_dangerous_command_before_call() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("Bash", seen_input.clone());
        let deps = make_deps(vec![tool], PermissionMode::Default);

        let result = deps
            .execute_tool(
                tool_request("Bash", json!({"command": "rm -rf /"})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result
            .result
            .data
            .as_str()
            .is_some_and(|text| text.contains("Dangerous command blocked")));
        assert!(
            seen_input.lock().is_none(),
            "security validation must stop before Tool::call"
        );
    }

    #[tokio::test]
    async fn execute_tool_returns_permission_deny_without_call() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = Arc::new(CanonicalTool {
            name: "DenyMe",
            validation_error: None,
            permission: Some(PermissionResult::Deny {
                message: "blocked by test".to_string(),
            }),
            result: ToolResult::default(),
            max_result_size_chars: 100_000,
            seen_input: seen_input.clone(),
            progress_payload: None,
        });
        let deps = make_deps(vec![tool], PermissionMode::Bypass);

        let result = deps
            .execute_tool(
                tool_request("DenyMe", json!({})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result
            .result
            .data
            .as_str()
            .is_some_and(|text| text.contains("Permission denied: blocked by test")));
        assert!(
            seen_input.lock().is_none(),
            "permission denial must stop before Tool::call"
        );
    }

    #[tokio::test]
    async fn execute_tool_preserves_ask_callback_and_progress_ids() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = Arc::new(CanonicalTool {
            name: "AskMe",
            validation_error: None,
            permission: Some(PermissionResult::Ask {
                message: "needs approval".to_string(),
            }),
            result: ToolResult {
                data: json!("approved"),
                new_messages: vec![],
                ..Default::default()
            },
            max_result_size_chars: 100_000,
            seen_input,
            progress_payload: Some(json!({"phase": "running"})),
        });
        let mut deps = make_deps(vec![tool], PermissionMode::Default);
        let callback: PermissionCallback =
            Arc::new(|_, _, _, _| Box::pin(async { "allow".to_string() }));
        deps.permission_callback = Some(callback);
        let seen_progress = Arc::new(parking_lot::Mutex::new(None));
        let progress_callback: Arc<dyn Fn(ToolProgress) + Send + Sync> = {
            let seen_progress = seen_progress.clone();
            Arc::new(move |progress| {
                *seen_progress.lock() = Some(progress);
            })
        };

        let result = deps
            .execute_tool(
                tool_request("AskMe", json!({})),
                &deps.get_tools(),
                &parent_message(),
                Some(progress_callback),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let progress = seen_progress
            .lock()
            .clone()
            .expect("tool should emit progress");
        assert_eq!(progress.tool_use_id, "tu_test");
        assert_eq!(progress.data, json!({"phase": "running"}));
    }
}
