//! File write diff summary rendering.

#[cfg(test)]
use crate::ui::diff::file_edit_diff::render_file_edit_diff_preview;

pub fn render_file_write_tool_diff(path: &str, new_lines: usize, replaced_lines: usize) -> String {
    format!("write diff: {path}\nnew lines: {new_lines}\nreplaced lines: {replaced_lines}")
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[cfg(test)]
pub fn render_file_write_tool_diff_from_hunks(
    path: &str,
    hunk_lines: &[String],
    width: usize,
    max_lines: usize,
) -> String {
    render_file_edit_diff_preview(path, hunk_lines, width, max_lines).join("\n")
}
