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

mod execution;
mod post_tool;
mod pre_tool;

pub use post_tool::{
    fire_notification_hook, run_event_hooks, run_post_tool_failure_hooks, run_post_tool_hooks,
    run_stop_hooks,
};
pub use pre_tool::run_pre_tool_hooks;

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

// ---------------------------------------------------------------------------
// Hook types (public API)
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
    StopContinuation { message: String },
}

// ---------------------------------------------------------------------------
// Hook configuration types (deserialized from settings.json)
// ---------------------------------------------------------------------------

/// Hook configuration from settings.json.
///
/// Each event (e.g. "PreToolUse") contains a list of these, each optionally
/// matching a tool name and containing a list of hook entries to run.
#[derive(Debug, Clone, Deserialize)]
pub struct HookEventConfig {
    /// Tool name matcher (e.g., "Bash", "Read", "*").
    /// None or "*" matches all tools.
    pub matcher: Option<String>,
    /// List of hook entries to run when this config matches.
    pub hooks: Vec<HookEntry>,
}

/// A single hook entry — currently only "command" type is supported.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum HookEntry {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default = "default_timeout")]
        timeout: u64, // seconds
    },
}

fn default_timeout() -> u64 {
    60
}

/// JSON output from a hook subprocess.
///
/// The subprocess writes a single JSON line to stdout. All fields are
/// optional; the default is to continue execution without changes.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HookOutput {
    /// If false, stop tool execution.
    #[serde(rename = "continue")]
    pub should_continue: bool,
    /// Reason for stopping (post-tool hooks).
    pub stop_reason: Option<String>,
    /// Decision string (e.g., "allow", "deny", "block").
    pub decision: Option<String>,
    /// Reason for the decision.
    pub reason: Option<String>,
    /// Permission decision for pre-tool hooks ("allow" or "deny").
    pub permission_decision: Option<String>,
    /// Modified tool input (pre-tool hooks).
    pub updated_input: Option<Value>,
    /// Additional context to include in messages.
    pub additional_context: Option<String>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            should_continue: true,
            stop_reason: None,
            decision: None,
            reason: None,
            permission_decision: None,
            updated_input: None,
            additional_context: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Matcher logic
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
pub fn load_hook_configs(
    hooks_value: &HashMap<String, Value>,
    event_name: &str,
) -> Vec<HookEventConfig> {
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
