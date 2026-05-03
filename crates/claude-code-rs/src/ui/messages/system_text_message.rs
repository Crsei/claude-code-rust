//! Rust-side helper for generic system text messages.

use crate::ui::theme::Theme;

pub fn render_system_text_message(tag: &str, message: &str, _theme: &Theme) -> String {
    let tag = tag.trim();
    let message = message.trim();
    if message.is_empty() {
        if tag.is_empty() {
            "System message".to_string()
        } else {
            format!("System({tag})")
        }
    } else if tag.is_empty() {
        message.to_string()
    } else {
        format!("System({tag}): {message}")
    }
}
