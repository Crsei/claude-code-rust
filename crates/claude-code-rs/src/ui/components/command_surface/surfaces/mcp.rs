use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::command_surface::adapters::mcp::{build_mcp_servers, selected_server_command};
use crate::ui::command_surface::{CommandSurfaceOutcome, cycle_index, render_tabs};
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
        format!(
            "{}\n{}\n\nLeft/Right switch action tabs | Up/Down navigate | Enter select | a add | Esc close",
            render_tabs(
                &["Status", "Edit", "Reconnect", "Remove"],
                self.action_index
            ),
            self.state.render()
        )
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
