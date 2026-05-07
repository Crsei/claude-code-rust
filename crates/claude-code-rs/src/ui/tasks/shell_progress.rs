//! Shell task progress rendering.

use super::TaskStatus;
use super::task_status_utils::{format_elapsed, progress_bar, state_label};

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
