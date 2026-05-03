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
