use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::better_view_panel::{plain_row, BetterViewPanel};
use crate::ui::command_surface::adapters::mcp::{build_mcp_servers, selected_server_command};
use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};
use crate::ui::mcp::mcp_list_panel::McpListPanelState;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSurface {
    pub(crate) state: McpListPanelState,
    pub(crate) action_index: usize,
}

impl McpSurface {
    pub(crate) fn new(cwd: &Path) -> Self {
        Self {
            state: McpListPanelState::new(build_mcp_servers(cwd)),
            action_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        let sections = vec![
            "Status".to_string(),
            "Edit".to_string(),
            "Reconnect".to_string(),
            "Remove".to_string(),
        ];
        let mut detail_lines = self
            .state
            .render()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        detail_lines.push(String::new());
        detail_lines.push("Commands".to_string());
        detail_lines.push(match self.action_index {
            0 => plain_row("Enter:", "/mcp status"),
            1 => plain_row("Enter:", "/mcp edit <server>"),
            2 => plain_row("Enter:", "/mcp reconnect <server>"),
            3 => plain_row("Enter:", "/mcp remove <server> direct-execute"),
            _ => plain_row("Enter:", "select"),
        });
        detail_lines.push(plain_row("a:", "/mcp add "));
        BetterViewPanel::new("MCP")
            .summary(format!(
                "servers={} action={}",
                self.state.servers.len(),
                sections.get(self.action_index).cloned().unwrap_or_default()
            ))
            .sections_title("Actions")
            .sections(sections, self.action_index)
            .detail_title("Servers")
            .detail_lines(detail_lines)
            .footer("Left/Right action | Up/Down server | Enter select | a add | Esc close")
            .render()
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.action_index = cycle_index(self.action_index, 4, -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.action_index = cycle_index(self.action_index, 4, 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self.selected_mcp_action(),
            KeyCode::Char('a') => CommandSurfaceOutcome::FillPrompt("/mcp add ".to_string()),
            KeyCode::Char('e') => selected_server_command(&self.state, "/mcp edit ", " "),
            KeyCode::Char('r') => selected_server_command(&self.state, "/mcp reconnect ", ""),
            KeyCode::Char('d') => selected_server_command(&self.state, "/mcp remove ", ""),
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn selected_mcp_action(&self) -> CommandSurfaceOutcome {
        match self.action_index {
            0 => CommandSurfaceOutcome::Submit("/mcp status".to_string()),
            1 => selected_server_command(&self.state, "/mcp edit ", " "),
            2 => selected_server_command(&self.state, "/mcp reconnect ", ""),
            3 => selected_server_command(&self.state, "/mcp remove ", ""),
            _ => CommandSurfaceOutcome::None,
        }
    }
}
