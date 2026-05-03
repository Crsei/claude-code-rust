//! Read-only hook detail view.

use super::select_event_mode::HookEvent;
use super::select_hook_mode::HookCommand;
use super::select_matcher_mode::HookMatcher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookView {
    pub event: HookEvent,
    pub matcher: HookMatcher,
    pub commands: Vec<HookCommand>,
}

pub fn render_view_hook_mode(view: &HookView) -> String {
    let mut lines = vec![
        format!("Event: {}", view.event.label()),
        format!("Matcher: {}", view.matcher.label()),
        "Commands:".to_string(),
    ];
    if view.commands.is_empty() {
        lines.push("  <none>".to_string());
    } else {
        for command in &view.commands {
            let enabled = if command.enabled {
                "enabled"
            } else {
                "disabled"
            };
            lines.push(format!("  - {} ({enabled})", command.command));
        }
    }
    lines.join("\n")
}
