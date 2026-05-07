use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value;

use crate::ui::command_surface::adapters::hooks::{hook_event_order, hook_summary};
use crate::ui::command_surface::{CommandSurfaceOutcome, cycle_index, render_tabs};
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
        format!(
            "{}\n{}\n\nLeft/Right switch settings scope | Up/Down navigate | Enter select | o open scope | Esc close",
            render_tabs(&["User settings", "Project settings"], self.scope_index),
            self.state.render()
        )
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
