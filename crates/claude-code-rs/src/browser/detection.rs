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

/// MCP tool-name prefix for all MCP-wrapped tools.
pub(crate) const MCP_PREFIX: &str = "mcp__";

/// Known browser-automation tool basenames.
///
/// Intentionally generous — different servers use different names for
/// essentially the same actions (`click` vs `browser_click`, `navigate`
/// vs `goto`). We match on basename only, so the leading `mcp__{server}__`
/// has already been stripped.
pub(crate) const BROWSER_TOOL_BASENAMES: &[&str] = &[
    // Navigation / tabs
    "navigate",
    "navigate_page",
    "goto",
    "tabs_create",
    "tabs_create_mcp",
    "tabs_close",
    "tabs_close_mcp",
    "tabs_context",
    "tabs_context_mcp",
    "new_page",
    "close_page",
    "switch_browser",
    "select_page",
    "list_pages",
    // Page reading
    "read_page",
    "get_page_text",
    "take_snapshot",
    "snapshot",
    "get_page",
    // DOM / element interaction
    "click",
    "browser_click",
    "double_click",
    "hover",
    "drag",
    "press_key",
    "type_text",
    "fill",
    "fill_form",
    "form_input",
    "select",
    // File upload
    "upload_file",
    "file_upload",
    // JavaScript execution
    "evaluate_script",
    "javascript_tool",
    "evaluate",
    // Console / network observability
    "get_console_message",
    "list_console_messages",
    "read_console_messages",
    "get_network_request",
    "list_network_requests",
    "read_network_requests",
    // Screenshots / visual
    "take_screenshot",
    "screenshot",
    // Misc
    "wait_for",
    "find",
    "resize_page",
    "resize_window",
    "emulate",
    "handle_dialog",
];

// ---------------------------------------------------------------------------
// Process-wide registry of browser server names
// ---------------------------------------------------------------------------
//
// Populated once at startup after MCP discovery + tool registration, then
// consulted by:
//   - the system-prompt assembler (to decide whether to inject the browser
//     section and which servers to mention)
//   - the permission decision path (so "Allow clicking on the page?" can
//     replace "Allow tool 'mcp__chrome__click'?" even for servers whose
//     action basename isn't in `BROWSER_TOOL_BASENAMES`).
//   - `/mcp list` (to tag browser servers distinctly).
//
// A global is justified here because the permission layer is called deep in
// the tool dispatch with no natural way to thread server metadata through.

static BROWSER_SERVERS: parking_lot::RwLock<Option<HashSet<String>>> =
    parking_lot::RwLock::new(None);

/// Install the set of browser MCP server names for the rest of the process.
///
/// Call this once after MCP discovery + tool registration. Subsequent calls
/// overwrite the registry.
pub fn install_browser_servers(servers: HashSet<String>) {
    *BROWSER_SERVERS.write() = Some(servers);
}

/// Snapshot the current set of browser server names (empty if not installed).
pub fn browser_servers_snapshot() -> HashSet<String> {
    BROWSER_SERVERS.read().clone().unwrap_or_default()
}

/// Check whether a server name is registered as a browser server. Consults the
/// global registry and falls back to `false` if the registry was never installed.
pub fn is_browser_server(name: &str) -> bool {
    match BROWSER_SERVERS.read().as_ref() {
        Some(set) => set.contains(name),
        None => false,
    }
}

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

/// Parse an MCP tool name into `(server, action)` if the action matches a
/// recognized browser basename.
pub fn extract_browser_action(tool_name: &str) -> Option<(&str, &str)> {
    let rest = tool_name.strip_prefix(MCP_PREFIX)?;
    let (server, action) = rest.split_once("__")?;
    if BROWSER_TOOL_BASENAMES.contains(&action) {
        Some((server, action))
    } else {
        None
    }
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
pub(crate) fn clear_browser_servers_for_tests() {
    *BROWSER_SERVERS.write() = None;
}

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
