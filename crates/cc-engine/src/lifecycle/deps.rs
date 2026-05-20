//! QueryEngineDeps -- QueryDeps implementation for the QueryEngine.
//!
//! Provides the query loop with access to the engine's shared state
//! (abort flag, app state, tools) and, optionally, a real `ApiClient`
//! for making Anthropic API calls.

use parking_lot::RwLock;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use uuid::Uuid;

use crate::compact::compaction::build_post_compact_messages_with_boundary;
use crate::permissions::decision::{
    AutoClassifierDecision, AutoClassifierStage, DenialTracker, PermissionDecision,
    PermissionDecisionReason,
};
use crate::tool_runtime::execution::{
    find_tool, is_plan_mode_plan_file_write, sandbox_allowed_command_applies, security_validate,
    ToolExecutionResult,
};
use crate::types::app_state::AppState;
use crate::types::message::{Message, StreamEvent};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{
    PermissionMode, PermissionRequestPayload, ToolProgress, Tools, ValidationResult,
};
use cc_engine::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};

use super::helpers::{build_messages_request, format_conversation_for_summary};
use super::{AutoClassifierFn, QueryEngineState};

/// Dependency injection bridge: provides the query loop with access to the
/// engine's shared state (abort flag, app state, tools) and, optionally, a
/// real `ApiClient` for making Anthropic API calls.
pub(crate) struct QueryEngineDeps {
    pub(crate) aborted: Arc<AtomicBool>,
    pub(crate) state: Arc<RwLock<QueryEngineState>>,
    pub(crate) cwd: String,
    pub(crate) session_id: String,
    /// Audit context for this submit — carries correlation IDs.
    pub(crate) audit_ctx: crate::observability::AuditContext,
    pub(crate) langfuse_trace: Option<crate::services::langfuse::LangfuseTrace>,
    /// When `Some`, the deps will use this client for `call_model` /
    /// `call_model_streaming`. When `None`, those methods bail with a
    /// descriptive error.
    pub(crate) api_client: Option<Arc<cc_api::api::client::ApiClient>>,
    /// Sub-agent context -- propagated into `ToolUseContext` so that
    /// nested Agent tool calls can enforce recursion depth limits.
    pub(crate) agent_context: Option<crate::types::config::AgentContext>,
    /// Async callback for interactive permission prompts.
    /// Propagated into `ToolUseContext` for headless/TUI permission flow.
    pub(crate) permission_callback: Option<crate::types::tool::PermissionCallback>,
    /// Background agent sender — forwarded into ToolUseContext.
    pub(crate) bg_agent_tx: Option<cc_types::agent_channel::AgentSender>,
    /// Optional callback that receives every `ToolProgress` emitted by a
    /// tool. The query loop pulls this via `tool_progress_callback()` and
    /// hands it to `execute_tool_calls`. Tests can leave it unset; in
    /// headless mode the engine installs a closure that forwards each
    /// progress event to `FrontendSink`.
    pub(crate) tool_progress_callback: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    /// Shared buffer of completed background agents.
    pub(crate) pending_bg_results: crate::agent_runtime::PendingBackgroundResults,
    /// Hook runner — used via the `HookRunner` trait from `cc-types::hooks` so
    /// the engine has no direct dependency on the concrete shell-hook runner.
    pub(crate) hook_runner: Arc<dyn cc_types::hooks::HookRunner>,
    /// Command dispatcher — forwarded into `ToolUseContext` for tools that
    /// spawn child engines (e.g. Agent).
    pub(crate) command_dispatcher: Arc<dyn cc_types::commands::CommandDispatcher>,
    /// Async callback for computing auto-mode classifier decisions.
    /// Called with (tool_name, tool_input, classifier_input, messages, cwd)
    /// when mode is Auto.
    /// Returns `None` if the classifier is unavailable or skipped.
    pub(crate) auto_classifier_fn: Option<AutoClassifierFn>,
}

fn auto_compact_trigger_tracking(tracking: Option<&AutoCompactTracking>) -> AutoCompactTracking {
    let base = tracking.cloned().unwrap_or(AutoCompactTracking {
        compacted: false,
        turn_counter: 0,
        turn_id: String::new(),
        consecutive_failures: 0,
    });

    AutoCompactTracking {
        compacted: true,
        turn_counter: base.turn_counter + 1,
        turn_id: base.turn_id,
        consecutive_failures: base.consecutive_failures,
    }
}

fn exact_auto_compact_triggered(
    heuristic_triggered: bool,
    exact_report: Option<&cc_utils::tokens::TokenUsageReport>,
) -> bool {
    exact_report.map_or(heuristic_triggered, |report| report.over_threshold)
}

fn central_permission_decision_for_tool(
    tool_name: &str,
    input: &serde_json::Value,
    app_state: &AppState,
    hook_decision: Option<&crate::permissions::decision::HookPermissionDecision>,
    auto_classifier: Option<&AutoClassifierDecision>,
    denial_tracker: Option<&mut DenialTracker>,
) -> PermissionDecision {
    use crate::permissions::decision::{self, PermissionBehavior};

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
        decision::has_permissions_to_use_tool_with_hook_and_auto_classifier(
            tool_name,
            input,
            &app_state.tool_permission_context,
            hook_decision,
            auto_classifier,
            denial_tracker,
        )
    };
    if matches!(&decision.behavior, PermissionBehavior::Ask)
        && matches!(&decision.reason, PermissionDecisionReason::Mode { .. })
        && app_state.tool_permission_context.mode != PermissionMode::Plan
        && sandbox_allowed_command_applies(tool_name, input, &app_state.to_tool_app_state())
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
    decision
}

fn auto_classifier_needed(decision: &PermissionDecision) -> bool {
    matches!(
        (&decision.behavior, &decision.reason),
        (
            crate::permissions::decision::PermissionBehavior::Allow,
            PermissionDecisionReason::Mode { mode }
        ) if mode == "auto"
    )
}

fn permission_result_from_decision(
    tool_name: &str,
    input: &mut serde_json::Value,
    decision: PermissionDecision,
) -> crate::types::tool::PermissionResult {
    use crate::permissions::decision::PermissionBehavior;

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

#[cfg(test)]
fn central_permission_result_for_tool(
    tool_name: &str,
    input: &mut serde_json::Value,
    app_state: &AppState,
    hook_decision: Option<&crate::permissions::decision::HookPermissionDecision>,
    auto_classifier: Option<&AutoClassifierDecision>,
) -> crate::types::tool::PermissionResult {
    let decision = central_permission_decision_for_tool(
        tool_name,
        input,
        app_state,
        hook_decision,
        auto_classifier,
        None,
    );
    permission_result_from_decision(tool_name, input, decision)
}

fn hook_error_is_critical(
    tool_name: &str,
    hook_configs: &[cc_types::hooks::HookEventConfig],
) -> bool {
    hook_configs.iter().any(|config| {
        config.critical
            && match config.matcher.as_deref() {
                None | Some("*") => true,
                Some(pattern) => tool_name == pattern || tool_name.starts_with(pattern),
            }
    })
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
        hook_stopped_continuation: result.hook_stopped_continuation,
    }
}

fn merge_refreshed_mcp_tools(existing_tools: Tools, refreshed_mcp_tools: Tools) -> Tools {
    let mut seen = HashSet::new();
    let mut merged = Vec::with_capacity(existing_tools.len() + refreshed_mcp_tools.len());

    for tool in existing_tools {
        if tool.mcp_server_name().is_some() {
            continue;
        }
        if seen.insert(tool.name().to_string()) {
            merged.push(tool);
        }
    }

    for tool in refreshed_mcp_tools {
        if seen.insert(tool.name().to_string()) {
            merged.push(tool);
        }
    }

    merged
}

fn prepare_model_call_params_for_client(
    params: &mut ModelCallParams,
    app_model: &str,
    client: &cc_api::api::client::ApiClient,
) {
    if params.model.as_deref().unwrap_or_default().is_empty() {
        params.model = Some(if app_model.is_empty() {
            client.config().default_model.clone()
        } else {
            app_model.to_string()
        });
    }

    if !cc_api::api::client::provider_supports_advisor(&client.config().provider)
        && params.advisor_model.is_some()
    {
        tracing::debug!(
            provider = client.langfuse_provider_name(),
            "dropping advisor_model - provider does not support it"
        );
        params.advisor_model = None;
    }
}

fn model_for_autocompact(
    params: &mut ModelCallParams,
    app_model: &str,
    client: Option<&cc_api::api::client::ApiClient>,
) -> String {
    if let Some(client) = client {
        prepare_model_call_params_for_client(params, app_model, client);
        return params
            .model
            .clone()
            .unwrap_or_else(|| client.config().default_model.clone());
    }

    let model = if let Some(model) = params.model.as_deref().filter(|model| !model.is_empty()) {
        model.to_string()
    } else if app_model.is_empty() {
        cc_models::default_fallback_model_id()
    } else {
        app_model.to_string()
    };
    params.model = Some(model.clone());
    model
}

fn build_auto_compact_exact_count_request(
    base_params: &ModelCallParams,
    messages: Vec<Message>,
    model: &str,
) -> cc_api::api::client::MessagesRequest {
    let mut count_params = base_params.clone();
    count_params.messages = messages;
    count_params.model = Some(model.to_string());
    count_params.skip_cache_write = Some(true);
    build_messages_request(&count_params)
}

fn record_request_snapshot(
    session_id: &str,
    provider: &str,
    request: &cc_api::api::client::MessagesRequest,
) {
    let value = match serde_json::to_value(request) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to serialize API request snapshot");
            return;
        }
    };
    if let Err(error) =
        cc_session::request_snapshot::record_api_request_snapshot(session_id, provider, &value)
    {
        tracing::warn!(session_id, %provider, %error, "failed to record API request snapshot");
    }
}

/// Tools that are always allowed in Auto mode without classifier classification.
const AUTO_MODE_ALLOWLISTED_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "LSP",
    "Sleep",
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "Plan",
    "WebSearch",
    "WebFetch",
];

impl QueryEngineDeps {
    /// Compute an auto-mode classifier decision if the classifier is configured
    /// and the mode is Auto. Returns `None` for non-Auto modes or when no
    /// classifier callback is installed.
    async fn compute_auto_classifier(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        tool_classifier_input: &serde_json::Value,
    ) -> Option<AutoClassifierDecision> {
        let fn_ref = self.auto_classifier_fn.as_ref()?;

        // Scope the lock guard so it's dropped before the async call.
        let (cwd, messages) = {
            let state = self.state.read();
            if state.app_state.tool_permission_context.mode != PermissionMode::Auto {
                return None;
            }
            // Allowlisted tools bypass the classifier entirely in Auto mode.
            if AUTO_MODE_ALLOWLISTED_TOOLS.contains(&tool_name) {
                return Some(AutoClassifierDecision::allow(
                    "allowlisted-tool",
                    AutoClassifierStage::Fast,
                ));
            }
            (self.cwd.clone(), state.messages.clone())
        };

        fn_ref(
            tool_name.to_string(),
            tool_input.clone(),
            tool_classifier_input.clone(),
            messages,
            cwd,
        )
        .await
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

        let app_model = self.state.read().app_state.main_loop_model.clone();
        prepare_model_call_params_for_client(&mut params, &app_model, client);

        // Strip advisor_model for providers that don't support it (issue #33).
        if !cc_api::api::client::provider_supports_advisor(&client.config().provider)
            && params.advisor_model.is_some()
        {
            tracing::debug!(
                provider = client.langfuse_provider_name(),
                "dropping advisor_model — provider does not support it"
            );
            params.advisor_model = None;
        }

        let request = build_messages_request(&params);
        record_request_snapshot(&self.session_id, client.langfuse_provider_name(), &request);
        let stream = client.messages_stream(request).await?;
        let mut stream = std::pin::pin!(stream);

        let mut accumulator = cc_api::api::streaming::StreamAccumulator::new();
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

        let app_model = self.state.read().app_state.main_loop_model.clone();
        prepare_model_call_params_for_client(&mut params, &app_model, client);

        // Strip advisor_model for providers that don't support it (issue #33).
        if !cc_api::api::client::provider_supports_advisor(&client.config().provider)
            && params.advisor_model.is_some()
        {
            tracing::debug!(
                provider = client.langfuse_provider_name(),
                "dropping advisor_model — provider does not support it"
            );
            params.advisor_model = None;
        }

        let request = build_messages_request(&params);
        record_request_snapshot(&self.session_id, client.langfuse_provider_name(), &request);
        if cc_api::api::client::is_env_truthy("CC_RUST_EXACT_TOKEN_DIAGNOSTICS")
            && client.supports_exact_token_count()
        {
            match client.count_token_usage_exact(&request).await {
                Ok(report) => {
                    tracing::debug!(
                        provider = report.provider.as_deref().unwrap_or("unknown"),
                        input_tokens = report.estimated_tokens,
                        context_window = report.context_window,
                        threshold_tokens = report.threshold_tokens,
                        over_threshold = report.over_threshold,
                        "provider exact token diagnostics"
                    );
                }
                Err(error) => {
                    tracing::debug!(
                        %error,
                        "provider exact token diagnostics unavailable; continuing with request"
                    );
                }
            }
        }
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
        mut params: ModelCallParams,
        tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        let original_messages = params.messages.clone();
        let app_model = self.state.read().app_state.main_loop_model.clone();
        let model = model_for_autocompact(
            &mut params,
            &app_model,
            self.api_client.as_ref().map(|client| client.as_ref()),
        );

        // Run the local context pipeline (budget -> snip -> microcompact -> auto-compact check)
        let pipeline_result = crate::compact::pipeline::run_context_pipeline(
            original_messages.clone(),
            tracking.clone(),
            &model,
        )
        .await;

        let mut auto_compact_triggered = pipeline_result.auto_compact_triggered;
        let mut auto_compact_tracking = pipeline_result.tracking.clone();

        if crate::compact::auto_compact::should_check_exact_for_auto_compact(
            pipeline_result.auto_compact_estimated_tokens,
            &model,
        ) {
            if let Some(client) = self
                .api_client
                .as_ref()
                .filter(|client| client.supports_exact_token_count())
            {
                let count_request = build_auto_compact_exact_count_request(
                    &params,
                    pipeline_result.messages.clone(),
                    &model,
                );
                match client.count_token_usage_exact(&count_request).await {
                    Ok(report) => {
                        let exact_triggered =
                            exact_auto_compact_triggered(auto_compact_triggered, Some(&report));
                        tracing::debug!(
                            provider = report.provider.as_deref().unwrap_or("unknown"),
                            exact_tokens = report.estimated_tokens,
                            threshold_tokens = report.threshold_tokens,
                            heuristic_tokens = pipeline_result.auto_compact_estimated_tokens,
                            heuristic_triggered = auto_compact_triggered,
                            exact_triggered = exact_triggered,
                            "auto-compact exact threshold check"
                        );
                        auto_compact_triggered = exact_triggered;
                        auto_compact_tracking = if auto_compact_triggered {
                            Some(auto_compact_trigger_tracking(tracking.as_ref()))
                        } else {
                            tracking.clone()
                        };
                    }
                    Err(error) => {
                        tracing::debug!(
                            %error,
                            heuristic_tokens = pipeline_result.auto_compact_estimated_tokens,
                            "auto-compact exact threshold check unavailable; keeping heuristic decision"
                        );
                    }
                }
            }
        }

        // If auto-compact was triggered AND we have an API client, generate a model summary
        if auto_compact_triggered {
            let updated_tracking = auto_compact_tracking
                .clone()
                .unwrap_or_else(|| auto_compact_trigger_tracking(tracking.as_ref()));
            let session_memory_context = {
                let state = self.state.read();
                state
                    .session_memory
                    .format_memory_context_for_workspace_excluding_session(
                        5,
                        Some(std::path::Path::new(&self.cwd)),
                        Some(self.audit_ctx.session_id.as_str()),
                    )
            };
            if let Some(context) = session_memory_context.as_deref() {
                if let Some(session_memory_result) =
                    crate::compact::session_memory_compact::session_memory_compact_if_needed(
                        pipeline_result.messages.clone(),
                        context,
                    )
                {
                    tracing::info!(
                        tokens_freed = session_memory_result.tokens_freed,
                        kept_start_index = session_memory_result.kept_start_index,
                        "autocompact: session-memory summary complete"
                    );
                    let new_tracking = crate::compact::compaction::tracking_on_success(
                        tracking.as_ref(),
                        &Uuid::new_v4().to_string(),
                    );
                    return Ok(Some(CompactionResult {
                        messages: session_memory_result.messages,
                        tracking: new_tracking,
                    }));
                }
            }

            // Try model-based summarization if API client is available
            if let Some(ref _client) = self.api_client {
                let summary_prompt = crate::compact::compaction::build_compaction_prompt();
                let pre_tokens = crate::utils::tokens::estimate_messages_tokens(&original_messages);

                // Build a summarization request
                let summary_messages = vec![Message::User(crate::types::message::UserMessage {
                    uuid: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    role: "user".into(),
                    content: crate::types::message::MessageContent::Text(
                        format_conversation_for_summary(&original_messages),
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

                        let post_messages = build_post_compact_messages_with_boundary(
                            &summary_text,
                            &original_messages,
                            &config,
                            pre_tokens,
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
                tracking: updated_tracking,
            }));
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
                    .unwrap_or_else(cc_models::default_fallback_model_id)
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
                    .unwrap_or_else(cc_models::default_fallback_model_id)
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
    // boundary declared in `QueryDeps`. Main-loop and future stream-time
    // scheduling must stay routed here so permission, hook, progress, audit,
    // security, and result handling remain single-sourced.
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
        // concrete shell-hook runner (see issue #74, Phase 5b).
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
                Arc::new(move || state.read().app_state.to_tool_app_state())
            },
            set_app_state: {
                let state = self.state.clone();
                Arc::new(move |updater: crate::types::tool::AppStateUpdater| {
                    let mut s = state.write();
                    let old = s.app_state.to_tool_app_state();
                    let updated = updater(old);
                    s.app_state.apply_tool_app_state(updated);
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
                    hook_stopped_continuation: false,
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
                    hook_stopped_continuation: false,
                });
            }
            Err(e) => {
                if hook_error_is_critical(&request.tool_name, &pre_configs) {
                    tracing::warn!(error = %e, tool = %request.tool_name, "critical pre-tool hook error, blocking tool execution");
                    return Ok(ToolExecResult {
                        tool_use_id: request.tool_use_id,
                        tool_name: request.tool_name,
                        result: crate::types::tool::ToolResult {
                            data: serde_json::json!(format!(
                                "Critical pre-tool hook failed: {}",
                                e
                            )),
                            new_messages: vec![],
                            ..Default::default()
                        },
                        is_error: true,
                        hook_stopped_continuation: false,
                    });
                } else {
                    tracing::warn!(error = %e, "optional pre-tool hook error, continuing");
                    (sanitized_input.clone(), None)
                }
            }
        };

        if effective_input != sanitized_input {
            match tool.validate_input(&effective_input, &ctx).await {
                ValidationResult::Ok => {}
                ValidationResult::Error { message, .. } => {
                    return Ok(ToolExecResult {
                        tool_use_id: request.tool_use_id,
                        tool_name: request.tool_name,
                        result: crate::types::tool::ToolResult {
                            data: serde_json::json!(format!(
                                "Pre-tool hook produced invalid input: {}.",
                                message
                            )),
                            new_messages: vec![],
                            ..Default::default()
                        },
                        is_error: true,
                        hook_stopped_continuation: false,
                    });
                }
            }

            if let Some(result) = security_validate(
                &request.tool_use_id,
                &request.tool_name,
                &effective_input,
                tool.as_ref(),
                &ctx,
                execution_started,
            ) {
                return Ok(tool_execution_result_to_exec_result(result));
            }
        }

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
                hook_stopped_continuation: false,
            });
        }

        {
            // Normal permission check via tool-local checks and the central rule engine
            let perm_audit_ctx = self.audit_ctx.with_tool_use(&request.tool_use_id);
            let perm_result = match tool.check_permissions(&effective_input, &ctx).await {
                PermissionResult::Allow { updated_input } => {
                    effective_input = updated_input;
                    let app_state = self.state.read().app_state.clone();
                    let mut decision = central_permission_decision_for_tool(
                        &request.tool_name,
                        &effective_input,
                        &app_state,
                        hook_decision.as_ref(),
                        None,
                        None,
                    );

                    if auto_classifier_needed(&decision) {
                        let mut classifier_input = tool.to_auto_classifier_input(&effective_input);
                        if matches!(&classifier_input, serde_json::Value::String(s) if s.is_empty())
                        {
                            classifier_input = effective_input.clone();
                        }
                        if self
                            .state
                            .read()
                            .auto_denial_tracker
                            .should_fallback_to_interactive()
                        {
                            let mut state = self.state.write();
                            let app_state = state.app_state.clone();
                            decision = central_permission_decision_for_tool(
                                &request.tool_name,
                                &effective_input,
                                &app_state,
                                hook_decision.as_ref(),
                                None,
                                Some(&mut state.auto_denial_tracker),
                            );
                        } else if let Some(auto_classifier) = self
                            .compute_auto_classifier(
                                &request.tool_name,
                                &effective_input,
                                &classifier_input,
                            )
                            .await
                        {
                            let mut state = self.state.write();
                            let app_state = state.app_state.clone();
                            decision = central_permission_decision_for_tool(
                                &request.tool_name,
                                &effective_input,
                                &app_state,
                                hook_decision.as_ref(),
                                Some(&auto_classifier),
                                Some(&mut state.auto_denial_tracker),
                            );
                        }
                    }

                    permission_result_from_decision(
                        &request.tool_name,
                        &mut effective_input,
                        decision,
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
                        hook_stopped_continuation: false,
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
                                            hook_stopped_continuation: false,
                                        });
                                    }
                                    _ => {} // unknown decision, continue with normal prompt
                                }
                            }
                        }
                    }

                    if !hook_allowed {
                        if let Some(ref callback) = ctx.permission_callback {
                            let options = vec![
                                "Allow".to_string(),
                                "Deny".to_string(),
                                "Always Allow".to_string(),
                            ];
                            let decision = callback(PermissionRequestPayload {
                                tool_use_id: request.tool_use_id.clone(),
                                tool_name: request.tool_name.clone(),
                                tool_input: effective_input.clone(),
                                message,
                                options,
                            })
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
                                        hook_stopped_continuation: false,
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
                                hook_stopped_continuation: false,
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
                let mut hook_stopped_continuation = false;
                if !post_configs.is_empty() {
                    match hooks
                        .run_post_tool_hooks(
                            &request.tool_name,
                            &effective_input,
                            &result.data,
                            &post_configs,
                        )
                        .await
                    {
                        Ok(PostToolHookResult::Continue) => {}
                        Ok(PostToolHookResult::StopContinuation { message }) => {
                            tracing::debug!(
                                message = %message,
                                "post-tool hook stopped continuation"
                            );
                            hook_stopped_continuation = true;
                        }
                        Err(e) if hook_error_is_critical(&request.tool_name, &post_configs) => {
                            tracing::warn!(error = %e, tool = %request.tool_name, "critical post-tool hook error, failing tool execution");
                            return Ok(ToolExecResult {
                                tool_use_id: request.tool_use_id,
                                tool_name: request.tool_name,
                                result: crate::types::tool::ToolResult {
                                    data: serde_json::json!(format!(
                                        "Critical post-tool hook failed: {}",
                                        e
                                    )),
                                    new_messages: vec![],
                                    ..Default::default()
                                },
                                is_error: true,
                                hook_stopped_continuation: false,
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, tool = %request.tool_name, "optional post-tool hook error, continuing");
                        }
                    }
                }

                result.data = cc_tools::result::enforce_result_size(
                    result.data,
                    tool.max_result_size_chars(),
                );

                Ok(ToolExecResult {
                    tool_use_id: request.tool_use_id,
                    tool_name: request.tool_name,
                    result,
                    is_error: false,
                    hook_stopped_continuation,
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
                    match hooks
                        .run_post_tool_failure_hooks(
                            &request.tool_name,
                            &effective_input,
                            &e.to_string(),
                            &failure_configs,
                        )
                        .await
                    {
                        Ok(()) => {}
                        Err(hook_error)
                            if hook_error_is_critical(&request.tool_name, &failure_configs) =>
                        {
                            tracing::warn!(error = %hook_error, tool = %request.tool_name, "critical post-failure hook error, failing tool execution");
                            return Ok(ToolExecResult {
                                tool_use_id: request.tool_use_id,
                                tool_name: request.tool_name,
                                result: crate::types::tool::ToolResult {
                                    data: serde_json::json!(format!(
                                        "Critical post-failure hook failed after tool error ({}): {}",
                                        e, hook_error
                                    )),
                                    new_messages: vec![],
                                    ..Default::default()
                                },
                                is_error: true,
                                hook_stopped_continuation: false,
                            });
                        }
                        Err(hook_error) => {
                            tracing::warn!(error = %hook_error, tool = %request.tool_name, "optional post-failure hook error, continuing");
                        }
                    }
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
                    hook_stopped_continuation: false,
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
        let Some(manager) = cc_mcp::runtime::current_manager() else {
            return Ok(self.state.read().tools.clone());
        };

        let mcp_tool_defs = {
            let manager_guard = manager.lock().await;
            manager_guard.all_tools()
        };
        let mcp_tools = crate::mcp_tool_adapter::mcp_tools_to_tools(mcp_tool_defs, manager);

        let refreshed = {
            let mut state = self.state.write();
            let refreshed = merge_refreshed_mcp_tools(state.tools.clone(), mcp_tools);
            state.tools = refreshed.clone();
            refreshed
        };

        crate::tool_runtime::tool_search::install_runtime_tool_catalog(&refreshed);
        Ok(refreshed)
    }

    fn drain_background_results(&self) -> Vec<crate::agent_runtime::CompletedBackgroundAgent> {
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
    use crate::lifecycle::QueryEngine;
    use crate::types::config::QueryEngineConfig;
    use crate::types::message::{AssistantMessage, MessageContent, ToolResultContent, UserMessage};
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

    struct FailingTool {
        name: &'static str,
        seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
    }

    #[async_trait::async_trait]
    impl Tool for FailingTool {
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
            ValidationResult::Ok
        }

        async fn check_permissions(
            &self,
            input: &Value,
            _ctx: &ToolUseContext,
        ) -> PermissionResult {
            PermissionResult::Allow {
                updated_input: input.clone(),
            }
        }

        async fn call(
            &self,
            input: Value,
            _ctx: &ToolUseContext,
            _parent_message: &AssistantMessage,
            _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> Result<ToolResult> {
            *self.seen_input.lock() = Some(input);
            Err(anyhow::anyhow!("tool failed for test"))
        }

        async fn prompt(&self) -> String {
            String::new()
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
            cwd: ".".to_string(),
            session_id: "test-session".to_string(),
            audit_ctx: crate::observability::AuditContext::noop("test"),
            langfuse_trace: None,
            api_client: None,
            agent_context: None,
            permission_callback: None,
            bg_agent_tx: None,
            tool_progress_callback: None,
            pending_bg_results: crate::agent_runtime::PendingBackgroundResults::new(),
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
            auto_classifier_fn: None,
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

    fn user_message(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text(text.to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
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

    struct FailingPreToolHookRunner {
        critical: bool,
    }

    #[async_trait::async_trait]
    impl cc_types::hooks::HookRunner for FailingPreToolHookRunner {
        fn load_hook_configs(
            &self,
            _hooks_value: &cc_types::hooks::HooksMap,
            event_name: &str,
        ) -> Vec<cc_types::hooks::HookEventConfig> {
            if event_name == "PreToolUse" {
                vec![cc_types::hooks::HookEventConfig {
                    matcher: Some("HookedTool".to_string()),
                    critical: self.critical,
                    hooks: vec![cc_types::hooks::HookEntry::Command {
                        command: "failing-test-hook".to_string(),
                        timeout: 1,
                        shell: None,
                        if_condition: None,
                    }],
                }]
            } else {
                vec![]
            }
        }

        async fn run_pre_tool_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PreToolHookResult> {
            Err(anyhow::anyhow!("pre hook failed for test"))
        }

        async fn run_post_tool_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _tool_result_data: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
            Ok(cc_types::hooks::PostToolHookResult::Continue)
        }

        async fn run_post_tool_failure_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _error: &str,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn run_event_hooks(
            &self,
            _event_name: &str,
            _payload: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::HookOutput> {
            Ok(cc_types::hooks::HookOutput::default())
        }

        async fn run_stop_hooks(
            &self,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
            Ok(cc_types::hooks::PostToolHookResult::Continue)
        }
    }

    struct FailingPostToolHookRunner {
        event_name: &'static str,
        critical: bool,
    }

    #[async_trait::async_trait]
    impl cc_types::hooks::HookRunner for FailingPostToolHookRunner {
        fn load_hook_configs(
            &self,
            _hooks_value: &cc_types::hooks::HooksMap,
            event_name: &str,
        ) -> Vec<cc_types::hooks::HookEventConfig> {
            if event_name == self.event_name {
                vec![cc_types::hooks::HookEventConfig {
                    matcher: Some("HookedTool".to_string()),
                    critical: self.critical,
                    hooks: vec![cc_types::hooks::HookEntry::Command {
                        command: "failing-test-hook".to_string(),
                        timeout: 1,
                        shell: None,
                        if_condition: None,
                    }],
                }]
            } else {
                vec![]
            }
        }

        async fn run_pre_tool_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PreToolHookResult> {
            Ok(cc_types::hooks::PreToolHookResult::Continue {
                updated_input: None,
                permission_override: None,
            })
        }

        async fn run_post_tool_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _tool_result_data: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
            if self.event_name == "PostToolUse" {
                Err(anyhow::anyhow!("post hook failed for test"))
            } else {
                Ok(cc_types::hooks::PostToolHookResult::Continue)
            }
        }

        async fn run_post_tool_failure_hooks(
            &self,
            _tool_name: &str,
            _input: &Value,
            _error: &str,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<()> {
            if self.event_name == "PostToolUseFailure" {
                Err(anyhow::anyhow!("post failure hook failed for test"))
            } else {
                Ok(())
            }
        }

        async fn run_event_hooks(
            &self,
            _event_name: &str,
            _payload: &Value,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::HookOutput> {
            Ok(cc_types::hooks::HookOutput::default())
        }

        async fn run_stop_hooks(
            &self,
            _hook_configs: &[cc_types::hooks::HookEventConfig],
        ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
            Ok(cc_types::hooks::PostToolHookResult::Continue)
        }
    }

    fn mcp_tool(server_name: &str, tool_name: &str) -> Arc<dyn Tool> {
        Arc::new(crate::mcp_tool_adapter::McpToolWrapper {
            def: cc_mcp::McpToolDef {
                name: tool_name.to_string(),
                description: format!("{server_name} tool"),
                input_schema: json!({"type": "object"}),
                server_name: server_name.to_string(),
            },
            server_name: server_name.to_string(),
            manager: Arc::new(tokio::sync::Mutex::new(cc_mcp::manager::McpManager::new())),
        })
    }

    #[test]
    fn refreshed_mcp_merge_replaces_wrapped_tools_without_prefix_guessing() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let native_mcp_named_tool: Arc<dyn Tool> =
            canonical_tool("mcp__computer-use__screenshot", seen_input);
        let stale_mcp_tool = mcp_tool("old-server", "mcp__old-server__stale");
        let fresh_mcp_tool = mcp_tool("new-server", "mcp__new-server__fresh");

        let merged = merge_refreshed_mcp_tools(
            vec![native_mcp_named_tool, stale_mcp_tool],
            vec![fresh_mcp_tool],
        );
        let names = merged.iter().map(|tool| tool.name()).collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["mcp__computer-use__screenshot", "mcp__new-server__fresh"]
        );
        assert_eq!(merged[0].mcp_server_name(), None);
        assert_eq!(merged[1].mcp_server_name(), Some("new-server"));
    }

    #[test]
    fn exact_auto_compact_trigger_keeps_heuristic_when_report_missing() {
        assert!(exact_auto_compact_triggered(true, None));
        assert!(!exact_auto_compact_triggered(false, None));
    }

    #[test]
    fn exact_auto_compact_trigger_overrides_near_threshold_heuristic() {
        let below = cc_utils::tokens::token_usage_report_from_count(
            159_000,
            "claude-sonnet-4-20250514",
            cc_utils::tokens::TokenCountMethod::ProviderExact,
            Some("test"),
        );
        let above = cc_utils::tokens::token_usage_report_from_count(
            161_000,
            "claude-sonnet-4-20250514",
            cc_utils::tokens::TokenCountMethod::ProviderExact,
            Some("test"),
        );

        assert!(!exact_auto_compact_triggered(true, Some(&below)));
        assert!(exact_auto_compact_triggered(false, Some(&above)));
    }

    #[test]
    fn auto_compact_exact_count_request_uses_final_request_boundary() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool: Arc<dyn Tool> = canonical_tool("BoundaryTool", seen_input);
        let params = ModelCallParams {
            messages: vec![user_message("pre-pipeline")],
            system_prompt: vec!["system boundary".to_string()],
            tools: vec![tool],
            model: Some("claude-sonnet-4-20250514".to_string()),
            max_output_tokens: Some(123),
            skip_cache_write: None,
            thinking_enabled: Some(true),
            effort_value: Some("low".to_string()),
            advisor_model: Some("advisor-model".to_string()),
        };

        let request = build_auto_compact_exact_count_request(
            &params,
            vec![user_message("post-pipeline")],
            "claude-sonnet-4-20250514",
        );

        assert_eq!(request.messages[0]["content"], json!("post-pipeline"));
        assert_eq!(
            request.system.as_ref().unwrap()[0]["text"],
            json!("system boundary")
        );
        assert_eq!(
            request.tools.as_ref().unwrap()[0]["name"],
            json!("BoundaryTool")
        );
        assert_eq!(request.max_tokens, 123);
        assert!(request.thinking.is_some());
        assert_eq!(request.advisor_model.as_deref(), Some("advisor-model"));
    }

    #[test]
    fn central_permission_default_mode_asks_for_bash_after_tool_allow() {
        let app_state = AppState::default();
        let mut input = json!({"command": "rm -rf F:/temp/gomoku_subagent/*"});

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None, None);

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

        let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None, None);

        assert!(matches!(result, PermissionResult::Allow { .. }));
    }

    #[test]
    fn central_permission_sandbox_allowed_command_allows_workspace_bash() {
        let mut app_state = AppState::default();
        app_state.settings.sandbox.enabled = Some(true);
        app_state.settings.sandbox.mode = Some("workspace".into());
        app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
        let mut input = json!({"command": "cargo test --all"});

        let result = crate::tool_runtime::execution::with_sandbox_availability_override(
            crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
            || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
        );

        assert!(matches!(result, PermissionResult::Allow { .. }));
    }

    #[test]
    fn central_permission_sandbox_allowed_command_asks_when_sandbox_unavailable() {
        let mut app_state = AppState::default();
        app_state.settings.sandbox.enabled = Some(true);
        app_state.settings.sandbox.mode = Some("workspace".into());
        app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
        let mut input = json!({"command": "cargo test --all"});

        let result = crate::tool_runtime::execution::with_sandbox_availability_override(
            crate::sandbox::Availability::Unavailable {
                platform: "test",
                reason: "forced unavailable".to_string(),
            },
            || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
        );

        assert!(matches!(result, PermissionResult::Ask { .. }));
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

        let result = crate::tool_runtime::execution::with_sandbox_availability_override(
            crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
            || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
        );

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

        let result = crate::tool_runtime::execution::with_sandbox_availability_override(
            crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
            || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
        );

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
            None,
        );

        assert!(matches!(result, PermissionResult::Ask { .. }));
    }

    #[tokio::test]
    async fn execute_tool_optional_pre_hook_error_continues_to_tool_call() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("HookedTool", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPreToolHookRunner { critical: false });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
    }

    #[tokio::test]
    async fn execute_tool_critical_pre_hook_error_blocks_tool_call() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("HookedTool", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPreToolHookRunner { critical: true });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.result.data.as_str().is_some_and(|text| {
            text.contains("Critical pre-tool hook failed")
                && text.contains("pre hook failed for test")
        }));
        assert!(
            seen_input.lock().is_none(),
            "critical pre-hook failure must stop before Tool::call"
        );
    }

    #[tokio::test]
    async fn execute_tool_optional_post_hook_error_keeps_tool_success() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("HookedTool", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPostToolHookRunner {
            event_name: "PostToolUse",
            critical: false,
        });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.result.data, json!("ok"));
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
    }

    #[tokio::test]
    async fn execute_tool_critical_post_hook_error_returns_failed_result() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("HookedTool", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPostToolHookRunner {
            event_name: "PostToolUse",
            critical: true,
        });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.result.data.as_str().is_some_and(|text| {
            text.contains("Critical post-tool hook failed")
                && text.contains("post hook failed for test")
        }));
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
    }

    #[tokio::test]
    async fn execute_tool_optional_post_failure_hook_error_keeps_tool_error() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool: Arc<dyn Tool> = Arc::new(FailingTool {
            name: "HookedTool",
            seen_input: seen_input.clone(),
        });
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPostToolHookRunner {
            event_name: "PostToolUseFailure",
            critical: false,
        });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert_eq!(result.result.data, json!("Error: tool failed for test"));
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
    }

    #[tokio::test]
    async fn execute_tool_critical_post_failure_hook_error_returns_hook_failure() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool: Arc<dyn Tool> = Arc::new(FailingTool {
            name: "HookedTool",
            seen_input: seen_input.clone(),
        });
        let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
        deps.hook_runner = Arc::new(FailingPostToolHookRunner {
            event_name: "PostToolUseFailure",
            critical: true,
        });

        let result = deps
            .execute_tool(
                tool_request("HookedTool", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.result.data.as_str().is_some_and(|text| {
            text.contains("Critical post-failure hook failed")
                && text.contains("tool failed for test")
                && text.contains("post failure hook failed for test")
        }));
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
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
    #[serial_test::serial]
    async fn execute_tool_allows_plan_file_write_in_plan_mode() {
        struct TestWriteTool;

        #[async_trait::async_trait]
        impl Tool for TestWriteTool {
            fn name(&self) -> &str {
                "Write"
            }

            async fn description(&self, _input: &serde_json::Value) -> String {
                "test write".to_string()
            }

            fn input_json_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }

            fn is_read_only(&self, _input: &serde_json::Value) -> bool {
                false
            }

            async fn call(
                &self,
                input: serde_json::Value,
                _ctx: &crate::types::tool::ToolUseContext,
                _parent_message: &AssistantMessage,
                _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
            ) -> anyhow::Result<crate::types::tool::ToolResult> {
                let path = input
                    .get("file_path")
                    .and_then(|value| value.as_str())
                    .unwrap();
                let content = input
                    .get("content")
                    .and_then(|value| value.as_str())
                    .unwrap();
                std::fs::write(path, content)?;
                Ok(crate::types::tool::ToolResult {
                    data: serde_json::json!("ok"),
                    new_messages: vec![],
                    ..Default::default()
                })
            }

            async fn prompt(&self) -> String {
                String::new()
            }
        }

        struct OriginalCwdGuard(std::path::PathBuf);

        impl Drop for OriginalCwdGuard {
            fn drop(&mut self) {
                crate::bootstrap::PROCESS_STATE.write().original_cwd = self.0.clone();
            }
        }

        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".cc-rust")).unwrap();
        let original_cwd = crate::bootstrap::PROCESS_STATE.read().original_cwd.clone();
        let _guard = OriginalCwdGuard(original_cwd);
        crate::bootstrap::PROCESS_STATE.write().original_cwd = temp.path().to_path_buf();

        let plan_path = crate::config::paths::current_plan_file_path(temp.path());
        let plan_path_string = plan_path.to_string_lossy().into_owned();
        let content = "## Plan\n- verify canonical plan file write";
        let tool: Arc<dyn Tool> = Arc::new(TestWriteTool);
        let deps = make_deps(vec![tool], PermissionMode::Plan);

        let result = deps
            .execute_tool(
                tool_request(
                    "Write",
                    json!({
                        "file_path": plan_path_string,
                        "content": content,
                    }),
                ),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(
            !result.is_error,
            "canonical plan file write should not require a prompt: {:?}",
            result.result.data
        );
        assert_eq!(std::fs::read_to_string(plan_path).unwrap(), content);
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
    async fn execute_tool_auto_mode_does_not_classify_when_rule_denies() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("DenyMe", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Auto);
        deps.state
            .write()
            .app_state
            .tool_permission_context
            .always_deny_rules
            .insert("test".to_string(), vec!["DenyMe".to_string()]);

        let classifier_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        deps.auto_classifier_fn = Some(Arc::new({
            let classifier_calls = classifier_calls.clone();
            move |_, _, _, _, _| {
                classifier_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async {
                    Some(AutoClassifierDecision::allow(
                        "test-classifier",
                        AutoClassifierStage::Fast,
                    ))
                })
            }
        }));

        let result = deps
            .execute_tool(
                tool_request("DenyMe", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert_eq!(classifier_calls.load(Ordering::SeqCst), 0);
        assert!(
            seen_input.lock().is_none(),
            "central deny rule must stop before Tool::call"
        );
    }

    #[tokio::test]
    async fn execute_tool_auto_classifier_denials_fall_back_to_prompt() {
        let seen_input = Arc::new(parking_lot::Mutex::new(None));
        let tool = canonical_tool("NeedsClassifier", seen_input.clone());
        let mut deps = make_deps(vec![tool], PermissionMode::Auto);
        let classifier_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        deps.auto_classifier_fn = Some(Arc::new({
            let classifier_calls = classifier_calls.clone();
            move |_, _, _, _, _| {
                classifier_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async {
                    Some(AutoClassifierDecision::deny(
                        "test-classifier",
                        AutoClassifierStage::Thinking,
                        "blocked by classifier",
                    ))
                })
            }
        }));
        let callback: PermissionCallback = Arc::new(|_| Box::pin(async { "allow".to_string() }));
        deps.permission_callback = Some(callback);

        for _ in 0..2 {
            let result = deps
                .execute_tool(
                    tool_request("NeedsClassifier", json!({"value": true})),
                    &deps.get_tools(),
                    &parent_message(),
                    None,
                )
                .await
                .unwrap();
            assert!(result.is_error);
        }
        assert!(
            seen_input.lock().is_none(),
            "classifier denials must stop before Tool::call"
        );

        let result = deps
            .execute_tool(
                tool_request("NeedsClassifier", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(classifier_calls.load(Ordering::SeqCst), 3);
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));

        *seen_input.lock() = None;
        let result = deps
            .execute_tool(
                tool_request("NeedsClassifier", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            classifier_calls.load(Ordering::SeqCst),
            3,
            "interactive fallback should avoid further classifier calls"
        );
        assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
    }

    #[tokio::test]
    async fn execute_tool_auto_classifier_uses_tool_specific_input() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("classifier-input.txt");
        let path_string = path.to_string_lossy().into_owned();
        let captured = Arc::new(parking_lot::Mutex::new(None::<(Value, Value)>));
        let mut deps = make_deps(
            vec![Arc::new(cc_tools::fs::file_write::FileWriteTool::new())],
            PermissionMode::Auto,
        );
        deps.auto_classifier_fn = Some(Arc::new({
            let captured = captured.clone();
            move |_, raw_input, classifier_input, _, _| {
                *captured.lock() = Some((raw_input, classifier_input));
                Box::pin(async {
                    Some(AutoClassifierDecision::allow(
                        "test-classifier",
                        AutoClassifierStage::Fast,
                    ))
                })
            }
        }));

        let result = deps
            .execute_tool(
                tool_request(
                    "Write",
                    json!({
                        "file_path": path_string,
                        "content": "secret body that should not be in classifier_input",
                    }),
                ),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let (raw_input, classifier_input) = captured.lock().clone().expect("classifier called");
        assert_eq!(
            raw_input.get("content").and_then(Value::as_str),
            Some("secret body that should not be in classifier_input")
        );
        assert_eq!(
            classifier_input.get("file_path"),
            raw_input.get("file_path")
        );
        assert_eq!(classifier_input.get("content"), None);
        assert_eq!(
            classifier_input.get("operation"),
            Some(&json!("write_file"))
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
        let callback: PermissionCallback = Arc::new(|_| Box::pin(async { "allow".to_string() }));
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
