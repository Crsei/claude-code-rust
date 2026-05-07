//! Dialog-level permission summary rendering.

use super::utils::{PermissionRequestView, render_permission_request};

pub fn render_permission_dialog_summary(view: &PermissionRequestView, width: u16) -> String {
    let mut rendered = render_permission_request(view);
    rendered.push_str(&format!("\nlayout width: {width}"));
    rendered
}
