//! Auto-dream detail dialog.

use super::TaskStatus;
use super::task_status_utils::task_header;

pub fn render_dream_detail_dialog(task: &TaskStatus) -> String {
    format!("{}\ndream summary: {}", task_header(task), task.summary)
}
