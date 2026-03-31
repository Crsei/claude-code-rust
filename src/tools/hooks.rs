//! Tool execution hooks — pre-tool, post-tool, and permission hooks.
//!
//! Corresponds to: LIFECYCLE_STATE_MACHINE.md §6 (Phase E)
//!   - Pre-Tool Hooks: run before tool execution, can modify input or stop
//!   - Post-Tool Hooks: run after successful tool execution
//!   - Post-Tool Failure Hooks: run after failed tool execution
//!
//! Hooks are user-defined shell commands configured in settings.json.
//! In Phase 1, hooks are mostly stubs — the infrastructure is in place
//! for wiring to the config system.

#![allow(unused)]

use anyhow::Result;
use serde_json::Value;
use tracing::{debug, warn};

use crate::types::tool::ToolResult;

// ---------------------------------------------------------------------------
// Hook types
// ---------------------------------------------------------------------------

/// Result of running pre-tool hooks.
#[derive(Debug, Clone)]
pub enum PreToolHookResult {
    /// Continue with execution (possibly with modified input).
    Continue {
        /// Modified input (None = use original).
        updated_input: Option<Value>,
        /// Permission override from hook.
        permission_override: Option<PermissionOverride>,
    },
    /// Stop tool execution (hook explicitly blocked it).
    Stop {
        /// Message explaining why the hook stopped execution.
        message: String,
    },
}

/// Permission override from a hook.
#[derive(Debug, Clone)]
pub enum PermissionOverride {
    /// Force allow.
    Allow,
    /// Force deny.
    Deny { reason: String },
}

/// Result of running post-tool hooks.
#[derive(Debug, Clone)]
pub enum PostToolHookResult {
    /// Continue normally.
    Continue,
    /// Hook wants to stop the continuation chain.
    StopContinuation {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Hook execution
// ---------------------------------------------------------------------------

/// Run pre-tool hooks for a tool invocation.
///
/// Corresponds to TypeScript: `runPreToolUseHooks()` in toolExecution.ts
///
/// In a full implementation, this would:
/// 1. Read hook configurations from settings
/// 2. For each matching hook, spawn a subprocess
/// 3. Parse the hook's JSON output for permission overrides, input changes, stop signals
/// 4. Aggregate results from all hooks
///
/// Phase 1: Returns Continue with no modifications.
pub async fn run_pre_tool_hooks(
    tool_name: &str,
    input: &Value,
) -> Result<PreToolHookResult> {
    debug!(tool = tool_name, "pre-tool hooks: no hooks configured (Phase 1)");
    Ok(PreToolHookResult::Continue {
        updated_input: None,
        permission_override: None,
    })
}

/// Run post-tool hooks after successful tool execution.
///
/// Corresponds to TypeScript: `runPostToolUseHooks()` in toolExecution.ts
///
/// Phase 1: No-op.
pub async fn run_post_tool_hooks(
    tool_name: &str,
    input: &Value,
    result: &ToolResult,
) -> Result<PostToolHookResult> {
    debug!(tool = tool_name, "post-tool hooks: no hooks configured (Phase 1)");
    Ok(PostToolHookResult::Continue)
}

/// Run post-tool failure hooks after failed tool execution.
///
/// Corresponds to TypeScript: `runPostToolUseFailureHooks()` in toolExecution.ts
///
/// Phase 1: No-op.
pub async fn run_post_tool_failure_hooks(
    tool_name: &str,
    input: &Value,
    error: &str,
) -> Result<()> {
    debug!(tool = tool_name, error = error, "post-tool failure hooks: no hooks configured (Phase 1)");
    Ok(())
}

/// Run post-sampling hooks (after the model response, before tool dispatch).
///
/// Corresponds to TypeScript: `executePostSamplingHooks()`
///
/// Phase 1: No-op.
pub async fn run_post_sampling_hooks() -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pre_tool_hooks_default() {
        let result = run_pre_tool_hooks("Bash", &Value::Null).await.unwrap();
        assert!(matches!(result, PreToolHookResult::Continue { .. }));
    }

    #[tokio::test]
    async fn test_post_tool_hooks_default() {
        let tool_result = ToolResult {
            data: Value::String("ok".into()),
            new_messages: vec![],
        };
        let result = run_post_tool_hooks("Bash", &Value::Null, &tool_result).await.unwrap();
        assert!(matches!(result, PostToolHookResult::Continue));
    }

    #[tokio::test]
    async fn test_post_tool_failure_hooks_default() {
        let result = run_post_tool_failure_hooks("Bash", &Value::Null, "test error").await;
        assert!(result.is_ok());
    }
}
