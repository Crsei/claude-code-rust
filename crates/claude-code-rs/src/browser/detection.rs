//! Browser MCP detection.
//!
//! Two identification paths:
//!
//! 1. **Explicit config flag** — a server entry in `settings.json` with
//!    `"browserMcp": true` is always treated as a browser MCP server.
//! 2. **Tool-name heuristic** — a server whose tool list contains any recognized
//!    browser-automation tool basename (e.g. `navigate`, `get_page_text`,
//!    `tabs_create`). This makes popular servers (`mcp-chrome`,
//!    `mcp-server-playwright`, Cursor's `browser`) work out-of-the-box.
//!
//! Detection runs over tool instances already registered in the tool registry.
//! MCP-wrapped tools report names shaped as `mcp__{server}__{tool}`, so we parse
//! the namespace to recover the server name without requiring access to
//! `McpToolWrapper` internals.

use std::collections::HashSet;
use std::sync::Arc;

use crate::mcp::McpServerConfig;
use crate::types::tool::Tool;

// Phase 4 (issue #73) moved the pure-parsing and server-registry helpers
// into `cc_browser::detection`. Re-export them here so every
// `crate::browser::detection::{MCP_PREFIX, BROWSER_TOOL_BASENAMES,
// extract_browser_action, install_browser_servers,
// browser_servers_snapshot, is_browser_server}` call site keeps
// resolving unchanged.
pub use cc_browser::detection::{
    browser_servers_snapshot, extract_browser_action, install_browser_servers, is_browser_server,
    BROWSER_TOOL_BASENAMES, MCP_PREFIX,
};
#[cfg(test)]
use cc_browser::detection::clear_browser_servers_for_tests;

/// Metadata for a tool that was classified as a browser MCP tool.
#[derive(Debug, Clone)]
pub struct BrowserToolInfo {
    /// Full tool name, e.g. `mcp__chrome__navigate`.
    pub full_name: String,
    /// Owning MCP server name, e.g. `chrome`.
    pub server_name: String,
    /// Action basename (the part after the last `__`), e.g. `navigate`.
    pub action: String,
}

/// Detect the set of browser MCP server names given registered tools and
/// server configs.
///
/// A server is a browser server if any of:
/// - its `McpServerConfig::browser_mcp == Some(true)` (explicit opt-in), OR
/// - any tool registered under `mcp__{name}__*` has a known browser basename.
pub fn detect_browser_servers(
    configs: &[McpServerConfig],
    tools: &[Arc<dyn Tool>],
) -> HashSet<String> {
    let mut servers: HashSet<String> = configs
        .iter()
        .filter(|c| c.browser_mcp.unwrap_or(false))
        .map(|c| c.name.clone())
        .collect();

    for tool in tools {
        let name = tool.user_facing_name(None);
        if let Some((server, _)) = extract_browser_action(&name) {
            servers.insert(server.to_string());
        }
    }

    servers
}

/// Walk the tool list and return detailed info for every browser-classified tool.
///
/// Uses a union of the two detection paths: any tool whose owning server is in
/// `browser_server_names` is included (even if its basename is unknown), and
/// any tool whose basename matches the browser list is included (even if its
/// server wasn't explicitly flagged).
pub fn detect_browser_tools(
    tools: &[Arc<dyn Tool>],
    browser_server_names: &HashSet<String>,
) -> Vec<BrowserToolInfo> {
    let mut out = Vec::new();

    for tool in tools {
        let full_name = tool.user_facing_name(None);
        let Some(rest) = full_name.strip_prefix(MCP_PREFIX) else {
            continue;
        };
        let Some((server, action)) = rest.split_once("__") else {
            continue;
        };

        let is_known_action = BROWSER_TOOL_BASENAMES.contains(&action);
        let is_flagged_server = browser_server_names.contains(server);

        if is_known_action || is_flagged_server {
            out.push(BrowserToolInfo {
                full_name: full_name.clone(),
                server_name: server.to_string(),
                action: action.to_string(),
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_returns_server_and_action() {
        assert_eq!(
            extract_browser_action("mcp__chrome__navigate"),
            Some(("chrome", "navigate"))
        );
        assert_eq!(
            extract_browser_action("mcp__playwright-x__tabs_create_mcp"),
            Some(("playwright-x", "tabs_create_mcp"))
        );
    }

    #[test]
    fn extract_rejects_non_mcp_tools() {
        assert!(extract_browser_action("Bash").is_none());
        assert!(extract_browser_action("Read").is_none());
    }

    #[test]
    fn extract_rejects_unknown_action() {
        assert!(extract_browser_action("mcp__filesystem__read_file").is_none());
        assert!(extract_browser_action("mcp__github__create_issue").is_none());
    }

    #[test]
    fn config_flag_marks_server_as_browser() {
        let configs = vec![McpServerConfig {
            name: "my-custom-browser".into(),
            transport: "stdio".into(),
            command: Some("my-browser-mcp".into()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: Some(true),
            disabled: None,
        }];
        let tools: Vec<Arc<dyn Tool>> = Vec::new();
        let servers = detect_browser_servers(&configs, &tools);
        assert!(servers.contains("my-custom-browser"));
    }

    #[test]
    fn detect_servers_without_tools_or_flags_is_empty() {
        let configs: Vec<McpServerConfig> = Vec::new();
        let servers = detect_browser_servers(&configs, &[]);
        assert!(servers.is_empty());
    }

    #[test]
    fn registry_install_and_snapshot() {
        let _guard = test_registry_lock();
        clear_browser_servers_for_tests();

        let mut set = HashSet::new();
        set.insert("chrome".to_string());
        install_browser_servers(set);

        assert!(is_browser_server("chrome"));
        assert!(!is_browser_server("filesystem"));
        assert_eq!(browser_servers_snapshot().len(), 1);

        clear_browser_servers_for_tests();
    }

    // Tests that touch the global registry serialize against each other.
    fn test_registry_lock() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}
