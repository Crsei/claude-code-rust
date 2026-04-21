//! Hook runner trait and plain data types for the tool-execution hook system.
//!
//! The engine uses hooks at several lifecycle points (PreToolUse, PostToolUse,
//! PostToolUseFailure, SubagentStart, SubagentStop, UserPromptSubmit,
//! InstructionsLoaded, PermissionRequest, PermissionDenied, …). The concrete
//! runner that spawns shell commands lives in the main crate's `tools::hooks`
//! module; the engine depends only on this trait so it has no direct edge to
//! `tools::hooks`.
//!
//! See issue #74 (`[workspace-split] Phase 5`).
use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Hook result types
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
// HookRunner trait
// ---------------------------------------------------------------------------

/// Type alias for the hooks map loaded from `settings.json`.
pub type HooksMap = HashMap<String, Value>;

/// Trait for running hook subprocess commands.
///
/// Decouples the engine from the concrete shell-execution implementation that
/// lives in `tools::hooks`. Object-safe: callers store this as
/// `Arc<dyn HookRunner>`.
#[async_trait]
pub trait HookRunner: Send + Sync {
    /// Load hook configurations for a specific event from the hooks settings.
    ///
    /// `event_name` is one of "PreToolUse", "PostToolUse", "Stop", etc.
    fn load_hook_configs(
        &self,
        hooks_value: &HooksMap,
        event_name: &str,
    ) -> Vec<HookEventConfig>;

    /// Run pre-tool hooks for a tool invocation.
    async fn run_pre_tool_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PreToolHookResult>;

    /// Run post-tool hooks after a successful tool call.
    ///
    /// `tool_result_data` is the serialized tool result payload (typically
    /// `ToolResult::data`).
    async fn run_post_tool_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        tool_result_data: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult>;

    /// Run post-tool failure hooks after a failed tool call.
    async fn run_post_tool_failure_hooks(
        &self,
        tool_name: &str,
        input: &Value,
        error: &str,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<()>;

    /// Generic event hook runner for non-tool lifecycle events
    /// (UserPromptSubmit, InstructionsLoaded, SubagentStart, …).
    async fn run_event_hooks(
        &self,
        event_name: &str,
        payload: &Value,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<HookOutput>;

    /// Run Stop lifecycle hooks — called by the query loop after the model
    /// stops generating (no tool calls in final assistant message).
    ///
    /// Returns `StopContinuation` if any hook asked the loop to keep going.
    async fn run_stop_hooks(
        &self,
        hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult>;
}

// ---------------------------------------------------------------------------
// NoopHookRunner — a safe default that never fires any hooks
// ---------------------------------------------------------------------------

/// A `HookRunner` that runs no hooks, regardless of settings.
///
/// Used as the default runner for engines constructed without an explicit
/// runner (e.g. in unit tests where hook semantics are irrelevant). Real call
/// sites (main binary, web handlers, IPC, teams) should override with the
/// concrete `ShellHookRunner` from `tools::hooks`.
pub struct NoopHookRunner;

impl NoopHookRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopHookRunner {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl HookRunner for NoopHookRunner {
    fn load_hook_configs(
        &self,
        _hooks_value: &HooksMap,
        _event_name: &str,
    ) -> Vec<HookEventConfig> {
        Vec::new()
    }

    async fn run_pre_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PreToolHookResult> {
        Ok(PreToolHookResult::Continue {
            updated_input: None,
            permission_override: None,
        })
    }

    async fn run_post_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _tool_result_data: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        Ok(PostToolHookResult::Continue)
    }

    async fn run_post_tool_failure_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _error: &str,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run_event_hooks(
        &self,
        _event_name: &str,
        _payload: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<HookOutput> {
        Ok(HookOutput::default())
    }

    async fn run_stop_hooks(
        &self,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        Ok(PostToolHookResult::Continue)
    }
}
