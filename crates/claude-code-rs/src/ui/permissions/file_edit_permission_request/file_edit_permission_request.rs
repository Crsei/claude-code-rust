//! File edit permission request rendering.

use crate::ui::permissions::file_permission_dialog::permission_options::file_permission_options;
use crate::ui::permissions::utils::{render_permission_request, PermissionRequestView};

pub fn render_file_edit_permission_request(
    path: &str,
    operation: &str,
    selected_index: usize,
) -> String {
    let view = PermissionRequestView::new("File edit permission", "edit", path)
        .with_detail(format!("operation: {operation}"))
        .with_options(file_permission_options(path, false), selected_index);
    render_permission_request(&view)
}
