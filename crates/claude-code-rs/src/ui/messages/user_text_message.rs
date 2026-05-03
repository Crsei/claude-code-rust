//! Rust-side helper for plain user text messages.

use crate::ui::theme::Theme;

pub fn render_user_text_message(text: &str, _theme: &Theme) -> String {
    if text.trim().is_empty() {
        "You: <empty>".to_string()
    } else {
        format!("You: {text}")
    }
}
