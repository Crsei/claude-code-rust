//! Permission rule matching engine.
//!
//! Evaluates tool permission rules in priority order:
//! 1. Deny rules (blanket deny -> deny with message)
//! 2. Allow rules (if matched -> allow)
//! 3. Ask rules (if matched -> ask)
//! 4. Default: depends on mode (Default -> ask, Auto -> allow, Bypass -> allow)

#![allow(unused)]

use crate::types::tool::{PermissionMode, ToolPermissionContext, ToolPermissionRulesBySource};
use serde_json::Value;

/// Result of a permission check against the rule engine.
#[derive(Debug, Clone)]
pub enum PermissionCheckResult {
    /// Tool execution is allowed.
    Allow,
    /// Tool execution is denied.
    Deny { reason: String },
    /// User must be asked for confirmation.
    Ask { message: String },
}

/// Check whether a tool invocation is permitted given the current permission context.
///
/// Evaluation order:
/// 1. Deny rules (highest priority -- any match blocks execution)
/// 2. Allow rules (if matched, tool is permitted)
/// 3. Ask rules (if matched, prompt the user)
/// 4. Fallback based on permission mode
pub fn check_tool_permission(
    tool_name: &str,
    input: &Value,
    ctx: &ToolPermissionContext,
) -> PermissionCheckResult {
    // --- Phase 1: Check deny rules ---
    if let Some(reason) = match_rules_any_source(tool_name, &ctx.always_deny_rules) {
        return PermissionCheckResult::Deny {
            reason: if reason.is_empty() {
                format!("Tool '{}' is denied by policy.", tool_name)
            } else {
                reason
            },
        };
    }

    // --- Phase 2: Check allow rules ---
    if match_rules_any_source(tool_name, &ctx.always_allow_rules).is_some() {
        return PermissionCheckResult::Allow;
    }

    // --- Phase 3: Check ask rules ---
    if let Some(msg) = match_rules_any_source(tool_name, &ctx.always_ask_rules) {
        return PermissionCheckResult::Ask {
            message: if msg.is_empty() {
                format!("Permission required for tool '{}'.", tool_name)
            } else {
                msg
            },
        };
    }

    // --- Phase 4: Fallback by mode ---
    match ctx.mode {
        PermissionMode::Default => PermissionCheckResult::Ask {
            message: format!("Allow tool '{}'?", tool_name),
        },
        PermissionMode::Auto => {
            // In auto mode we allow by default -- a real implementation would
            // invoke the safety classifier here.
            PermissionCheckResult::Allow
        }
        PermissionMode::Bypass => PermissionCheckResult::Allow,
        PermissionMode::Plan => {
            // Plan mode: only read-only tools are allowed; the caller should
            // gate on Tool::is_read_only(). We conservatively ask.
            PermissionCheckResult::Ask {
                message: format!(
                    "Tool '{}' requires confirmation in plan mode (read-only).",
                    tool_name
                ),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Iterate all sources in a rule set and return Some(matched_rule) if any rule
/// matches `tool_name`. The returned string is the raw rule text (may be empty
/// for blanket rules).
fn match_rules_any_source(
    tool_name: &str,
    rules_by_source: &ToolPermissionRulesBySource,
) -> Option<String> {
    for (_source, rules) in rules_by_source.iter() {
        for rule in rules {
            if rule_matches(tool_name, rule) {
                return Some(rule.clone());
            }
        }
    }
    None
}

/// Check if a single rule pattern matches a tool name.
///
/// Matching strategies (in order):
/// 1. Exact match: `"Bash"` matches `"Bash"`
/// 2. Prefix match: `"mcp__server"` matches `"mcp__server__tool"`
/// 3. Glob-style wildcard: `"mcp__*"` matches `"mcp__anything"`
fn rule_matches(tool_name: &str, rule: &str) -> bool {
    // Exact match
    if tool_name == rule {
        return true;
    }

    // If the rule contains a `*`, treat it as a glob pattern.
    if rule.contains('*') {
        return glob_match(tool_name, rule);
    }

    // Prefix match (e.g. "mcp__server" matches "mcp__server__tool")
    if tool_name.starts_with(rule) {
        // Only match at a boundary (the next char after the prefix should be
        // `_`, `/`, or end-of-string).
        let rest = &tool_name[rule.len()..];
        if rest.is_empty() || rest.starts_with('_') || rest.starts_with('/') {
            return true;
        }
    }

    false
}

/// Minimal glob matching that supports `*` as a wildcard for any sequence of
/// characters. This is intentionally simple -- we only need to support the
/// patterns used in Claude Code's permission rules (e.g. `"mcp__*"`,
/// `"Bash(*)"`, `"*"`).
///
/// Also exposed as `glob_match_public` for use in the decision module.
fn glob_match(text: &str, pattern: &str) -> bool {
    // Split pattern by `*` -- all literal segments must appear in order.
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
            // First segment must match at the start if the pattern doesn't start with `*`.
            if i == 0 && !pattern.starts_with('*') && found != 0 {
                return false;
            }
            pos += found + segment.len();
        } else {
            return false;
        }
    }

    // Last segment must match at the end if the pattern doesn't end with `*`.
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
    use std::collections::HashMap;

    fn default_ctx() -> ToolPermissionContext {
        ToolPermissionContext {
            mode: PermissionMode::Default,
            additional_working_directories: HashMap::new(),
            always_allow_rules: HashMap::new(),
            always_deny_rules: HashMap::new(),
            always_ask_rules: HashMap::new(),
            is_bypass_permissions_mode_available: false,
            is_auto_mode_available: None,
            pre_plan_mode: None,
        }
    }

    #[test]
    fn test_exact_match() {
        assert!(rule_matches("Bash", "Bash"));
        assert!(!rule_matches("BashTool", "Bash"));
    }

    #[test]
    fn test_prefix_match() {
        assert!(rule_matches("mcp__server__tool", "mcp__server"));
        assert!(!rule_matches("mcp__serverX", "mcp__server"));
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
}
