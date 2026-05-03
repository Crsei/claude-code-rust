//! Rust-side helper for assistant thinking messages.

use crate::ui::theme::Theme;

pub fn render_assistant_thinking_message(thinking: &str, _theme: &Theme) -> String {
    let body = thinking.trim();
    if body.is_empty() {
        return "Assistant thinking: <empty>".to_string();
    }

    let preview_len = body.chars().take(120).collect::<String>();
    if body.len() > preview_len.len() {
        format!("Assistant thinking: {preview_len}...")
    } else {
        format!("Assistant thinking: {body}")
    }
}
