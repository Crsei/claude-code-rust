//! Hook command selection.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookCommand {
    pub command: String,
    pub enabled: bool,
    pub timeout_seconds: Option<u64>,
}

impl HookCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            enabled: true,
            timeout_seconds: None,
        }
    }
}

pub fn render_select_hook_mode(commands: &[HookCommand], selected_index: usize) -> String {
    if commands.is_empty() {
        return "Select hook\nNo hook commands configured".to_string();
    }
    let mut lines = vec!["Select hook".to_string()];
    for (idx, command) in commands.iter().enumerate() {
        let marker = if idx == selected_index { ">" } else { " " };
        let enabled = if command.enabled {
            "enabled"
        } else {
            "disabled"
        };
        let timeout = command
            .timeout_seconds
            .map(|seconds| format!(" timeout={}s", seconds))
            .unwrap_or_default();
        lines.push(format!(
            "{marker} {:<8} {}{}",
            enabled, command.command, timeout
        ));
    }
    lines.join("\n")
}
