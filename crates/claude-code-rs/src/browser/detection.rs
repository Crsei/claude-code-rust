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

use cc_engine::types::tool::Tool;
use cc_mcp::McpServerConfig;

use cc_browser::detection::extract_browser_action;
#[cfg(test)]
use cc_browser::detection::{
    browser_servers_snapshot, clear_browser_servers_for_tests, install_browser_servers,
    is_browser_server,
};

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
            oauth: None,
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
