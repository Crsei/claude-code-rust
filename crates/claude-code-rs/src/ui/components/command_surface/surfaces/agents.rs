use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::agents::agents_list::AgentsListState;
use crate::ui::agents::types::{AgentDefinition, AgentSourceFilter};
use crate::ui::agents::utils::get_agent_source_display_name;
use crate::ui::command_surface::adapters::agents::{agent_entry_to_ui, agent_source_tabs};
use crate::ui::command_surface::{cycle_index, render_tabs, CommandSurfaceOutcome};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsSurface {
    pub(crate) state: AgentsListState,
    pub(crate) source_tabs: Vec<AgentSourceFilter>,
    pub(crate) source_index: usize,
}

impl AgentsSurface {
    pub(crate) fn new(cwd: &Path) -> Self {
        let agents: Vec<AgentDefinition> = crate::ipc::agent_settings::list_all_agents(cwd)
            .into_iter()
            .map(agent_entry_to_ui)
            .collect();
        let source_tabs = agent_source_tabs(&agents);
        let mut state = AgentsListState::new(AgentSourceFilter::All, agents);
        state.show_create_new = false;
        state.create_new_selected = false;
        Self {
            state,
            source_tabs,
            source_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        let labels = self
            .source_tabs
            .iter()
            .map(|source| get_agent_source_display_name(*source))
            .collect::<Vec<_>>();
        format!(
            "{}\n{}\n\nLeft/Right switch source tabs | Up/Down navigate | Enter select | Esc close",
            render_tabs(&labels, self.source_index),
            self.state.render()
        )
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.switch_source_tab(-1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.switch_source_tab(1);
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
            KeyCode::Enter => self
                .state
                .selected_agent()
                .map(|agent| {
                    CommandSurfaceOutcome::Submit(format!("/agents show {}", agent.agent_type))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn switch_source_tab(&mut self, direction: isize) {
        self.source_index = cycle_index(self.source_index, self.source_tabs.len(), direction);
        if let Some(source) = self.source_tabs.get(self.source_index).copied() {
            self.state.source = source;
            self.state.selected_index = 0;
            self.state.create_new_selected = false;
        }
    }
}
