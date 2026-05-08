//! File write permission request rendering.

use super::file_write_tool_diff::render_file_write_tool_diff;
use crate::ui::permissions::file_permission_dialog::permission_options::file_permission_options;
use crate::ui::permissions::utils::{render_permission_request, PermissionRequestView};

pub fn render_file_write_permission_request(
    path: &str,
    new_lines: usize,
    replaced_lines: usize,
    selected_index: usize,
) -> String {
    let view = PermissionRequestView::new("File write permission", "write", path)
        .with_detail(render_file_write_tool_diff(path, new_lines, replaced_lines))
        .with_options(file_permission_options(path, false), selected_index);
    render_permission_request(&view)
}
