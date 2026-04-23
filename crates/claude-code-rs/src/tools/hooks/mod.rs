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
    fire_notification_hook, run_event_hooks, run_post_tool_failure_hooks, run_post_tool_hooks,
    run_stop_hooks,
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
use tracing::warn;

// ---------------------------------------------------------------------------
// Matcher logic (still needed by the free-function implementations)
// ---------------------------------------------------------------------------

/// Check if a matcher pattern matches a tool name.
///
/// - `None` or `"*"` matches everything.
/// - Exact match: `"Bash"` matches `"Bash"`.
/// - Prefix match: `"mcp__"` matches `"mcp__server__tool"`.
fn matches_tool(matcher: Option<&str>, tool_name: &str) -> bool {
    match matcher {
        None => true,
        Some("*") => true,
        Some(pattern) => tool_name == pattern || tool_name.starts_with(pattern),
    }
}

// ---------------------------------------------------------------------------
// Hook config loading
// ---------------------------------------------------------------------------

/// Load hook configurations for a specific event from the hooks settings value.
///
/// `hooks_value` is the deserialized `hooks` map from GlobalConfig.
/// `event_name` is one of "PreToolUse", "PostToolUse", "Stop".
pub fn load_hook_configs(hooks_value: &HooksMap, event_name: &str) -> Vec<HookEventConfig> {
    let Some(event_value) = hooks_value.get(event_name) else {
        return vec![];
    };

    match serde_json::from_value::<Vec<HookEventConfig>>(event_value.clone()) {
        Ok(configs) => configs,
        Err(e) => {
            warn!(
                event = event_name,
                error = %e,
                "failed to deserialize hook configs"
            );
            vec![]
        }
    }
}

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
        load_hook_configs(hooks_value, event_name)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    // -- matches_tool tests --

    #[test]
    fn test_matches_tool_exact() {
        assert!(matches_tool(Some("Bash"), "Bash"));
        assert!(!matches_tool(Some("Bash"), "Read"));
    }

    #[test]
    fn test_matches_tool_wildcard() {
        assert!(matches_tool(None, "Bash"));
        assert!(matches_tool(None, "Read"));
        assert!(matches_tool(Some("*"), "Bash"));
        assert!(matches_tool(Some("*"), "anything_at_all"));
    }

    #[test]
    fn test_matches_tool_prefix() {
        assert!(matches_tool(Some("mcp__"), "mcp__server__tool"));
        assert!(matches_tool(Some("mcp__"), "mcp__another"));
        assert!(!matches_tool(Some("mcp__"), "Bash"));
        assert!(!matches_tool(Some("mcp__"), "mcp_single_underscore"));
    }

    // -- load_hook_configs tests --

    #[test]
    fn test_load_hook_configs() {
        let mut hooks_value: HashMap<String, Value> = HashMap::new();
        hooks_value.insert(
            "PreToolUse".to_string(),
            json!([
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "echo ok",
                            "timeout": 30
                        }
                    ]
                },
                {
                    "matcher": "*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "echo audit"
                        }
                    ]
                }
            ]),
        );

        let configs = load_hook_configs(&hooks_value, "PreToolUse");
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(configs[0].hooks.len(), 1);
        match &configs[0].hooks[0] {
            HookEntry::Command { command, timeout } => {
                assert_eq!(command, "echo ok");
                assert_eq!(*timeout, 30);
            }
        }
        assert_eq!(configs[1].matcher.as_deref(), Some("*"));
        match &configs[1].hooks[0] {
            HookEntry::Command { command, timeout } => {
                assert_eq!(command, "echo audit");
                assert_eq!(*timeout, 60); // default
            }
        }
    }

    #[test]
    fn test_load_hook_configs_missing_event() {
        let hooks_value: HashMap<String, Value> = HashMap::new();
        let configs = load_hook_configs(&hooks_value, "PreToolUse");
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_hook_configs_invalid_json() {
        let mut hooks_value: HashMap<String, Value> = HashMap::new();
        hooks_value.insert("PreToolUse".to_string(), json!("not an array"));
        let configs = load_hook_configs(&hooks_value, "PreToolUse");
        assert!(configs.is_empty());
    }
}
