//! Rust-side helper for highlighted thinking text.

use crate::ui::theme::Theme;

pub fn render_highlighted_thinking_text(thinking: &str, _theme: &Theme) -> String {
    let trimmed = thinking.trim();
    if trimmed.is_empty() {
        "[no thinking text]".to_string()
    } else {
        format!("[THINKING] {trimmed}")
    }
}
