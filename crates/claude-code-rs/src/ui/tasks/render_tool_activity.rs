//! Task activity rendering.

use super::task_status_utils::{format_elapsed, state_label};
use super::TaskStatus;

pub fn render_task_tool_activity(tasks: &[TaskStatus]) -> String {
    if tasks.is_empty() {
        return "No task activity".to_string();
    }
    tasks
        .iter()
        .map(|task| {
            format!(
                "{} {} {} - {}",
                state_label(task.state),
                format_elapsed(task.elapsed_ms),
                task.title,
                task.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
