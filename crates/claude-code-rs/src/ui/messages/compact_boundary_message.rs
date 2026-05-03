//! Rust-side helper for compact-boundary system messages.

use crate::ui::theme::Theme;

pub fn render_compact_boundary_message(
    before_tokens: usize,
    after_tokens: usize,
    _theme: &Theme,
) -> String {
    format!("Context compacted: before={before_tokens} after={after_tokens}")
}
