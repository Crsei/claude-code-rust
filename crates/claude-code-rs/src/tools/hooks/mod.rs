//! Tool execution hooks — pre-tool, post-tool, stop, and permission hooks.
//!
//! Corresponds to: LIFECYCLE_STATE_MACHINE.md §6 (Phase E)
//!   - Pre-Tool Hooks: run before tool execution, can modify input or stop
//!   - Post-Tool Hooks: run after successful tool execution
//!   - Post-Tool Failure Hooks: run after failed tool execution
//!   - Stop Hooks: run when the model stops
//!
//! Hooks are user-defined shell commands configured in settings.json under
//! the `hooks` key. Each hook event (PreToolUse, PostToolUse, Stop) contains
//! a list of HookEventConfig entries, each with an optional matcher and a
//! list of HookEntry commands to execute as subprocesses.
//!
//! The plain data types and the `HookRunner` trait live in `cc-types::hooks`.
//! This module provides the concrete shell-command runner (`ShellHookRunner`)
//! together with free functions that the rest of the crate uses directly.

mod execution;
mod post_tool;
mod pre_tool;

pub use post_tool::{
    fire_notification_hook, run_event_hooks, run_post_tool_failure_hooks, run_stop_hooks,
};
pub use pre_tool::run_pre_tool_hooks;

// Re-export the plain data types from cc-types so existing
// `crate::tools::hooks::{HookEventConfig, HookOutput, ...}` import paths keep
// working without changes.
pub use cc_types::hooks::{
    HookEntry, HookEventConfig, HookOutput, HookRunner, HooksMap, PermissionOverride,
    PostToolHookResult, PreToolHookResult,
};

use async_trait::async_trait;
use serde_json::Value;

// ---------------------------------------------------------------------------
// ShellHookRunner — concrete HookRunner backed by the shell-command impl
// ---------------------------------------------------------------------------

/// Shell-command-backed `HookRunner`.
///
/// Delegates every trait method to the free functions in this module so the
/// implementations (`execute_command_hook`, subprocess spawning, etc.) remain
/// in one place.
pub struct ShellHookRunner;

impl ShellHookRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellHookRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HookRunner for ShellHookRunner {
    fn load_hook_configs(&self, hooks_value: &HooksMap, event_name: &str) -> Vec<HookEventConfig> {
        cc_types::hooks::load_hook_configs(hooks_value, event_name)
    }

    async fn run_pre_tool_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PreToolHookResult> {
        run_pre_tool_hooks(tool_name, input, hook_configs).await
    }

    async fn run_post_tool_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        tool_result_data: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        post_tool::run_post_tool_hooks_data(tool_name, input, tool_result_data, hook_configs).await
    }

    async fn run_post_tool_failure_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        error: &str,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<()> {
        run_post_tool_failure_hooks(tool_name, input, error, hook_configs).await
    }

    async fn run_event_hooks(
        &self,
        event_name: &str,
        payload: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<HookOutput> {
        run_event_hooks(event_name, payload, hook_configs).await
    }

    async fn run_stop_hooks(
        &self,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        run_stop_hooks(hook_configs).await
    }
}
