use std::collections::HashSet;
use std::sync::Arc;

use crate::types::tool::Tool;

pub mod detection {
    pub use cc_browser::detection::browser_servers_snapshot;
}

pub mod prompt {
    use super::*;

    #[derive(Debug, Clone)]
    struct BrowserToolInfo {
        full_name: String,
        server_name: String,
        action: String,
    }

    pub fn browser_system_prompt(
        tools: &[Arc<dyn Tool>],
        browser_server_names: &HashSet<String>,
    ) -> Option<String> {
        let detected = detect_browser_tools(tools, browser_server_names);
        if detected.is_empty() && browser_server_names.is_empty() {
            return None;
        }

        let mut by_server: std::collections::BTreeMap<&str, Vec<&BrowserToolInfo>> =
            std::collections::BTreeMap::new();
        for info in &detected {
            by_server
                .entry(info.server_name.as_str())
                .or_default()
                .push(info);
        }
        for server in browser_server_names {
            by_server.entry(server.as_str()).or_default();
        }

        let mut sections = Vec::new();
        for (server, infos) in &by_server {
            let mut lines = Vec::new();
            lines.push(format!("Server `{}`:", server));
            if infos.is_empty() {
                lines.push(
                    "  (configured as a browser MCP server; no tools have been reported yet)"
                        .to_string(),
                );
            } else {
                for info in infos {
                    let cat = cc_browser::permissions::classify_browser_action(&info.action);
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
             One or more MCP servers in this session expose browser-automation tools.\n\n\
             ## Available browser tools\n\
             {servers}\n\n\
             ## Usage guidelines\n\
             - Start from a known state and re-observe after navigation, clicks, or form submissions.\n\
             - Prefer structured selectors over coordinates when both are available.\n\
             - Do not paste user secrets into forms unless explicitly requested.\n",
            servers = sections.join("\n\n"),
        ))
    }

    fn detect_browser_tools(
        tools: &[Arc<dyn Tool>],
        browser_server_names: &HashSet<String>,
    ) -> Vec<BrowserToolInfo> {
        let mut out = Vec::new();
        for tool in tools {
            let full_name = tool.user_facing_name(None);
            let Some(rest) = full_name.strip_prefix(cc_browser::detection::MCP_PREFIX) else {
                continue;
            };
            let Some((server, action)) = rest.split_once("__") else {
                continue;
            };
            let server = server.to_string();
            let action = action.to_string();
            let is_known_action =
                cc_browser::detection::BROWSER_TOOL_BASENAMES.contains(&action.as_str());
            let is_flagged_server = browser_server_names.contains(&server);
            if is_known_action || is_flagged_server {
                out.push(BrowserToolInfo {
                    full_name,
                    server_name: server,
                    action,
                });
            }
        }
        out
    }
}
