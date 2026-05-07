use crossterm::event::{KeyCode, KeyEvent};

use crate::types::app_state::AppState;
use crate::ui::command_surface::adapters::team::team_summary_from_state;
use crate::ui::command_surface::{CommandSurfaceOutcome, cycle_index};
use crate::ui::teams::teams_dialog::{TeamSummary, TeammateStatus, render_teams_dialog};
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
        let mut lines = render_teams_dialog(&self.summary, self.selected_index)
            .lines()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let help = if self.active {
            "Enter status | k kill | s send | p spawn | l list | c create | Esc close"
        } else {
            "c create | l list | Esc close"
        };
        if lines
            .last()
            .is_some_and(|line| line.starts_with("k kill |"))
        {
            if let Some(last) = lines.last_mut() {
                *last = help.to_string();
            }
        } else {
            lines.push(help.to_string());
        }
        lines.join("\n")
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected_index =
                    cycle_index(self.selected_index, self.summary.teammates.len(), -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.selected_index =
                    cycle_index(self.selected_index, self.summary.teammates.len(), 1);
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
        self.summary.teammates.get(self.selected_index)
    }
}
