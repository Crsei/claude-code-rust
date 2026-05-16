use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value;

use crate::ui::better_view_panel::{plain_row, BetterViewPanel};
use crate::ui::command_surface::adapters::hooks::{hook_event_order, hook_summary};
use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};
use crate::ui::hooks::hooks_config_menu::{HookConfigSummary, HooksConfigMenuState};
use crate::ui::hooks::select_event_mode::HOOK_EVENTS;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksSurface {
    pub(crate) state: HooksConfigMenuState,
    pub(crate) scope_index: usize,
}

impl HooksSurface {
    pub(crate) fn new(hooks: &HashMap<String, Value>) -> Self {
        let mut items: Vec<HookConfigSummary> = HOOK_EVENTS
            .iter()
            .copied()
            .map(|event| hook_summary(event, hooks.get(event.label())))
            .collect();
        items.sort_by_key(|item| hook_event_order(item.event));
        Self {
            state: HooksConfigMenuState::new(items),
            scope_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut detail_lines = self
            .state
            .render()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        detail_lines.push(String::new());
        detail_lines.push("Actions".to_string());
        detail_lines.push(plain_row(
            "open selected scope:",
            if self.scope_index == 0 {
                "/hooks open user"
            } else {
                "/hooks open project"
            },
        ));
        detail_lines.push(plain_row("event detail:", "/hooks list <event>"));
        BetterViewPanel::new("Hooks")
            .summary(format!(
                "scope={} events={}",
                if self.scope_index == 0 {
                    "user"
                } else {
                    "project"
                },
                self.state.items.len()
            ))
            .sections_title("Settings")
            .sections(
                vec!["User settings".to_string(), "Project settings".to_string()],
                self.scope_index,
            )
            .detail_title("Hook events")
            .detail_lines(detail_lines)
            .footer("Left/Right scope | Up/Down event | Enter list | o open scope | Esc close")
            .render()
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.scope_index = cycle_index(self.scope_index, 2, -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.scope_index = cycle_index(self.scope_index, 2, 1);
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
                .selected_event()
                .map(|event| {
                    CommandSurfaceOutcome::Submit(format!("/hooks list {}", event.label()))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Char('u') => CommandSurfaceOutcome::Submit("/hooks open user".to_string()),
            KeyCode::Char('p') => CommandSurfaceOutcome::Submit("/hooks open project".to_string()),
            KeyCode::Char('o') => CommandSurfaceOutcome::Submit(format!(
                "/hooks open {}",
                if self.scope_index == 0 {
                    "user"
                } else {
                    "project"
                }
            )),
            _ => CommandSurfaceOutcome::None,
        }
    }
}
