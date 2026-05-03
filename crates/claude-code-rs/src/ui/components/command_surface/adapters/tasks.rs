use crate::ui::command_surface::surfaces::tasks::{TaskSurfaceItem, TaskSurfaceSource};
use crate::ui::tasks::{
    TaskKind as UiTaskKind, TaskState as UiTaskState, TaskStatus as UiTaskStatus,
};
pub(crate) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn elapsed_ms_since_timestamp(timestamp: i64) -> u64 {
    chrono::Utc::now()
        .timestamp()
        .saturating_sub(timestamp)
        .max(0) as u64
        * 1000
}

pub(crate) fn task_surface_items() -> Vec<TaskSurfaceItem> {
    let mut items = crate::tools::tasks::global_store()
        .list()
        .into_iter()
        .map(tool_task_surface_item)
        .collect::<Vec<_>>();
    items.extend(
        crate::teams::in_process::InProcessBackend::task_snapshots()
            .into_iter()
            .map(team_task_surface_item),
    );
    items
}

pub(crate) fn tool_task_surface_item(task: crate::tools::tasks::TaskEntry) -> TaskSurfaceItem {
    let title = if task.subject.trim().is_empty() {
        task.id.clone()
    } else {
        task.subject.clone()
    };
    let summary = first_non_empty([
        task.output_summary.as_str(),
        task.description.as_str(),
        task.status.as_str(),
    ]);
    let elapsed_ms = elapsed_ms_since_timestamp(task.created_at);
    let output_lines = task
        .output
        .lines()
        .take(20)
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TaskSurfaceItem {
        task: UiTaskStatus {
            id: task.id,
            title,
            kind: ui_task_kind_from_tool_kind(&task.kind),
            state: ui_task_state_from_tool_status(task.status),
            progress: None,
            summary,
            elapsed_ms,
            output_lines,
        },
        source: TaskSurfaceSource::Tool,
    }
}

pub(crate) fn team_task_surface_item(
    task: crate::teams::in_process::TeammateTaskSnapshot,
) -> TaskSurfaceItem {
    let summary = if task.has_error {
        first_non_empty([
            task.error_message.as_deref().unwrap_or_default(),
            "team task failed",
        ])
    } else if task.awaiting_plan_approval {
        "awaiting plan approval".to_string()
    } else if task.is_idle {
        "idle".to_string()
    } else {
        first_non_empty([task.prompt.as_str(), "working"])
    };

    TaskSurfaceItem {
        task: UiTaskStatus {
            id: task.id,
            title: format!("{} ({})", task.agent_name, task.team_name),
            kind: UiTaskKind::InProcessTeammate,
            state: ui_task_state_from_team_status(task.status, task.has_error),
            progress: None,
            summary,
            elapsed_ms: 0,
            output_lines: vec![task.prompt.clone()],
        },
        source: TaskSurfaceSource::Team {
            teammate_name: task.agent_name,
        },
    }
}

pub(crate) fn ui_task_kind_from_tool_kind(kind: &str) -> UiTaskKind {
    match kind.to_ascii_lowercase().as_str() {
        value if value.contains("agent") => UiTaskKind::AsyncAgent,
        value if value.contains("remote") => UiTaskKind::RemoteSession,
        value if value.contains("monitor") => UiTaskKind::MonitorMcp,
        value if value.contains("dream") => UiTaskKind::Dream,
        value if value.contains("workflow") => UiTaskKind::Workflow,
        _ => UiTaskKind::Shell,
    }
}

pub(crate) fn ui_task_state_from_tool_status(
    status: crate::tools::tasks::TaskStatus,
) -> UiTaskState {
    match status {
        crate::tools::tasks::TaskStatus::Pending => UiTaskState::Pending,
        crate::tools::tasks::TaskStatus::InProgress
        | crate::tools::tasks::TaskStatus::Interrupted
        | crate::tools::tasks::TaskStatus::Recoverable => UiTaskState::Running,
        crate::tools::tasks::TaskStatus::Completed => UiTaskState::Succeeded,
        crate::tools::tasks::TaskStatus::Failed => UiTaskState::Failed,
        crate::tools::tasks::TaskStatus::Cancelled | crate::tools::tasks::TaskStatus::Stopped => {
            UiTaskState::Canceled
        }
    }
}

pub(crate) fn ui_task_state_from_team_status(
    status: crate::teams::types::TaskStatus,
    has_error: bool,
) -> UiTaskState {
    if has_error {
        return UiTaskState::Failed;
    }
    match status {
        crate::teams::types::TaskStatus::Running => UiTaskState::Running,
        crate::teams::types::TaskStatus::Stopped => UiTaskState::Canceled,
        crate::teams::types::TaskStatus::Completed => UiTaskState::Succeeded,
    }
}
