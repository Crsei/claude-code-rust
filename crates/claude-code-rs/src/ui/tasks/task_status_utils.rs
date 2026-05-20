//! Shared task status formatting helpers.

#[cfg(test)]
use super::TaskStatus;
use super::{TaskKind, TaskState};
#[cfg(test)]
use crate::ui::progress_bar::{render_progress_bar, ProgressBar};
#[cfg(test)]
use crate::ui::theme::ThemeColors;

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

#[cfg(test)]
pub fn progress_bar(task: &TaskStatus, width: usize) -> String {
    let Some((done, total)) = task.progress else {
        return render_progress_bar(0.0, width);
    };

    let ratio = if total == 0 {
        0.0
    } else {
        done.min(total) as f64 / total as f64
    };
    render_progress_bar(ratio, width)
}

/// Theme-aware variant returning styled progress-bar spans.
///
/// Uses the new `ProgressBar` widget with the provided theme colors.
/// The progress bar uses accent color for the filled portion.
#[cfg(test)]
pub fn progress_bar_styled(
    task: &TaskStatus,
    width: usize,
    colors: &ThemeColors,
) -> ratatui::text::Line<'static> {
    let ratio = task.progress.map_or(0.0, |(done, total)| {
        if total == 0 {
            0.0
        } else {
            done.min(total) as f64 / total as f64
        }
    });
    ProgressBar::new(ratio, width).render(colors)
}

#[cfg(test)]
pub fn progress_detail(task: &TaskStatus) -> String {
    let Some((done, total)) = task.progress else {
        return "no progress reported".to_string();
    };

    if total == 0 {
        return format!("{done}/0 steps");
    }

    let capped = done.min(total);
    let percent = (capped as f64 / total as f64) * 100.0;
    format!("{capped}/{total} steps ({percent:.0}%)")
}

#[cfg(test)]
pub fn task_header(task: &TaskStatus) -> String {
    format!(
        "{} [{}] {} {}",
        task.title,
        kind_label(task.kind),
        state_label(task.state),
        format_elapsed(task.elapsed_ms)
    )
}
