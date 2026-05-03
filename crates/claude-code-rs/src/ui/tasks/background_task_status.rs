//! Footer status for background tasks.

use super::{TaskState, TaskStatus};

pub fn render_background_task_status(tasks: &[TaskStatus], selected: bool) -> String {
    let running = tasks
        .iter()
        .filter(|task| task.state == TaskState::Running)
        .count();
    let failed = tasks
        .iter()
        .filter(|task| task.state == TaskState::Failed)
        .count();
    if tasks.is_empty() {
        return "no background tasks".to_string();
    }
    let mut label = format!(
        "{} tasks, {} running, {} failed",
        tasks.len(),
        running,
        failed
    );
    if selected {
        label = format!("[{label}] Enter to view");
    }
    label
}
