//! Top-level hooks configuration menu.

use super::select_event_mode::HookEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookConfigSummary {
    pub event: HookEvent,
    pub matcher_count: usize,
    pub command_count: usize,
}

pub fn render_hooks_config_menu(items: &[HookConfigSummary], selected_index: usize) -> String {
    let mut lines = vec!["Hooks".to_string()];
    if items.is_empty() {
        lines.push("No hooks configured".to_string());
        return lines.join("\n");
    }
    for (idx, item) in items.iter().enumerate() {
        let marker = if idx == selected_index { ">" } else { " " };
        lines.push(format!(
            "{marker} {:<14} matchers={} commands={}",
            item.event.label(),
            item.matcher_count,
            item.command_count
        ));
    }
    lines.push("a add | e edit | d delete".to_string());
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksConfigMenuState {
    pub items: Vec<HookConfigSummary>,
    pub selected_index: usize,
}

impl HooksConfigMenuState {
    pub fn new(items: Vec<HookConfigSummary>) -> Self {
        Self {
            items,
            selected_index: 0,
        }
    }

    pub fn move_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.items.len();
    }

    pub fn move_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.items.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    pub fn selected_event(&self) -> Option<HookEvent> {
        self.items.get(self.selected_index).map(|item| item.event)
    }

    pub fn render(&self) -> String {
        render_hooks_config_menu(&self.items, self.selected_index)
    }
}
