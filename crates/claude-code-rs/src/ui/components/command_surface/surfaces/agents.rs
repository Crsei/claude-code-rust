use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::agents::agent_detail::render_agent_detail;
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
    pub(crate) mode: AgentsSurfaceMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentsSurfaceMode {
    List,
    Detail(AgentDefinition),
}

impl AgentsSurface {
    pub(crate) fn new(cwd: &Path) -> Self {
        let agents: Vec<AgentDefinition> = crate::ipc::agent_settings::list_all_agents(cwd)
            .into_iter()
            .map(agent_entry_to_ui)
            .collect();
        let source_tabs = agent_source_tabs(&agents);
        let mut state = AgentsListState::new(AgentSourceFilter::All, agents);
        state.show_create_new = supports_create_agent_wizard_entry();
        state.create_new_selected = state.show_create_new;
        Self {
            state,
            source_tabs,
            source_index: 0,
            mode: AgentsSurfaceMode::List,
        }
    }

    pub(crate) fn render(&self) -> String {
        match &self.mode {
            AgentsSurfaceMode::List => {
                let labels = self
                    .source_tabs
                    .iter()
                    .map(|source| get_agent_source_display_name(*source))
                    .collect::<Vec<_>>();
                format!(
                    "{}\n{}\n\nLeft/Right switch source tabs | Up/Down navigate | Enter detail | Esc close",
                    render_tabs(&labels, self.source_index),
                    self.state.render()
                )
            }
            AgentsSurfaceMode::Detail(agent) => format!(
                "Agent detail: {}\n{}\n\nBackspace/b return to list | Enter submit `/agents show {}` | Esc close",
                agent.agent_type,
                render_agent_detail(agent, 80),
                agent.agent_type
            ),
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match self.mode.clone() {
            AgentsSurfaceMode::List => self.handle_list_key(key),
            AgentsSurfaceMode::Detail(agent) => match key.code {
                KeyCode::Backspace | KeyCode::Char('b') | KeyCode::Left => {
                    self.mode = AgentsSurfaceMode::List;
                    CommandSurfaceOutcome::None
                }
                KeyCode::Enter => {
                    CommandSurfaceOutcome::Submit(format!("/agents show {}", agent.agent_type))
                }
                _ => CommandSurfaceOutcome::None,
            },
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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
            KeyCode::Enter => {
                if let Some(agent) = self.state.selected_agent() {
                    self.mode = AgentsSurfaceMode::Detail(agent);
                }
                CommandSurfaceOutcome::None
            }
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn switch_source_tab(&mut self, direction: isize) {
        self.source_index = cycle_index(self.source_index, self.source_tabs.len(), direction);
        if let Some(source) = self.source_tabs.get(self.source_index).copied() {
            self.state.source = source;
            self.state.selected_index = 0;
            self.state.create_new_selected = self.state.show_create_new;
            self.mode = AgentsSurfaceMode::List;
        }
    }
}

fn supports_create_agent_wizard_entry() -> bool {
    // Keep the command surface read-only until the create wizard can commit and
    // cancel through the same AgentSettingsCommand paths used by the full agents
    // settings UI. Those paths validate User/Project destinations; exposing a
    // create row here before save/cancel is wired would create an unsafe dead end.
    false
}
