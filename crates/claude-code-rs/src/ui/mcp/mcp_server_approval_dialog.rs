//! MCP `.mcp.json` server approval dialog rendering.

use crate::ui::selection_surface::{SelectionItem, SelectionSurface};

use super::mcp_server_dialog_copy::mcp_server_safety_copy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerApprovalChoice {
    pub id: String,
    pub label: String,
    pub description: String,
    pub command: String,
}

pub fn mcp_server_approval_choices(server_name: &str) -> Vec<McpServerApprovalChoice> {
    vec![
        McpServerApprovalChoice {
            id: "yes_all".to_string(),
            label: "Use this and all future MCP servers in this project".to_string(),
            description: "enable current server and trust future project MCP entries".to_string(),
            command: format!("/mcp approve {server_name} --all-project"),
        },
        McpServerApprovalChoice {
            id: "yes".to_string(),
            label: "Use this MCP server".to_string(),
            description: "enable only this project MCP server".to_string(),
            command: format!("/mcp approve {server_name}"),
        },
        McpServerApprovalChoice {
            id: "no".to_string(),
            label: "Continue without using this MCP server".to_string(),
            description: "record this project MCP server as disabled".to_string(),
            command: format!("/mcp reject {server_name}"),
        },
    ]
}

pub fn build_mcp_server_approval_surface(server_name: &str) -> SelectionSurface {
    SelectionSurface::new(
        format!("New MCP server found in .mcp.json: {server_name}"),
        mcp_server_approval_choices(server_name)
            .into_iter()
            .map(|choice| SelectionItem {
                id: choice.id,
                label: choice.label,
                description: format!("{} -> {}", choice.description, choice.command),
                enabled: true,
            })
            .collect(),
    )
}

pub fn render_mcp_server_approval_dialog(server_name: &str, selected_index: usize) -> String {
    let mut surface = build_mcp_server_approval_surface(server_name);
    if !surface.items.is_empty() {
        surface.selected = selected_index.min(surface.items.len() - 1);
    }

    let mut lines = surface.render_lines(surface.items.len());
    lines.insert(1, mcp_server_safety_copy().to_string());
    lines.push("Enter submit | Up/Down navigate | Esc reject".to_string());
    lines.join("\n")
}
