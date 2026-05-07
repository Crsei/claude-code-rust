//! Bash permission request rendering.

use crate::ui::permissions::shell_permission_helpers::{
    ShellKind, render_shell_permission_details, shell_permission_options,
};
use crate::ui::permissions::utils::{PermissionRequestView, render_permission_request};

pub fn render_bash_permission_request(command: &str, selected_index: usize) -> String {
    let view = PermissionRequestView::new("Bash command permission", "bash", command)
        .with_detail(render_shell_permission_details(ShellKind::Bash, command))
        .with_options(shell_permission_options(command), selected_index);
    render_permission_request(&view)
}
