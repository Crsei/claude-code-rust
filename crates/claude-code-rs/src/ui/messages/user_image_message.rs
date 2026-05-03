//! Rust-side helper for user image message attachments.

use crate::ui::theme::Theme;

pub fn render_user_image_message(path: &str, format_hint: &str, _theme: &Theme) -> String {
    let format_hint = if format_hint.trim().is_empty() {
        "unknown"
    } else {
        format_hint
    };
    if path.trim().is_empty() {
        format!("Image: <unknown> ({format_hint})")
    } else {
        format!("Image: {path} ({format_hint})")
    }
}
