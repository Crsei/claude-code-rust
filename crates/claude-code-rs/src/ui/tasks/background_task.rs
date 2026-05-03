//! One-line background task row.

use super::task_status_utils::{progress_bar, state_label};
use super::TaskStatus;

pub fn render_background_task(task: &TaskStatus, selected: bool) -> String {
    let marker = if selected { ">" } else { " " };
    format!(
        "{marker} {:<18} {:<9} [{}] {}",
        task.title,
        state_label(task.state),
        progress_bar(task, 10),
        task.summary
    )
}
