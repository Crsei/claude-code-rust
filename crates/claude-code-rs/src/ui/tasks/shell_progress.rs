//! Shell task progress rendering.

use super::task_status_utils::{format_elapsed, progress_bar, state_label};
use super::TaskStatus;

pub fn render_shell_progress(task: &TaskStatus) -> String {
    format!(
        "$ {}\nstate: {}\nelapsed: {}\nprogress: [{}]\n{}",
        task.title,
        state_label(task.state),
        format_elapsed(task.elapsed_ms),
        progress_bar(task, 20),
        task.summary
    )
}
