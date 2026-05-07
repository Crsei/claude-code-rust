//! Review-artifact permission request rendering.

use crate::ui::permissions::utils::{PermissionRequestView, render_permission_request};

pub fn render_review_artifact_permission_request(artifact_path: &str, reviewer: &str) -> String {
    let view = PermissionRequestView::new("Review artifact permission", "review", artifact_path)
        .with_detail(format!("reviewer: {reviewer}"));
    render_permission_request(&view)
}
