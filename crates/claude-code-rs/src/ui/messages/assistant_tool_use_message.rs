//! Rust-side helper for assistant tool-use messages.

use crate::ui::theme::Theme;

pub fn render_assistant_tool_use_message(tool_name: &str, input: &str, _theme: &Theme) -> String {
    let normalized = input.trim();
    let rendered_input = if normalized.is_empty() {
        "<no input>".to_string()
    } else {
        normalized.to_string()
    };
    format!("Assistant used tool {tool_name}: {rendered_input}")
}
