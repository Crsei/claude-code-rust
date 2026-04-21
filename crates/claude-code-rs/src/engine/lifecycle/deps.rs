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
use crate::types::app_state::AppState;
use crate::types::message::{Message, StreamEvent};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

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
    /// Shared buffer of completed background agents.
    pub(crate) pending_bg_results: crate::tools::background_agents::PendingBackgroundResults,
    /// Hook runner — used via the `HookRunner` trait from `cc-types::hooks` so
    /// the engine has no direct dependency on `crate::tools::hooks`.
    pub(crate) hook_runner: Arc<dyn cc_types::hooks::HookRunner>,
    /// Command dispatcher — forwarded into `ToolUseContext` for tools that
    /// spawn child engines (e.g. Agent).
    pub(crate) command_dispatcher: Arc<dyn cc_types::commands::CommandDispatcher>,
}

#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
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

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &crate::types::message::AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        use cc_types::hooks::{PermissionOverride, PostToolHookResult, PreToolHookResult};
        use crate::types::tool::PermissionResult;

        // Hook dispatcher trait object — decouples the engine from the concrete
        // `crate::tools::hooks` impl (see issue #74, Phase 5b).
        let hooks = self.hook_runner.as_ref();

        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool_name)
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
            read_file_state: crate::types::tool::FileStateCache::default(),
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

        // ── Load hook configs from AppState ────────────────────────
        let hooks_map = self.state.read().app_state.hooks.clone();
        let pre_configs = hooks.load_hook_configs(&hooks_map, "PreToolUse");
        let post_configs = hooks.load_hook_configs(&hooks_map, "PostToolUse");
        let failure_configs = hooks.load_hook_configs(&hooks_map, "PostToolUseFailure");

        // ── Pre-tool hooks ─────────────────────────────────────────
        let (effective_input, permission_override) =
            match hooks
                .run_pre_tool_hooks(&request.tool_name, &request.input, &pre_configs)
                .await
            {
                Ok(PreToolHookResult::Continue {
                    updated_input,
                    permission_override,
                }) => (
                    updated_input.unwrap_or_else(|| request.input.clone()),
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
                    (request.input.clone(), None)
                }
            };

        // ── Permission check (hook override first, then rule engine) ──
        if let Some(override_decision) = permission_override {
            match override_decision {
                PermissionOverride::Deny { reason } => {
                    // Fire PermissionDenied hook
                    let deny_configs = hooks.load_hook_configs(&hooks_map, "PermissionDenied");
                    if !deny_configs.is_empty() {
                        let payload = serde_json::json!({
                            "tool_name": request.tool_name,
                            "tool_input": request.input,
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
                            data: serde_json::json!(format!(
                                "Permission denied by hook: {}",
                                reason
                            )),
                            new_messages: vec![],
                            ..Default::default()
                        },
                        is_error: true,
                    });
                }
                PermissionOverride::Allow => {
                    tracing::debug!(
                        tool = %request.tool_name,
                        "permission allowed by hook override"
                    );
                }
            }
        } else {
            // Normal permission check via the rule engine
            let perm_audit_ctx = self.audit_ctx.with_tool_use(&request.tool_use_id);
            let perm_result = tool.check_permissions(&effective_input, &ctx).await;
            match perm_result {
                PermissionResult::Allow { .. } => { /* proceed */ }
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
                            "tool_input": request.input,
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
                    let perm_req_configs =
                        hooks.load_hook_configs(&hooks_map, "PermissionRequest");
                    if !perm_req_configs.is_empty() {
                        let payload = serde_json::json!({
                            "tool_name": request.tool_name,
                            "tool_input": request.input,
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
                                        let deny_configs = hooks
                                            .load_hook_configs(&hooks_map, "PermissionDenied");
                                        if !deny_configs.is_empty() {
                                            let deny_payload = serde_json::json!({
                                                "tool_name": request.tool_name,
                                                "tool_input": request.input,
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
                                    let deny_configs = hooks
                                        .load_hook_configs(&hooks_map, "PermissionDenied");
                                    if !deny_configs.is_empty() {
                                        let payload = serde_json::json!({
                                            "tool_name": request.tool_name,
                                            "tool_input": request.input,
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
                                    "tool_input": request.input,
                                    "reason": format!("Permission required (no callback): {}", message),
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

        // ── Tool execution with post-hooks ─────────────────────────

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

        match tool
            .call(effective_input.clone(), &ctx, parent_message, None)
            .await
        {
            Ok(result) => {
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
                            &request.input,
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
    ) -> Vec<crate::tools::background_agents::CompletedBackgroundAgent> {
        self.pending_bg_results.drain_all()
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
