//! Remote session progress rendering.

use super::TaskStatus;
use super::task_status_utils::state_label;

pub fn render_remote_session_progress(task: &TaskStatus) -> String {
    format!(
        "remote: {}\nstate: {}\n{}",
        task.title,
        state_label(task.state),
        task.summary
    )
}
