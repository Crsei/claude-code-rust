//! Permission rule matching engine.
//!
//! Evaluates tool permission rules in priority order, matching the
//! precedence documented in `docs/claude-code-configuration/permissions.md`:
//!
//!   1. **Deny** rules (highest priority, blocks even if allow matches)
//!   2. **Ask**  rules (force a prompt even when allow matches)
//!   3. **Allow** rules (pre-approve)
//!   4. Permission-mode fallback (Default → Ask, Auto → Allow, ...)
//!
//! `Bash` patterns are dispatched through [`crate::permissions::bash_matcher`]
//! so compound commands and process wrappers participate in matching.

use crate::permissions::bash_matcher;
use crate::types::tool::{PermissionMode, ToolPermissionContext, ToolPermissionRulesBySource};
use serde_json::Value;

/// Result of a permission check against the rule engine.
///
/// Retained as a stable internal API for tests and for callers that want
/// the rule-engine result without the full hook + mode flow (which lives
/// in [`crate::permissions::decision`]).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PermissionCheckResult {
    /// Tool execution is allowed.
    Allow,
    /// Tool execution is denied.
    Deny { reason: String },
    /// User must be asked for confirmation.
    #[allow(dead_code)]
    Ask { message: String },
}

/// Tools whose execution counts as an "edit" for [`PermissionMode::AcceptEdits`].
const EDIT_TOOLS: &[&str] = &[
    "Write",
    "Edit",
    "MultiEdit",
    "FileWrite",
    "FileEdit",
    "FileMultiEdit",
    "NotebookEdit",
    "ApplyPatch",
    "ApplyDiff",
];

/// True when a tool counts as a file-system edit for accept-edits semantics.
pub fn is_edit_tool(tool_name: &str) -> bool {
    EDIT_TOOLS.iter().any(|t| *t == tool_name)
}

/// Check whether a tool invocation is permitted given the current context.
///
/// Returns the rule-based decision only — caller is responsible for the
/// hook overlay and mode fallback (see [`crate::permissions::decision`]).
#[allow(dead_code)]
pub fn check_tool_permission(
    tool_name: &str,
    input: &Value,
    ctx: &ToolPermissionContext,
) -> PermissionCheckResult {
    // 1. Deny rules
    if let Some(reason) = match_rules_any_source(tool_name, input, &ctx.always_deny_rules) {
        return PermissionCheckResult::Deny {
            reason: if reason.is_empty() {
                format!("Tool '{}' is denied by policy.", tool_name)
            } else {
                format!("Denied by rule: {}", reason)
            },
        };
    }

    // 2. Ask rules (forced prompt — wins over allow per the spec)
    if let Some(rule) = match_rules_any_source(tool_name, input, &ctx.always_ask_rules) {
        return PermissionCheckResult::Ask {
            message: if rule.is_empty() {
                format!("Permission required for tool '{}'.", tool_name)
            } else {
                format!("Ask rule matched: {}", rule)
            },
        };
    }

    // 3. Allow rules
    if match_rules_any_source(tool_name, input, &ctx.always_allow_rules).is_some() {
        return PermissionCheckResult::Allow;
    }

    // 4. Mode fallback
    match ctx.mode {
        PermissionMode::Default => PermissionCheckResult::Ask {
            message: format!("Allow tool '{}'?", tool_name),
        },
        PermissionMode::Auto => PermissionCheckResult::Allow,
        PermissionMode::Bypass => PermissionCheckResult::Allow,
        PermissionMode::Plan => PermissionCheckResult::Ask {
            message: format!(
                "Tool '{}' requires confirmation in plan mode (read-only).",
                tool_name
            ),
        },
        PermissionMode::AcceptEdits => {
            if is_edit_tool(tool_name) {
                PermissionCheckResult::Allow
            } else {
                PermissionCheckResult::Ask {
                    message: format!("Allow tool '{}'?", tool_name),
                }
            }
        }
        PermissionMode::DontAsk => PermissionCheckResult::Deny {
            reason: format!(
                "Permission mode 'dontAsk' silently denies '{}': add an allow rule to permit it.",
                tool_name
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Iterate all sources in a rule set and return Some(matched_rule) if any
/// rule matches (`tool_name`, `input`).
#[allow(dead_code)]
fn match_rules_any_source(
    tool_name: &str,
    input: &Value,
    rules_by_source: &ToolPermissionRulesBySource,
) -> Option<String> {
    for (_source, rules) in rules_by_source.iter() {
        for rule in rules {
            if rule_matches(tool_name, input, rule) {
                return Some(rule.clone());
            }
        }
    }
    None
}

/// Check if a single rule pattern matches a tool invocation.
///
/// Supports four shapes:
///   1. Exact tool name: `"Bash"` or `"Read"`.
///   2. Tool prefix: `"mcp__server"` matches `"mcp__server__tool"`.
///   3. Glob: `"mcp__*"` matches `"mcp__anything"`.
///   4. Specifier: `"Bash(pattern)"`, `"Read(/tmp/*)"`, etc. — the inner
///      pattern is dispatched per-tool via [`specifier_matches`].
pub(crate) fn rule_matches(tool_name: &str, input: &Value, rule: &str) -> bool {
    // Specifier form: ToolName(pattern)
    if let Some(open) = rule.find('(') {
        if rule.ends_with(')') {
            let rule_tool = &rule[..open];
            if !tool_name_matches(tool_name, rule_tool) {
                return false;
            }
            let pattern = &rule[open + 1..rule.len() - 1];
            return specifier_matches(tool_name, input, pattern);
        }
    }

    tool_name_matches(tool_name, rule)
}

/// Match a rule's tool-name component (no specifier) against a tool name.
fn tool_name_matches(tool_name: &str, rule: &str) -> bool {
    if tool_name == rule {
        return true;
    }
    if rule.contains('*') {
        return glob_match(tool_name, rule);
    }
    if tool_name.starts_with(rule) {
        let rest = &tool_name[rule.len()..];
        if rest.is_empty() || rest.starts_with('_') || rest.starts_with('/') {
            return true;
        }
    }
    false
}

/// Match a `ToolName(pattern)` specifier against the tool's input.
fn specifier_matches(tool_name: &str, input: &Value, pattern: &str) -> bool {
    match tool_name {
        "Bash" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            bash_matcher::bash_pattern_matches(pattern, cmd)
        }
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            text_specifier_matches(pattern, path)
        }
        "Glob" => {
            let pat = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            text_specifier_matches(pattern, pat)
        }
        "WebFetch" | "WebSearch" => {
            let url = input
                .get("url")
                .or_else(|| input.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            text_specifier_matches(pattern, url)
        }
        "Agent" => {
            let agent_type = input
                .get("subagent_type")
                .or_else(|| input.get("agent_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            text_specifier_matches(pattern, agent_type)
        }
        _ if tool_name.starts_with("mcp__") => {
            text_specifier_matches(pattern, tool_name)
        }
        _ => {
            // Generic specifier: stringify input and glob-match.
            let s = input.to_string();
            text_specifier_matches(pattern, &s)
        }
    }
}

fn text_specifier_matches(pattern: &str, value: &str) -> bool {
    let pat = pattern.trim();
    if pat.is_empty() || pat == "*" {
        return true;
    }
    if let Some(prefix) = pat.strip_prefix("prefix:") {
        return value.starts_with(prefix);
    }
    if pat.contains('*') {
        return glob_match(value, pat);
    }
    value == pat
}

/// Minimal glob matching that supports `*` as a wildcard for any sequence
/// of characters. This is intentionally simple — we only need to support
/// the patterns used in Claude Code's permission rules.
///
/// Also exposed as `glob_match_public` for use in [`crate::permissions::bash_matcher`].
fn glob_match(text: &str, pattern: &str) -> bool {
    let segments: Vec<&str> = pattern.split('*').collect();

    if segments.is_empty() {
        return true;
    }

    let mut pos = 0usize;

    for (i, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }

        if let Some(found) = text[pos..].find(segment) {
            if i == 0 && !pattern.starts_with('*') && found != 0 {
                return false;
            }
            pos += found + segment.len();
        } else {
            return false;
        }
    }

    if !pattern.ends_with('*') {
        if let Some(last_seg) = segments.last() {
            if !last_seg.is_empty() && !text.ends_with(last_seg) {
                return false;
            }
        }
    }

    true
}

/// Public wrapper around `glob_match` for use in other permission modules.
pub fn glob_match_public(text: &str, pattern: &str) -> bool {
    glob_match(text, pattern)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn default_ctx() -> ToolPermissionContext {
        ToolPermissionContext {
            mode: PermissionMode::Default,
            additional_working_directories: HashMap::new(),
            always_allow_rules: HashMap::new(),
            always_deny_rules: HashMap::new(),
            always_ask_rules: HashMap::new(),
            session_allow_rules: HashMap::new(),
            is_bypass_permissions_mode_available: false,
            is_auto_mode_available: None,
            pre_plan_mode: None,
        }
    }

    #[test]
    fn test_exact_match() {
        assert!(rule_matches("Bash", &Value::Null, "Bash"));
        assert!(!rule_matches("BashTool", &Value::Null, "Bash"));
    }

    #[test]
    fn test_prefix_match() {
        assert!(rule_matches(
            "mcp__server__tool",
            &Value::Null,
            "mcp__server"
        ));
        assert!(!rule_matches("mcp__serverX", &Value::Null, "mcp__server"));
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("mcp__anything", "mcp__*"));
        assert!(glob_match("Bash", "*"));
        assert!(!glob_match("FileRead", "Bash*"));
        assert!(glob_match("Bash(rm -rf)", "Bash(*)"));
    }

    #[test]
    fn test_deny_overrides_allow() {
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["Bash".into()]);
        ctx.always_deny_rules
            .insert("policy".into(), vec!["Bash".into()]);

        let result = check_tool_permission("Bash", &Value::Null, &ctx);
        assert!(matches!(result, PermissionCheckResult::Deny { .. }));
    }

    #[test]
    fn test_ask_overrides_allow() {
        // New precedence: ask wins over allow.
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["Bash".into()]);
        ctx.always_ask_rules
            .insert("project".into(), vec!["Bash".into()]);
        let result = check_tool_permission("Bash", &Value::Null, &ctx);
        assert!(
            matches!(result, PermissionCheckResult::Ask { .. }),
            "ask must override allow per spec"
        );
    }

    #[test]
    fn test_allow_rule() {
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["FileRead".into()]);

        let result = check_tool_permission("FileRead", &Value::Null, &ctx);
        assert!(matches!(result, PermissionCheckResult::Allow));
    }

    #[test]
    fn test_default_mode_asks() {
        let ctx = default_ctx();
        let result = check_tool_permission("SomeTool", &Value::Null, &ctx);
        assert!(matches!(result, PermissionCheckResult::Ask { .. }));
    }

    #[test]
    fn test_bypass_mode_allows() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::Bypass;
        let result = check_tool_permission("SomeTool", &Value::Null, &ctx);
        assert!(matches!(result, PermissionCheckResult::Allow));
    }

    #[test]
    fn test_auto_mode_allows() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::Auto;
        let result = check_tool_permission("SomeTool", &Value::Null, &ctx);
        assert!(matches!(result, PermissionCheckResult::Allow));
    }

    #[test]
    fn test_accept_edits_allows_edit_tools() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::AcceptEdits;
        assert!(matches!(
            check_tool_permission("Edit", &Value::Null, &ctx),
            PermissionCheckResult::Allow
        ));
        assert!(matches!(
            check_tool_permission("Write", &Value::Null, &ctx),
            PermissionCheckResult::Allow
        ));
        // Non-edit tool still asks.
        assert!(matches!(
            check_tool_permission("Bash", &Value::Null, &ctx),
            PermissionCheckResult::Ask { .. }
        ));
    }

    #[test]
    fn test_dont_ask_denies_silently() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::DontAsk;
        let r = check_tool_permission("AnyTool", &Value::Null, &ctx);
        assert!(matches!(r, PermissionCheckResult::Deny { .. }));
    }

    #[test]
    fn test_dont_ask_respects_allow_rules() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::DontAsk;
        ctx.always_allow_rules
            .insert("user".into(), vec!["Read".into()]);
        let r = check_tool_permission("Read", &Value::Null, &ctx);
        assert!(matches!(r, PermissionCheckResult::Allow));
    }

    // -- specifier tests --

    #[test]
    fn test_bash_specifier_compound_deny_via_token() {
        let input = json!({"command": "git status && rm -rf /tmp/x"});
        assert!(rule_matches("Bash", &input, "Bash(rm)"));
        assert!(rule_matches("Bash", &input, "Bash(prefix:rm)"));
    }

    #[test]
    fn test_bash_specifier_with_wrapper() {
        let input = json!({"command": "sudo cargo build"});
        assert!(rule_matches("Bash", &input, "Bash(prefix:cargo)"));
    }

    #[test]
    fn test_read_specifier_glob() {
        let input = json!({"file_path": "/tmp/foo.log"});
        assert!(rule_matches("Read", &input, "Read(/tmp/*)"));
        assert!(!rule_matches("Read", &input, "Read(/etc/*)"));
    }

    #[test]
    fn test_webfetch_specifier_glob() {
        let input = json!({"url": "https://api.example.com/v1/foo"});
        assert!(rule_matches("WebFetch", &input, "WebFetch(https://api.example.com/*)"));
    }

    #[test]
    fn test_agent_specifier_exact() {
        let input = json!({"subagent_type": "Explore"});
        assert!(rule_matches("Agent", &input, "Agent(Explore)"));
        assert!(!rule_matches("Agent", &input, "Agent(Plan)"));
    }
}
