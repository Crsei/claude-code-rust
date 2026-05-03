//! MCP server list panel rendering.

use super::index::McpServer;

pub fn render_mcp_list_panel(servers: &[McpServer], selected_index: usize) -> String {
    if servers.is_empty() {
        return "MCP servers\nNo servers configured".to_string();
    }
    let mut lines = vec![format!("MCP servers ({})", servers.len())];
    for (idx, server) in servers.iter().enumerate() {
        let marker = if idx == selected_index { ">" } else { " " };
        lines.push(format!(
            "{marker} {:<14} {:<7} {:<10} tools={}",
            server.name,
            server.kind.label(),
            server.status.label(),
            server.tools.len()
        ));
        if !server.command_or_url.is_empty() {
            lines.push(format!("  {}", server.command_or_url));
        }
    }
    lines.join("\n")
}
