//! Rust-side helper for redacted assistant thinking messages.

use crate::ui::theme::Theme;

pub fn render_assistant_redacted_thinking_message(_theme: &Theme) -> String {
    "Assistant thinking was redacted by policy".to_string()
}
