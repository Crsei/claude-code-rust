//! Main tool execution pipeline.

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;
use tracing::{debug, warn};

use crate::permissions::decision::{
    self, HookPermissionDecision, PermissionBehavior, PermissionDecision,
};
use crate::tools::hooks::{
    self, HookEventConfig, PermissionOverride, PostToolHookResult, PreToolHookResult,
};

/// Convert the legacy [`PermissionOverride`] (carried back by
/// `run_pre_tool_hooks`) into the richer [`HookPermissionDecision`] the
/// central decision flow expects.
fn build_hook_decision(o: Option<&PermissionOverride>) -> Option<HookPermissionDecision> {
    let o = o?;
    let mut h = HookPermissionDecision::default();
    h.source = Some("PreToolUse".into());
    match o {
        PermissionOverride::Allow => h.allow = true,
        PermissionOverride::Deny { reason } => h.deny = Some(reason.clone()),
    }
    Some(h)
}
use crate::types::message::AssistantMessage;
use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, Tools};

use super::security::{enforce_result_size, find_tool, security_validate};
use super::{make_error_result, ToolExecutionResult};

/// Execute a single tool call through the full pipeline.
///
/// Corresponds to TypeScript: `runToolUse()` in toolExecution.ts
///
/// This is the central function that takes a tool_use block from the
/// assistant's response and runs it through validation, hooks, permissions,
/// execution, and result processing.
pub async fn run_tool_use(
    tool_use_id: &str,
    tool_name: &str,
    input: Value,
    tools: &Tools,
    ctx: &ToolUseContext,
    parent_message: &AssistantMessage,
    on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    hook_configs: &[HookEventConfig],
) -> ToolExecutionResult {
    let started = Instant::now();

    // ── Stage 1: Tool lookup ────────────────────────────────────────
    let tool = match find_tool(tool_name, tools) {
        Some(t) => t,
        None => {
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!(
                    "No tool named '{}' is available. Available tools: {}",
                    tool_name,
                    tools
                        .iter()
                        .map(|t| t.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                started,
            );
        }
    };

    // ── Stage 2: Abort check ────────────────────────────────────────
    if *ctx.abort_signal.borrow() {
        return make_error_result(
            tool_use_id,
            tool_name,
            "Tool execution cancelled (abort signal received).",
            started,
        );
    }

    // ── Stage 3a: Input schema validation ───────────────────────────
    let validation = tool.validate_input(&input, ctx).await;
    match validation {
        crate::types::tool::ValidationResult::Error {
            message,
            error_code,
        } => {
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!(
                    "Input validation error: {}. The schema was not sent — please check the tool's input requirements.",
                    message
                ),
                started,
            );
        }
        crate::types::tool::ValidationResult::Ok => {}
    }

    // ── Stage 3b: Input sanitization ────────────────────────────────
    // Strip any _simulatedSedEdit field (defense-in-depth)
    let mut sanitized_input = input.clone();
    if let Some(obj) = sanitized_input.as_object_mut() {
        obj.remove("_simulatedSedEdit");
    }

    // ── Stage 3c: Security validation ──────────────────────────────
    if let Some(err_result) = security_validate(
        tool_use_id,
        tool_name,
        &sanitized_input,
        tool.as_ref(),
        ctx,
        started,
    ) {
        return err_result;
    }

    // ── Stage 4: Pre-tool hooks ─────────────────────────────────────
    let hook_start = Instant::now();
    let (effective_input, permission_override) =
        match hooks::run_pre_tool_hooks(tool_name, &sanitized_input, hook_configs).await {
            Ok(PreToolHookResult::Continue {
                updated_input,
                permission_override,
            }) => (
                updated_input.unwrap_or(sanitized_input),
                permission_override,
            ),
            Ok(PreToolHookResult::Stop { message }) => {
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Pre-tool hook stopped execution: {}", message),
                    started,
                );
            }
            Err(e) => {
                warn!(tool = tool_name, error = %e, "pre-tool hook error, continuing");
                (sanitized_input, None)
            }
        };

    let hook_duration = hook_start.elapsed();
    if hook_duration.as_millis() > 500 {
        debug!(
            tool = tool_name,
            duration_ms = hook_duration.as_millis(),
            "pre-tool hooks took >500ms"
        );
    }

    // ── Stage 5: Permission check ───────────────────────────────────
    // Build a `HookPermissionDecision` from the pre-tool hook output and
    // route everything (rules + hook + mode) through the central
    // decision flow so deny / ask still beat hook allow per spec.
    let hook_decision = build_hook_decision(permission_override.as_ref());

    // The tool may still expose its own per-tool checks (e.g. dangerous
    // command detection in Bash). Run them first so tool-local deny/ask
    // decisions cannot be bypassed by hook overrides, broad allow rules,
    // or permissive modes.
    match tool.check_permissions(&effective_input, ctx).await {
        crate::types::tool::PermissionResult::Deny { message } => {
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!("Permission denied: {}", message),
                started,
            );
        }
        crate::types::tool::PermissionResult::Ask { message } => {
            if ctx.options.is_non_interactive_session {
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Permission required (non-interactive mode): {}", message),
                    started,
                );
            }
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!("Permission required: {}", message),
                started,
            );
        }
        crate::types::tool::PermissionResult::Allow { .. } => {
            // Continue to the central flow.
        }
    }

    let app_state = (ctx.get_app_state)();
    let central_decision = decision::has_permissions_to_use_tool_with_hook(
        tool_name,
        &effective_input,
        &app_state.tool_permission_context,
        hook_decision.as_ref(),
        None,
    );

    let effective_input = central_decision
        .updated_input
        .clone()
        .unwrap_or(effective_input);

    match central_decision.behavior {
        PermissionBehavior::Allow => {
            debug!(
                tool = tool_name,
                reason = ?central_decision.reason,
                "permission allowed"
            );
        }
        PermissionBehavior::Deny => {
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!(
                    "Permission denied: {}",
                    central_decision.message.unwrap_or_else(|| "policy".into())
                ),
                started,
            );
        }
        PermissionBehavior::Ask => {
            let message = central_decision
                .message
                .unwrap_or_else(|| format!("Allow tool '{}'?", tool_name));
            if ctx.options.is_non_interactive_session {
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Permission required (non-interactive mode): {}", message),
                    started,
                );
            }
            return make_error_result(
                tool_use_id,
                tool_name,
                &format!("Permission required: {}", message),
                started,
            );
        }
    }

    // ── Stage 6: Tool execution ─────────────────────────────────────
    // Check abort again before the potentially long-running call
    if *ctx.abort_signal.borrow() {
        return make_error_result(
            tool_use_id,
            tool_name,
            "Tool execution cancelled before call.",
            started,
        );
    }

    debug!(tool = tool_name, "tool call starting");
    let call_result = tool
        .call(
            effective_input.clone(),
            ctx,
            parent_message,
            on_progress.map(|f| {
                Box::new(move |p: ToolProgress| f(p)) as Box<dyn Fn(ToolProgress) + Send + Sync>
            }),
        )
        .await;

    // ── Stage 7: Post-tool hooks ────────────────────────────────────
    let mut hook_stopped_continuation = false;

    match &call_result {
        Ok(result) => {
            match hooks::run_post_tool_hooks(tool_name, &effective_input, result, hook_configs)
                .await
            {
                Ok(PostToolHookResult::StopContinuation { message }) => {
                    hook_stopped_continuation = true;
                    debug!(
                        tool = tool_name,
                        message = %message,
                        "post-tool hook stopped continuation"
                    );
                }
                Ok(PostToolHookResult::Continue) => {}
                Err(e) => {
                    warn!(tool = tool_name, error = %e, "post-tool hook error");
                }
            }
        }
        Err(e) => {
            let _ = hooks::run_post_tool_failure_hooks(
                tool_name,
                &effective_input,
                &e.to_string(),
                hook_configs,
            )
            .await;
        }
    }

    // ── Stage 8: Result assembly ────────────────────────────────────
    let duration_ms = started.elapsed().as_millis() as u64;

    // Record tool duration in global ProcessState
    crate::bootstrap::PROCESS_STATE
        .read()
        .tool_duration
        .record(duration_ms);

    match call_result {
        Ok(tool_result) => {
            debug!(
                tool = tool_name,
                duration_ms = duration_ms,
                "tool call succeeded"
            );
            let new_messages = tool_result.new_messages.clone();

            // Enforce result size limit
            let data = enforce_result_size(tool_result.data, tool.max_result_size_chars());

            ToolExecutionResult {
                tool_use_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                result: ToolResult {
                    data,
                    new_messages: vec![],
                    ..Default::default()
                },
                is_error: false,
                new_messages,
                hook_stopped_continuation,
                duration_ms,
            }
        }
        Err(e) => {
            warn!(
                tool = tool_name,
                error = %e,
                duration_ms = duration_ms,
                "tool execution failed"
            );
            ToolExecutionResult {
                tool_use_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                result: ToolResult {
                    data: Value::String(format!("Error: {}", e)),
                    new_messages: vec![],
                    ..Default::default()
                },
                is_error: true,
                new_messages: vec![],
                hook_stopped_continuation,
                duration_ms,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// `run_tool_use` is a full orchestration function — it requires a live
// ToolUseContext (abort_signal, AppState, permissions) and spawns async I/O.
// It cannot be unit-tested without an integration harness.
//
// The pipeline stages are covered by:
//   - `super::security` tests (stages 3c.1–3c.3)
//   - `super::tests` (make_error_result, find_tool, enforce_result_size,
//                     StreamingToolExecutor, security_validate paths)
//   - `coordinator::tests` (batch-grouping and flag assignment)
//
// The test below is a compile-check: it verifies that the public API surface
// of this module (types and functions used by the pipeline) can be imported
// and referenced without errors.

#[cfg(test)]
mod tests {
    use super::super::make_error_result;
    use super::*;
    use crate::types::app_state::AppState;
    use crate::types::message::AssistantMessage;
    use crate::types::tool::{
        FileStateCache, PermissionMode, PermissionResult, ToolUseOptions, ValidationResult,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, RwLock};
    use std::time::Instant;
    use uuid::Uuid;

    struct AskTool;

    #[async_trait]
    impl Tool for AskTool {
        fn name(&self) -> &str {
            "AskTool"
        }

        async fn description(&self, _input: &Value) -> String {
            "Test tool that always requests confirmation.".into()
        }

        fn input_json_schema(&self) -> Value {
            json!({
                "type": "object",
                "additionalProperties": false
            })
        }

        fn is_concurrency_safe(&self, _input: &Value) -> bool {
            true
        }

        fn is_read_only(&self, _input: &Value) -> bool {
            true
        }

        async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
            ValidationResult::Ok
        }

        async fn check_permissions(
            &self,
            _input: &Value,
            _ctx: &ToolUseContext,
        ) -> PermissionResult {
            PermissionResult::Ask {
                message: "tool-local confirmation".into(),
            }
        }

        async fn call(
            &self,
            _input: Value,
            _ctx: &ToolUseContext,
            _parent_message: &AssistantMessage,
            _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                data: Value::String("executed".into()),
                ..Default::default()
            })
        }

        async fn prompt(&self) -> String {
            "Ask tool prompt".into()
        }
    }

    fn make_ctx(state: Arc<RwLock<AppState>>, is_non_interactive_session: bool) -> ToolUseContext {
        let state_reader = Arc::clone(&state);
        let state_writer = Arc::clone(&state);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test-model".to_string(),
                verbose: false,
                is_non_interactive_session,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: rx,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || state_reader.read().unwrap().clone()),
            set_app_state: Arc::new(move |update| {
                let current = state_writer.read().unwrap().clone();
                let next = update(current);
                *state_writer.write().unwrap() = next;
            }),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            ask_user_callback: None,
            bg_agent_tx: None,
        }
    }

    fn dummy_parent() -> AssistantMessage {
        AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }

    /// Verify that `make_error_result` (the shared early-exit helper used by
    /// every pipeline stage) produces a correctly structured error result.
    /// This is the only pure, side-effect-free code the pipeline module calls
    /// that is not already covered in `super::tests`.
    #[test]
    fn make_error_result_sets_is_error_true() {
        let started = Instant::now();
        let result = make_error_result("tool-use-1", "Bash", "something went wrong", started);
        assert!(result.is_error);
        assert_eq!(result.tool_use_id, "tool-use-1");
        assert_eq!(result.tool_name, "Bash");
        assert_eq!(result.result.data.as_str().unwrap(), "something went wrong");
        assert!(result.new_messages.is_empty());
        assert!(!result.hook_stopped_continuation);
    }

    #[test]
    fn make_error_result_duration_ms_is_non_negative() {
        let started = Instant::now();
        let result = make_error_result("id", "Read", "err", started);
        // duration_ms is a u64 — always non-negative; just confirm the field exists
        let _ = result.duration_ms;
    }

    #[test]
    fn make_error_result_empty_message() {
        let started = Instant::now();
        let result = make_error_result("id", "Tool", "", started);
        assert!(result.is_error);
        assert_eq!(result.result.data.as_str().unwrap(), "");
    }

    /// Compile-check: `run_tool_use` is in scope and the module compiles correctly.
    #[test]
    fn pipeline_run_tool_use_is_accessible() {
        // Referencing the async fn without calling it confirms it is in scope.
        // We use size_of_val on a ZST to avoid an invalid cast.
        let _ = std::mem::size_of_val(&run_tool_use);
    }

    #[tokio::test]
    async fn tool_local_ask_is_not_bypassed_by_permission_mode() {
        let mut app_state = AppState::default();
        app_state.tool_permission_context.mode = PermissionMode::Bypass;
        let state = Arc::new(RwLock::new(app_state));
        let ctx = make_ctx(state, false);
        let tools: Tools = vec![Arc::new(AskTool)];

        let result = run_tool_use(
            "tool-use-1",
            "AskTool",
            json!({}),
            &tools,
            &ctx,
            &dummy_parent(),
            None,
            &[],
        )
        .await;

        assert!(result.is_error, "tool-local Ask should stop execution");
        assert_eq!(
            result.result.data.as_str(),
            Some("Permission required: tool-local confirmation")
        );
    }
}
