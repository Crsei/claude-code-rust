//! Remote session detail dialog.

use super::TaskStatus;
use super::task_status_utils::task_header;

pub fn render_remote_session_detail_dialog(task: &TaskStatus) -> String {
    let mut lines = vec![task_header(task), format!("summary: {}", task.summary)];
    lines.push("session log:".to_string());
    for line in &task.output_lines {
        lines.push(format!("  {line}"));
    }
    lines.join("\n")
}
