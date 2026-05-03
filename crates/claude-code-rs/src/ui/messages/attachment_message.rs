//! Rust-side helper for message attachment previews.

use crate::ui::theme::Theme;

pub fn render_attachment_message(label: &str, detail: &str, _theme: &Theme) -> String {
    let detail_line = if detail.trim().is_empty() {
        "no details".to_string()
    } else {
        detail.to_string()
    };
    format!("Attachment: {label} -> {detail_line}")
}
