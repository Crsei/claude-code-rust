use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::better_view_panel::{plain_row, selected_row, BetterViewPanel};
use crate::ui::command_surface::adapters::team::team_summary_from_state;
use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};
use crate::ui::teams::teams_dialog::{TeamSummary, TeammateStatus};
use cc_engine::types::app_state::AppState;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSurface {
    pub(crate) summary: TeamSummary,
    pub(crate) active: bool,
    pub(crate) selected_index: usize,
}

impl TeamSurface {
    pub(crate) fn new(state: &AppState) -> Self {
        let (summary, active) = team_summary_from_state(state);
        Self {
            summary,
            active,
            selected_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        let teammates = self
            .summary
            .teammates
            .iter()
            .filter(|teammate| teammate.name != "team-lead")
            .collect::<Vec<_>>();
        let mut detail_lines = if teammates.is_empty() {
            vec!["No teammates".to_string()]
        } else {
            teammates
                .iter()
                .enumerate()
                .map(|(idx, teammate)| {
                    selected_row(
                        &teammate.name,
                        format!(
                            "status={} mode={} tasks={} {}",
                            teammate.state, teammate.mode, teammate.assigned_tasks, teammate.role
                        ),
                        idx == self.selected_index,
                    )
                })
                .collect::<Vec<_>>()
        };
        detail_lines.push(String::new());
        detail_lines.push("Commands".to_string());
        if self.active {
            detail_lines.push(plain_row("Enter:", "/team status"));
            if let Some(teammate) = self.selected_teammate() {
                detail_lines.push(plain_row("s:", format!("/team send {} ", teammate.name)));
                detail_lines.push(plain_row(
                    "k:",
                    format!("/team kill {} direct-execute", teammate.name),
                ));
            }
            detail_lines.push(plain_row("p:", "/team spawn "));
        } else {
            detail_lines.push(plain_row("c:", "/team create "));
            detail_lines.push(plain_row("l:", "/team list"));
        }
        BetterViewPanel::new("Team")
            .summary(format!(
                "team={} active={} members={}",
                self.summary.name,
                self.active,
                teammates.len()
            ))
            .sections_title("Teammates")
            .sections(vec!["Current team".to_string()], 0)
            .detail_title("Teammate details")
            .detail_lines(detail_lines)
            .footer(if self.active {
                "Up/Down member | Enter status | s send | k kill | p spawn | Esc"
            } else {
                "c create | l list | Esc close"
            })
            .render()
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected_index =
                    cycle_index(self.selected_index, self.selectable_teammate_count(), -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.selected_index =
                    cycle_index(self.selected_index, self.selectable_teammate_count(), 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => CommandSurfaceOutcome::Submit("/team status".to_string()),
            KeyCode::Char('l') => CommandSurfaceOutcome::Submit("/team list".to_string()),
            KeyCode::Char('c') => CommandSurfaceOutcome::FillPrompt("/team create ".to_string()),
            KeyCode::Char('p') => CommandSurfaceOutcome::FillPrompt("/team spawn ".to_string()),
            KeyCode::Char('s') => self
                .selected_teammate()
                .map(|teammate| {
                    CommandSurfaceOutcome::FillPrompt(format!("/team send {} ", teammate.name))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Char('k') => self
                .selected_teammate()
                .map(|teammate| {
                    CommandSurfaceOutcome::Submit(format!("/team kill {}", teammate.name))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn selected_teammate(&self) -> Option<&TeammateStatus> {
        self.summary
            .teammates
            .iter()
            .filter(|teammate| teammate.name != "team-lead")
            .nth(self.selected_index)
    }

    fn selectable_teammate_count(&self) -> usize {
        self.summary
            .teammates
            .iter()
            .filter(|teammate| teammate.name != "team-lead")
            .count()
    }
}
