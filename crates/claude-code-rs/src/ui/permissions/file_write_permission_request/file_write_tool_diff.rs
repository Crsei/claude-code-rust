//! File write diff summary rendering.

pub fn render_file_write_tool_diff(path: &str, new_lines: usize, replaced_lines: usize) -> String {
    format!("write diff: {path}\nnew lines: {new_lines}\nreplaced lines: {replaced_lines}")
}
