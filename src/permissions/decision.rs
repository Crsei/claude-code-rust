//! Permission decision state machine — full decision flow.
//!
//! Corresponds to: LIFECYCLE_STATE_MACHINE.md §7 (Phase F)
//!
//! Decision flow:
//!   Phase 1a: Unconditional rules (tool name only)
//!     - toolAlwaysAllowedRule → allow
//!     - getDenyRuleForTool → deny
//!     - getAskRuleForTool → ask
//!
//!   Phase 1b: Pattern matching rules (tool name + parameters)
//!     - preparePermissionMatcher(input) → Matcher
//!     - Check allow/deny/ask rules with matcher
//!
//!   Phase 2: Hook interception
//!     - executePermissionRequestHooks()
//!     - Can return: allow / deny / modified permission context
//!
//!   Phase 3: Mode check
//!     - BypassPermissions → allow
//!     - DontAsk → deny (silent)
//!     - Default/Plan → interactive prompt
//!     - Auto → classifier (stub)
//!
//! Rule sources (priority descending):
//!   1. policySettings (enterprise)
//!   2. projectSettings (.claude/settings.json)
//!   3. userSettings (~/.claude/settings.json)
//!   4. localSettings (repo-specific)
//!   5. cliArg (command-line)
//!   6. session (session-level grants)

#![allow(unused)]

use serde_json::Value;
use tracing::{debug, warn};

use crate::types::tool::{PermissionMode, ToolPermissionContext, ToolPermissionRulesBySource};
use super::rules::{self, PermissionCheckResult};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Full permission decision with provenance tracking.
#[derive(Debug, Clone)]
pub struct PermissionDecision {
    /// The behavior to apply.
    pub behavior: PermissionBehavior,
    /// Optionally modified input (from hooks or rules).
    pub updated_input: Option<Value>,
    /// Human-readable reason for the decision.
    pub message: Option<String>,
    /// How the decision was reached.
    pub reason: PermissionDecisionReason,
}

/// Permission behavior — the action to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// How a permission decision was reached (for audit/debugging).
#[derive(Debug, Clone)]
pub enum PermissionDecisionReason {
    /// Matched a rule from a specific source.
    Rule { source: String, pattern: String },
    /// A hook made the decision.
    Hook { hook_name: String },
    /// The permission mode determined the outcome.
    Mode { mode: String },
    /// Pattern matching on tool input.
    PatternMatch { tool: String, pattern: String },
}

/// Denial tracking state for Auto mode.
#[derive(Debug, Clone, Default)]
pub struct DenialTracker {
    /// Consecutive denials in auto mode.
    pub consecutive_denials: usize,
    /// Total denials in auto mode.
    pub total_denials: usize,
}

impl DenialTracker {
    /// Record a denial. Returns true if fallback to interactive should happen.
    pub fn record_denial(&mut self) -> bool {
        self.consecutive_denials += 1;
        self.total_denials += 1;
        self.should_fallback_to_interactive()
    }

    /// Reset consecutive denial counter (on a successful allow).
    pub fn record_allow(&mut self) {
        self.consecutive_denials = 0;
    }

    /// Check if we should fall back to interactive prompting.
    ///
    /// consecutiveDenials >= 3 → fallback
    /// totalDenials >= 20 → forced fallback
    pub fn should_fallback_to_interactive(&self) -> bool {
        self.consecutive_denials >= 3 || self.total_denials >= 20
    }
}

// ---------------------------------------------------------------------------
// Permission matcher (Phase 1b: pattern matching)
// ---------------------------------------------------------------------------

/// Prepare a permission matcher for a tool invocation.
///
/// Corresponds to TypeScript: `preparePermissionMatcher(input)`
///
/// For Bash: matches the command prefix.
/// For file tools: matches the file path.
/// For other tools: no pattern matching.
fn prepare_matcher(tool_name: &str, input: &Value) -> Option<String> {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| {
                // Extract the first word (command prefix) for matching
                cmd.split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            }),
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Check if a pattern rule matches a matcher value.
///
/// Pattern rules in TypeScript can be:
/// - `Bash(prefix:git)` → matches commands starting with "git"
/// - `Read(/tmp/*)` → matches file paths under /tmp/
/// - Plain tool name → matches unconditionally
fn pattern_rule_matches(rule: &str, tool_name: &str, matcher: Option<&str>) -> bool {
    // Check for pattern syntax: ToolName(pattern)
    if let Some(inner_start) = rule.find('(') {
        if let Some(inner_end) = rule.rfind(')') {
            let rule_tool = &rule[..inner_start];
            if rule_tool != tool_name {
                return false;
            }
            let pattern = &rule[inner_start + 1..inner_end];

            // prefix: matching
            if let Some(prefix_val) = pattern.strip_prefix("prefix:") {
                return matcher
                    .map_or(false, |m| m.starts_with(prefix_val));
            }

            // Glob-style matching
            return matcher
                .map_or(false, |m| rules::glob_match_public(m, pattern));
        }
    }

    // Plain tool name match
    rule == tool_name
}

/// Check pattern rules against all sources.
fn check_pattern_rules(
    tool_name: &str,
    matcher: Option<&str>,
    rules_by_source: &ToolPermissionRulesBySource,
) -> Option<(String, String)> {
    for (source, patterns) in rules_by_source.iter() {
        for pattern in patterns {
            if pattern_rule_matches(pattern, tool_name, matcher) {
                return Some((source.clone(), pattern.clone()));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Full decision flow
// ---------------------------------------------------------------------------

/// Execute the full permission decision flow.
///
/// Corresponds to TypeScript: `hasPermissionsToUseTool()` in permissions.ts
pub fn has_permissions_to_use_tool(
    tool_name: &str,
    input: &Value,
    ctx: &ToolPermissionContext,
    denial_tracker: Option<&mut DenialTracker>,
) -> PermissionDecision {
    // ── Phase 1a: Unconditional rules (tool name only) ──────────────
    let simple_check = rules::check_tool_permission(tool_name, input, ctx);
    match simple_check {
        PermissionCheckResult::Allow => {
            if let Some(tracker) = denial_tracker {
                tracker.record_allow();
            }
            return PermissionDecision {
                behavior: PermissionBehavior::Allow,
                updated_input: None,
                message: None,
                reason: PermissionDecisionReason::Rule {
                    source: "unconditional".into(),
                    pattern: tool_name.into(),
                },
            };
        }
        PermissionCheckResult::Deny { reason } => {
            return PermissionDecision {
                behavior: PermissionBehavior::Deny,
                updated_input: None,
                message: Some(reason.clone()),
                reason: PermissionDecisionReason::Rule {
                    source: "unconditional".into(),
                    pattern: reason,
                },
            };
        }
        PermissionCheckResult::Ask { .. } => {
            // Fall through to pattern matching
        }
    }

    // ── Phase 1b: Pattern matching rules ────────────────────────────
    let matcher = prepare_matcher(tool_name, input);
    let matcher_ref = matcher.as_deref();

    // Check allow patterns
    if let Some((source, pattern)) =
        check_pattern_rules(tool_name, matcher_ref, &ctx.always_allow_rules)
    {
        if let Some(tracker) = denial_tracker {
            tracker.record_allow();
        }
        return PermissionDecision {
            behavior: PermissionBehavior::Allow,
            updated_input: None,
            message: None,
            reason: PermissionDecisionReason::PatternMatch {
                tool: tool_name.into(),
                pattern,
            },
        };
    }

    // Check deny patterns
    if let Some((source, pattern)) =
        check_pattern_rules(tool_name, matcher_ref, &ctx.always_deny_rules)
    {
        return PermissionDecision {
            behavior: PermissionBehavior::Deny,
            updated_input: None,
            message: Some(format!("Denied by pattern: {}", pattern)),
            reason: PermissionDecisionReason::PatternMatch {
                tool: tool_name.into(),
                pattern,
            },
        };
    }

    // ── Phase 2: Hook interception ──────────────────────────────────
    // Phase 1 stub: no hooks, fall through to mode check

    // ── Phase 3: Mode check ─────────────────────────────────────────
    match ctx.mode {
        PermissionMode::Bypass => {
            if let Some(tracker) = denial_tracker {
                tracker.record_allow();
            }
            PermissionDecision {
                behavior: PermissionBehavior::Allow,
                updated_input: None,
                message: None,
                reason: PermissionDecisionReason::Mode {
                    mode: "bypass".into(),
                },
            }
        }
        PermissionMode::Auto => {
            // Auto mode: allow by default, but check denial tracker
            if let Some(tracker) = denial_tracker {
                if tracker.should_fallback_to_interactive() {
                    // Fallback to interactive prompting
                    return PermissionDecision {
                        behavior: PermissionBehavior::Ask,
                        updated_input: None,
                        message: Some(format!(
                            "Auto mode fallback: {} consecutive denials",
                            tracker.consecutive_denials
                        )),
                        reason: PermissionDecisionReason::Mode {
                            mode: "auto_fallback".into(),
                        },
                    };
                }
                tracker.record_allow();
            }
            PermissionDecision {
                behavior: PermissionBehavior::Allow,
                updated_input: None,
                message: None,
                reason: PermissionDecisionReason::Mode {
                    mode: "auto".into(),
                },
            }
        }
        PermissionMode::Plan => {
            // Plan mode: ask for confirmation
            PermissionDecision {
                behavior: PermissionBehavior::Ask,
                updated_input: None,
                message: Some(format!(
                    "Tool '{}' requires confirmation in plan mode.",
                    tool_name
                )),
                reason: PermissionDecisionReason::Mode {
                    mode: "plan".into(),
                },
            }
        }
        PermissionMode::Default => {
            // Default mode: ask for confirmation
            PermissionDecision {
                behavior: PermissionBehavior::Ask,
                updated_input: None,
                message: Some(format!("Allow tool '{}'?", tool_name)),
                reason: PermissionDecisionReason::Mode {
                    mode: "default".into(),
                },
            }
        }
    }
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
    fn test_bypass_mode_allows_everything() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::Bypass;
        let decision = has_permissions_to_use_tool("Bash", &Value::Null, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Allow);
    }

    #[test]
    fn test_deny_rule_blocks() {
        let mut ctx = default_ctx();
        ctx.always_deny_rules
            .insert("policy".into(), vec!["Bash".into()]);
        let decision = has_permissions_to_use_tool("Bash", &Value::Null, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Deny);
    }

    #[test]
    fn test_allow_rule_allows() {
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["Read".into()]);
        let decision = has_permissions_to_use_tool("Read", &Value::Null, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Allow);
    }

    #[test]
    fn test_default_mode_asks() {
        let ctx = default_ctx();
        let decision = has_permissions_to_use_tool("SomeTool", &Value::Null, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Ask);
    }

    #[test]
    fn test_pattern_match_bash_prefix() {
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["Bash(prefix:git)".into()]);
        let input = serde_json::json!({"command": "git status"});
        let decision = has_permissions_to_use_tool("Bash", &input, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Allow);
    }

    #[test]
    fn test_pattern_no_match_different_prefix() {
        let mut ctx = default_ctx();
        ctx.always_allow_rules
            .insert("user".into(), vec!["Bash(prefix:git)".into()]);
        let input = serde_json::json!({"command": "rm -rf /"});
        let decision = has_permissions_to_use_tool("Bash", &input, &ctx, None);
        // Should fall through to mode check (default → ask)
        assert_eq!(decision.behavior, PermissionBehavior::Ask);
    }

    #[test]
    fn test_auto_mode_denial_tracker_fallback() {
        let mut tracker = DenialTracker::default();

        // Record 3 consecutive denials
        tracker.record_denial();
        tracker.record_denial();
        tracker.record_denial();

        // After 3 consecutive denials, should_fallback_to_interactive is true
        assert!(tracker.should_fallback_to_interactive());
    }

    #[test]
    fn test_auto_mode_allows_by_default() {
        let mut ctx = default_ctx();
        ctx.mode = PermissionMode::Auto;
        let decision = has_permissions_to_use_tool("SomeTool", &Value::Null, &ctx, None);
        assert_eq!(decision.behavior, PermissionBehavior::Allow);
    }

    #[test]
    fn test_denial_tracker_reset_on_allow() {
        let mut tracker = DenialTracker::default();
        tracker.record_denial();
        tracker.record_denial();
        assert_eq!(tracker.consecutive_denials, 2);

        tracker.record_allow();
        assert_eq!(tracker.consecutive_denials, 0);
        assert_eq!(tracker.total_denials, 2); // total doesn't reset
    }
}
