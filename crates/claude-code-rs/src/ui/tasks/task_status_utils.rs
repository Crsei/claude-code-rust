//! Shared task status formatting helpers.

use super::{TaskKind, TaskState, TaskStatus};

pub fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Pending => "pending",
        TaskState::Running => "running",
        TaskState::Succeeded => "succeeded",
        TaskState::Failed => "failed",
        TaskState::Canceled => "canceled",
    }
}

pub fn kind_label(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Shell => "shell",
        TaskKind::RemoteSession => "remote",
        TaskKind::AsyncAgent => "agent",
        TaskKind::InProcessTeammate => "teammate",
        TaskKind::MonitorMcp => "mcp",
        TaskKind::Dream => "dream",
        TaskKind::Workflow => "workflow",
    }
}

pub fn format_elapsed(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

pub fn progress_bar(task: &TaskStatus, width: usize) -> String {
    let width = width.max(1);
    let Some((done, total)) = task.progress else {
        return "-".repeat(width);
    };
    let filled = if total == 0 {
        0
    } else {
        width.saturating_mul(done.min(total)) / total
    };
    format!("{}{}", "#".repeat(filled), "-".repeat(width - filled))
}

pub fn task_header(task: &TaskStatus) -> String {
    format!(
        "{} [{}] {} {}",
        task.title,
        kind_label(task.kind),
        state_label(task.state),
        format_elapsed(task.elapsed_ms)
    )
}
