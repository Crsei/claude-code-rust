//! Main tool execution pipeline.

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;
use tracing::{debug, warn};

use crate::permissions::decision::{self, PermissionBehavior, PermissionDecision};
use crate::tools::hooks::{
    self, HookEventConfig, PermissionOverride, PostToolHookResult, PreToolHookResult,
};
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
    // Resolve hook permission override first, then fall back to rule engine
    if let Some(override_decision) = permission_override {
        match override_decision {
            PermissionOverride::Deny { reason } => {
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Permission denied by hook: {}", reason),
                    started,
                );
            }
            PermissionOverride::Allow => {
                // Hook explicitly allowed — skip normal permission check
                debug!(tool = tool_name, "permission allowed by hook override");
            }
        }
    } else {
        // Normal permission check via the rule engine
        let perm_result = tool.check_permissions(&effective_input, ctx).await;
        match perm_result {
            crate::types::tool::PermissionResult::Allow { .. } => {
                // Allowed
            }
            crate::types::tool::PermissionResult::Deny { message } => {
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Permission denied: {}", message),
                    started,
                );
            }
            crate::types::tool::PermissionResult::Ask { message } => {
                // In non-interactive mode, deny with explanation
                if ctx.options.is_non_interactive_session {
                    return make_error_result(
                        tool_use_id,
                        tool_name,
                        &format!("Permission required (non-interactive mode): {}", message),
                        started,
                    );
                }
                // In interactive mode, this would prompt the user.
                // Phase 1: deny with explanation.
                return make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Permission required: {}", message),
                    started,
                );
            }
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
    use super::super::{make_error_result, ToolExecutionResult};
    use super::*;
    use std::time::Instant;

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
}
