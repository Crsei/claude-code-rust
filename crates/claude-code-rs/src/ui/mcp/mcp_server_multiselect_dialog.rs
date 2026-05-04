//! MCP `.mcp.json` multi-server approval dialog rendering.

use super::mcp_server_dialog_copy::mcp_server_safety_copy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerMultiSelectItem {
    pub name: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerMultiselectState {
    pub servers: Vec<McpServerMultiSelectItem>,
    pub selected_index: usize,
}

impl McpServerMultiselectState {
    pub fn new(names: Vec<String>) -> Self {
        Self {
            servers: names
                .into_iter()
                .map(|name| McpServerMultiSelectItem {
                    name,
                    selected: true,
                })
                .collect(),
            selected_index: 0,
        }
    }

    pub fn selected_names(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|server| server.selected)
            .map(|server| server.name.clone())
            .collect()
    }

    pub fn rejected_names(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|server| !server.selected)
            .map(|server| server.name.clone())
            .collect()
    }

    pub fn approve_command(&self) -> Option<String> {
        command_for("approve", &self.selected_names())
    }

    pub fn reject_command(&self) -> Option<String> {
        command_for("reject", &self.rejected_names())
    }
}

pub fn render_mcp_server_multiselect_dialog(state: &McpServerMultiselectState) -> String {
    let mut lines = vec![
        format!("{} new MCP servers found in .mcp.json", state.servers.len()),
        "Select any you wish to enable.".to_string(),
        mcp_server_safety_copy().to_string(),
    ];

    for (idx, server) in state.servers.iter().enumerate() {
        let marker = if idx == state.selected_index {
            ">"
        } else {
            " "
        };
        let checked = if server.selected { "x" } else { " " };
        lines.push(format!("{marker} [{checked}] {}", server.name));
    }

    if let Some(command) = state.approve_command() {
        lines.push(format!("enable: {command}"));
    }
    if let Some(command) = state.reject_command() {
        lines.push(format!("disable: {command}"));
    }
    lines.push("Enter apply | Space toggle | Esc reject all".to_string());
    lines.join("\n")
}

fn command_for(verb: &str, names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    Some(format!("/mcp {verb} {}", names.join(" ")))
}
