//! System prompt section for browser MCP.
//!
//! Injected when at least one browser MCP server is active so the model sees
//! an explicit, dedicated block describing the browser-automation capabilities
//! it has, and a short playbook for using them safely.

use std::collections::HashSet;
use std::sync::Arc;

use crate::types::tool::Tool;

use super::detection::{detect_browser_tools, BrowserToolInfo};
use super::permissions::classify_browser_action;

/// Build the system prompt section for browser MCP capabilities.
///
/// Emits the section when EITHER of the following is true:
///
/// - at least one connected tool classifies as a browser MCP tool, OR
/// - at least one server in `browser_server_names` is configured (even if it
///   hasn't been connected yet — this keeps `--dump-system-prompt` useful
///   for validating `"browserMcp": true` entries in settings.json).
///
/// Returns `None` when neither holds — the section is opt-in and should not
/// be emitted for sessions that never touch a browser MCP.
pub fn browser_system_prompt(
    tools: &[Arc<dyn Tool>],
    browser_server_names: &HashSet<String>,
) -> Option<String> {
    let detected = detect_browser_tools(tools, browser_server_names);
    if detected.is_empty() && browser_server_names.is_empty() {
        return None;
    }

    // Group by server so the model knows which tools belong together.
    let mut by_server: std::collections::BTreeMap<&str, Vec<&BrowserToolInfo>> =
        std::collections::BTreeMap::new();
    for info in &detected {
        by_server
            .entry(info.server_name.as_str())
            .or_default()
            .push(info);
    }
    // Also list configured-but-not-yet-connected servers so the model knows
    // the capability is expected to come online.
    for server in browser_server_names {
        by_server.entry(server.as_str()).or_default();
    }

    let mut sections = Vec::new();
    for (server, infos) in &by_server {
        let mut lines = Vec::new();
        lines.push(format!("Server `{}`:", server));
        if infos.is_empty() {
            lines.push(
                "  (configured as a browser MCP server; no tools have been \
                 reported yet — they will appear once the server connects)"
                    .to_string(),
            );
        } else {
            for info in infos {
                let cat = classify_browser_action(&info.action);
                lines.push(format!(
                    "  - `{}` ({}, category: {})",
                    info.full_name,
                    info.action,
                    cat.label()
                ));
            }
        }
        sections.push(lines.join("\n"));
    }

    Some(format!(
        "# Browser Automation (via MCP)\n\n\
         One or more MCP servers in this session expose browser-automation \
         tools. These let you drive a real web browser (navigate pages, read \
         DOM, fill forms, take screenshots, run scripts). They are a \
         high-trust surface — treat them with the same care as a desktop \
         control tool.\n\n\
         ## Available browser tools\n\
         {servers}\n\n\
         ## Usage guidelines\n\
         - Start from a known state. Open or reuse an existing tab before \
           navigating; do not create an endless stack of tabs.\n\
         - Read before you write. Call `get_page_text`, `take_snapshot`, or \
           `read_page` to orient yourself before clicking or typing.\n\
         - Prefer structured selectors (from a page snapshot) over coordinates \
           when both are available.\n\
         - After a navigation, a click, or a form submission, re-observe the \
           page before deciding the action worked. DOM changes are not \
           instantaneous — use a wait tool if the server provides one.\n\
         - Do NOT paste user secrets into forms unless the user has clearly \
           asked for that specific action on that specific site.\n\
         - JavaScript execution (`evaluate_script`/`javascript_tool`) and file \
           uploads are particularly sensitive. Explain what you plan to do \
           before you call them.\n\
         - Browser console and network logs may contain sensitive tokens. \
           Summarize findings instead of dumping raw logs back to the user \
           unless they asked to see them.\n\n\
         ## When NOT to use these tools\n\
         - If you can accomplish the task with `WebFetch` / a direct HTTP \
           request, prefer that — it is cheaper, more reliable, and does not \
           touch a live browser session.\n\
         - Do not use browser tools to bypass a site's terms, scrape rate \
           limits, or automate logged-in actions on accounts the user did not \
           explicitly ask you to operate.\n",
        servers = sections.join("\n\n"),
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_no_browser_tools() {
        let tools: Vec<Arc<dyn Tool>> = Vec::new();
        let servers: HashSet<String> = HashSet::new();
        assert!(browser_system_prompt(&tools, &servers).is_none());
    }

    // Positive case is exercised by the e2e smoke test in tests/mcp_browser_e2e.rs
    // where a real McpToolWrapper participates in the tool list.
}
