//! Shell task progress rendering.

use super::task_status_utils::{progress_bar, state_label};
use super::TaskStatus;

pub fn render_shell_progress(task: &TaskStatus) -> String {
    format!(
        "$ {}\nstate: {}\nprogress: [{}]\n{}",
        task.title,
        state_label(task.state),
        progress_bar(task, 20),
        task.summary
    )
}
