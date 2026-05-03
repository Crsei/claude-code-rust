//! Rust-side helper for assistant text messages.

use crate::ui::theme::Theme;

pub fn render_assistant_text_message(text: &str, _theme: &Theme) -> String {
    if text.trim().is_empty() {
        "Assistant: <empty text>".to_string()
    } else {
        format!("Assistant: {text}")
    }
}
