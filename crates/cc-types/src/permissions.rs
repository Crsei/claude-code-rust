//! Permission-context types used by both the tool trait (still in the
//! root crate) and subsystems that need to consult permission rules without
//! pulling in the full `ToolUseContext` (which still depends on
//! `ipc::agent_channel::AgentSender`).
//!
//! Moved into cc-types in Phase 4 (issue #73) so downstream crates like
//! `cc-sandbox` and `cc-permissions` can be workspace leaves without a
//! cycle through the root crate.

use std::collections::HashMap;

/// Permission mode.
///
/// Mirrors the six modes described in
/// `docs/claude-code-configuration/permissions.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionMode {
    /// Default / ask mode: require user confirmation.
    Default,
    /// Auto: auto-approve (with a safety classifier).
    Auto,
    /// Bypass: skip all permission checks.
    Bypass,
    /// Plan: read-only, no writes.
    Plan,
    /// AcceptEdits: Write/Edit/MultiEdit and common workspace filesystem
    /// commands are allowed by default; other tools still follow the normal
    /// ask flow.
    AcceptEdits,
    /// DontAsk: silently deny any request that would otherwise prompt.
    /// Useful for headless / CI to avoid blocking. `deny` rules still win.
    DontAsk,
}

impl PermissionMode {
    /// Parse a mode string. Accepts both kebab-case and camelCase tokens
    /// from the permissions docs as well as the legacy lower-case forms.
    /// Unknown / empty values fall back to [`PermissionMode::Default`].
    pub fn parse(value: &str) -> PermissionMode {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => PermissionMode::Auto,
            "bypass" | "bypasspermissions" | "bypass-permissions" => PermissionMode::Bypass,
            "plan" | "readonly" | "read-only" => PermissionMode::Plan,
            "acceptedits" | "accept-edits" | "accept_edits" => PermissionMode::AcceptEdits,
            "dontask" | "dont-ask" | "dont_ask" | "no-ask" => PermissionMode::DontAsk,
            _ => PermissionMode::Default,
        }
    }

    /// Stable lower-case identifier (camelCase) used for source-map tagging
    /// and `/permissions show` output.
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::Auto => "auto",
            PermissionMode::Bypass => "bypass",
            PermissionMode::Plan => "plan",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::DontAsk => "dontAsk",
        }
    }
}

/// Tool permission context — the slice of runtime state that permission /
/// sandbox code consults.
#[derive(Debug, Clone)]
pub struct ToolPermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: HashMap<String, AdditionalWorkingDirectory>,
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
    /// Session-level allow grants (cleared on session end).
    ///
    /// Checked between `always_allow_rules` and mode fallback.
    /// Used for Computer Use "always allow" to avoid permanent rules
    /// for high-risk desktop control tools.
    pub session_allow_rules: ToolPermissionRulesBySource,
    pub is_bypass_permissions_mode_available: bool,
    pub is_auto_mode_available: Option<bool>,
    /// The permission mode in effect before plan mode was entered
    /// (used to restore on exit).
    pub pre_plan_mode: Option<PermissionMode>,
}

impl ToolPermissionContext {
    /// Add a session-level allow grant for a tool.
    pub fn grant_session_allow(&mut self, tool_name: &str) {
        self.session_allow_rules
            .entry("session".into())
            .or_default()
            .push(tool_name.to_string());
    }

    /// Check if a tool has a session-level allow grant.
    pub fn has_session_grant(&self, tool_name: &str) -> bool {
        self.session_allow_rules
            .values()
            .any(|rules| rules.iter().any(|r| r == tool_name))
    }

    /// Clear all session-level grants (called on session end).
    pub fn clear_session_grants(&mut self) {
        self.session_allow_rules.clear();
    }
}

#[derive(Debug, Clone, Default)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub read_only: bool,
}

/// Permission rules grouped by origin (settings layer, session, etc.).
pub type ToolPermissionRulesBySource = HashMap<String, Vec<String>>;
