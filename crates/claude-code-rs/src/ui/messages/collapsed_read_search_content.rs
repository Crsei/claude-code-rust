//! Rust-side helper for collapsed read/search content.

use crate::ui::theme::Theme;

pub fn render_collapsed_read_search_content(
    source: &str,
    line_count: usize,
    _theme: &Theme,
) -> String {
    if line_count == 0 {
        format!("Read {source}: no lines")
    } else {
        format!("Read {source}: {line_count} lines (collapsed)")
    }
}
