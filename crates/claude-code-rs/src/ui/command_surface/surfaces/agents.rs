use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::agents::agent_detail::render_agent_detail;
use crate::ui::agents::agents_list::AgentsListState;
use crate::ui::agents::types::{AgentDefinition, AgentSourceFilter};
use crate::ui::agents::utils::get_agent_source_display_name;
use crate::ui::better_view_panel::{plain_row, selected_row, BetterViewPanel};
use crate::ui::command_surface::adapters::agents::{agent_entry_to_ui, agent_source_tabs};
use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};

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
        let agents: Vec<AgentDefinition> = cc_services::agent_definitions::list_all_agents(cwd)
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
                let sections = self
                    .source_tabs
                    .iter()
                    .map(|source| get_agent_source_display_name(*source))
                    .collect::<Vec<_>>();
                let visible = self.state.visible_agents();
                let mut detail_lines = if visible.is_empty() {
                    vec!["No agents available for this source".to_string()]
                } else {
                    visible
                        .iter()
                        .enumerate()
                        .map(|(idx, agent)| {
                            let detail =
                                format!("{}  {}", agent.source.display_name(), agent.when_to_use);
                            selected_row(
                                &agent.agent_type,
                                detail,
                                !self.state.create_new_selected && idx == self.state.selected_index,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                if let Some(agent) = self.state.selected_agent() {
                    detail_lines.push(String::new());
                    detail_lines.push("Details".to_string());
                    detail_lines.push(plain_row(
                        "command:",
                        format!("/agents show {}", agent.agent_type),
                    ));
                }
                BetterViewPanel::new("Agents")
                    .summary(format!(
                        "source={} agents={}",
                        sections.get(self.source_index).cloned().unwrap_or_default(),
                        visible.len()
                    ))
                    .sections_title("Sources")
                    .sections(sections, self.source_index)
                    .detail_title("Agents")
                    .detail_lines(detail_lines)
                    .footer("Left/Right source | Up/Down agent | Enter show | Esc close")
                    .render()
            }
            AgentsSurfaceMode::Detail(agent) => {
                let mut detail_lines = vec![format!("Agent detail: {}", agent.agent_type)];
                detail_lines.extend(render_agent_detail(agent, 80).lines().map(str::to_string));
                detail_lines.push(String::new());
                detail_lines.push(format!("Enter submit `/agents show {}`", agent.agent_type));
                BetterViewPanel::new(format!("Agents / {}", agent.agent_type))
                    .summary(format!("source={}", agent.source.display_name()))
                    .sections_title("View")
                    .sections(vec!["Agent detail".to_string(), "Commands".to_string()], 0)
                    .detail_title("Details")
                    .detail_lines(detail_lines)
                    .footer("Backspace/b return to list | Enter show | Esc close")
                    .render()
            }
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
