//! Hook event selection.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    Notification,
    Stop,
    SubagentStop,
}

impl HookEvent {
    pub fn label(self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::Notification => "Notification",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStop => "SubagentStop",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "Before a tool executes",
            HookEvent::PostToolUse => "After a tool returns",
            HookEvent::Notification => "When a notification is emitted",
            HookEvent::Stop => "When the main agent stops",
            HookEvent::SubagentStop => "When a subagent stops",
        }
    }
}

pub const HOOK_EVENTS: &[HookEvent] = &[
    HookEvent::PreToolUse,
    HookEvent::PostToolUse,
    HookEvent::Notification,
    HookEvent::Stop,
    HookEvent::SubagentStop,
];

pub fn render_select_event_mode(selected: HookEvent) -> String {
    let mut lines = vec!["Select hook event".to_string()];
    for event in HOOK_EVENTS {
        let marker = if *event == selected { ">" } else { " " };
        lines.push(format!(
            "{marker} {:<14} {}",
            event.label(),
            event.description()
        ));
    }
    lines.join("\n")
}
