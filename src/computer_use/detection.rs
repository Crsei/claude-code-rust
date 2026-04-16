//! Computer Use tool detection and classification.
//!
//! Identifies `mcp__computer-use__*` tools by name prefix and classifies
//! them into risk categories for permissions and UI.

use std::sync::Arc;

use crate::types::tool::Tool;

/// Reserved MCP server name for Computer Use.
#[allow(dead_code)]
pub const COMPUTER_USE_SERVER: &str = "computer-use";

/// Prefix for all Computer Use tool names.
pub const COMPUTER_USE_PREFIX: &str = "mcp__computer-use__";

/// Risk level for Computer Use tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuRiskLevel {
    /// Read-only observation (screenshot, cursor_position)
    Medium,
    /// Active input (click, type, key, scroll)
    High,
}

/// A recognized Computer Use tool with its metadata.
#[derive(Debug, Clone)]
pub struct CuToolInfo {
    /// Full tool name (e.g. "mcp__computer-use__screenshot")
    pub full_name: String,
    /// Short action name (e.g. "screenshot")
    pub action: String,
    /// Risk classification (used by permission system in Phase 2)
    #[allow(dead_code)]
    pub risk: CuRiskLevel,
}

/// Check if a tool name belongs to Computer Use.
pub fn is_computer_use_tool(tool_name: &str) -> bool {
    tool_name.starts_with(COMPUTER_USE_PREFIX)
}

/// Extract the action name from a Computer Use tool name.
///
/// `"mcp__computer-use__screenshot"` → `Some("screenshot")`
/// `"Bash"` → `None`
pub fn extract_cu_action(tool_name: &str) -> Option<&str> {
    tool_name.strip_prefix(COMPUTER_USE_PREFIX)
}

/// Classify the risk level of a Computer Use action.
pub(crate) fn classify_risk(action: &str) -> CuRiskLevel {
    match action {
        "screenshot" | "cursor_position" => CuRiskLevel::Medium,
        _ => CuRiskLevel::High, // click, type, key, scroll, etc.
    }
}

fn risk_label(risk: CuRiskLevel) -> &'static str {
    match risk {
        CuRiskLevel::Medium => "medium risk",
        CuRiskLevel::High => "high risk",
    }
}

/// Detect all Computer Use tools from a tool list.
pub fn detect_cu_tools(tools: &[Arc<dyn Tool>]) -> Vec<CuToolInfo> {
    tools
        .iter()
        .filter_map(|t| {
            let name = t.user_facing_name(None);
            if !is_computer_use_tool(&name) {
                return None;
            }
            let action = extract_cu_action(&name)?.to_string();
            let risk = classify_risk(&action);
            Some(CuToolInfo {
                full_name: name,
                action,
                risk,
            })
        })
        .collect()
}

/// Build a system prompt section for Computer Use capabilities.
///
/// Returns `None` if no CU tools are detected.
pub fn computer_use_system_prompt(tools: &[Arc<dyn Tool>]) -> Option<String> {
    let cu_tools = detect_cu_tools(tools);
    if cu_tools.is_empty() {
        return None;
    }

    let tool_list: Vec<String> = cu_tools
        .iter()
        .map(|t| format!("- `{}` ({}, {})", t.full_name, t.action, risk_label(t.risk)))
        .collect();

    Some(format!(
        "# Computer Use\n\n\
         You have access to Computer Use tools from the `{}` MCP server that let you observe and interact with the user's desktop.\n\n\
         Available Computer Use tools:\n\
         {}\n\n\
         ## Usage guidelines\n\
         - Always take a screenshot first to understand what is on screen before acting.\n\
         - After performing an action (click, type, key), take another screenshot to verify the result.\n\
         - Use coordinates from the screenshot to target clicks precisely.\n\
         - Be cautious with keyboard shortcuts — they can have unintended side effects.\n\
         - Prefer using existing UI elements (buttons, fields) over keyboard shortcuts when possible.\n",
        COMPUTER_USE_SERVER,
        tool_list.join("\n")
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_computer_use_tool() {
        assert!(is_computer_use_tool("mcp__computer-use__screenshot"));
        assert!(is_computer_use_tool("mcp__computer-use__left_click"));
        assert!(!is_computer_use_tool("mcp__filesystem__read_file"));
        assert!(!is_computer_use_tool("Bash"));
    }

    #[test]
    fn test_extract_cu_action() {
        assert_eq!(
            extract_cu_action("mcp__computer-use__screenshot"),
            Some("screenshot")
        );
        assert_eq!(
            extract_cu_action("mcp__computer-use__left_click"),
            Some("left_click")
        );
        assert_eq!(extract_cu_action("Bash"), None);
    }

    #[test]
    fn test_classify_risk() {
        assert_eq!(classify_risk("screenshot"), CuRiskLevel::Medium);
        assert_eq!(classify_risk("cursor_position"), CuRiskLevel::Medium);
        assert_eq!(classify_risk("left_click"), CuRiskLevel::High);
        assert_eq!(classify_risk("type_text"), CuRiskLevel::High);
        assert_eq!(classify_risk("key"), CuRiskLevel::High);
        assert_eq!(classify_risk("scroll"), CuRiskLevel::High);
    }
}
