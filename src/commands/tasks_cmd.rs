//! `/tasks` slash command — background task list + detail controls.
//!
//! Rust issue #54. Aggregates every task surface cc-rust already tracks
//! today (tool-driven tasks in `tools::tasks::GLOBAL_STORE`, teammate
//! runners in `teams::in_process::TASK_REGISTRY`) and renders them as a
//! drill-down tree.
//!
//! Core acceptance criteria:
//! * `list` shows every current task, grouped by kind;
//! * `show <id>` drills in on one task, printing full prompt / error /
//!   subject;
//! * `stop <id>` asks the tool-side store to mark the task stopped.
//!
//! Remote-agent tasks are out of scope for the rust-lite branch (the
//! `remote` module was removed); the framing here leaves room for that
//! bucket without pulling it in.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Local, TimeZone};

use super::{CommandContext, CommandHandler, CommandResult};
use crate::teams::in_process::{InProcessBackend, TeammateTaskSnapshot};
use crate::teams::types::TaskStatus as TeamTaskStatus;
use crate::tools::tasks::{global_store, TaskEntry, TaskStatus as ToolTaskStatus};
use crate::ui::browser::{render_with_footer, TreeNode};

pub struct TasksHandler;

#[async_trait]
impl CommandHandler for TasksHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("").to_ascii_lowercase();
        match sub.as_str() {
            "" | "list" | "ls" => Ok(CommandResult::Output(render_list())),
            "show" | "info" => {
                let id = parts.next().unwrap_or("").trim();
                if id.is_empty() {
                    return Ok(CommandResult::Output("Usage: /tasks show <id>".to_string()));
                }
                Ok(CommandResult::Output(render_detail(id)))
            }
            "stop" | "kill" => {
                let id = parts.next().unwrap_or("").trim();
                if id.is_empty() {
                    return Ok(CommandResult::Output("Usage: /tasks stop <id>".to_string()));
                }
                Ok(CommandResult::Output(stop_task(id)))
            }
            other => Ok(CommandResult::Output(format!(
                "Unknown /tasks subcommand '{}'.\n\n\
                 Usage:\n  \
                 /tasks               — list current background tasks\n  \
                 /tasks show <id>     — drill into one task\n  \
                 /tasks stop <id>     — mark a tool task as stopped\n",
                other
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_list() -> String {
    let tool_tasks = global_store().list();
    let team_tasks = InProcessBackend::task_snapshots();

    let mut roots: Vec<TreeNode> = Vec::new();

    // Tool-backed tasks: shell, tool orchestration, whatever the TaskCreate
    // tool registered.
    let mut tool_node = TreeNode::leaf(format!("Tool tasks ({})", tool_tasks.len()));
    if tool_tasks.is_empty() {
        tool_node.push_child(TreeNode::leaf("(none)"));
    } else {
        for task in &tool_tasks {
            tool_node.push_child(
                TreeNode::leaf(format!("{}  {}", task.id, task.subject))
                    .with_badge(format!("tool:{}", task.status.as_str()))
                    .with_detail(format_timestamp(task.created_at)),
            );
        }
    }
    roots.push(tool_node);

    // Team-backed tasks: in-process teammates registered through the Team
    // spawning flow. These are runtime-only and only appear when the user
    // is inside a team.
    let mut team_node = TreeNode::leaf(format!("Team tasks ({})", team_tasks.len()));
    if team_tasks.is_empty() {
        team_node.push_child(TreeNode::leaf("(none)"));
    } else {
        for task in &team_tasks {
            let status_tag = format!(
                "team:{}",
                match task.status {
                    TeamTaskStatus::Running if task.is_idle => "running-idle",
                    TeamTaskStatus::Running => "running",
                    TeamTaskStatus::Stopped => "stopped",
                    TeamTaskStatus::Completed => "completed",
                }
            );
            team_node.push_child(
                TreeNode::leaf(format!(
                    "{}  {} ({})",
                    task.id, task.agent_name, task.team_name
                ))
                .with_badge(status_tag)
                .with_detail(if task.has_error {
                    format!("error: {}", task.error_message.clone().unwrap_or_default())
                } else {
                    format!("prompt: {}", first_line(&task.prompt))
                }),
            );
        }
    }
    roots.push(team_node);

    let footer = "\
Tool tasks persist across turns; team tasks live for the duration of the \
parent team. Use `/tasks show <id>` for full output, or `/tasks stop <id>` \
to mark a tool task as stopped.";

    render_with_footer("Background tasks", &roots, footer)
}

fn render_detail(id: &str) -> String {
    if let Some(task) = global_store().get(id) {
        return render_tool_detail(&task);
    }
    if let Some(task) = InProcessBackend::task_snapshots()
        .into_iter()
        .find(|t| t.id == id)
    {
        return render_team_detail(&task);
    }
    format!(
        "No task with id '{}'. Run `/tasks` to list every current background task.",
        id
    )
}

fn render_tool_detail(task: &TaskEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!("Tool task {}\n", task.id));
    out.push_str(&"─".repeat(11 + task.id.len()));
    out.push('\n');
    out.push_str(&format!("  Subject:     {}\n", task.subject));
    out.push_str(&format!("  Description: {}\n", task.description));
    out.push_str(&format!("  Status:      {}\n", task.status.as_str()));
    out.push_str(&format!(
        "  Created:     {}\n",
        format_timestamp(task.created_at)
    ));
    out.push_str(&format!(
        "  Updated:     {}\n",
        format_timestamp(task.updated_at)
    ));
    if task.output.is_empty() {
        out.push_str("  Output:      (empty)\n");
    } else {
        out.push_str("  Output:\n");
        for line in task.output.lines() {
            out.push_str(&format!("    {}\n", line));
        }
    }
    if matches!(
        task.status,
        ToolTaskStatus::Pending | ToolTaskStatus::InProgress
    ) {
        out.push_str("\nTip: `/tasks stop <id>` marks the task as stopped.\n");
    }
    out
}

fn render_team_detail(task: &TeammateTaskSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!("Team task {}\n", task.id));
    out.push_str(&"─".repeat(11 + task.id.len()));
    out.push('\n');
    out.push_str(&format!(
        "  Agent:    {} ({})\n",
        task.agent_name, task.agent_id
    ));
    out.push_str(&format!("  Team:     {}\n", task.team_name));
    out.push_str(&format!(
        "  Status:   {}{}\n",
        match task.status {
            TeamTaskStatus::Running => "running",
            TeamTaskStatus::Stopped => "stopped",
            TeamTaskStatus::Completed => "completed",
        },
        if task.is_idle { " (idle)" } else { "" }
    ));
    if task.awaiting_plan_approval {
        out.push_str("  Plan:     awaiting approval\n");
    }
    if let Some(model) = &task.model {
        out.push_str(&format!("  Model:    {}\n", model));
    }
    if task.has_error {
        out.push_str(&format!(
            "  Error:    {}\n",
            task.error_message.clone().unwrap_or_default()
        ));
    }
    out.push_str("  Prompt:\n");
    for line in task.prompt.lines() {
        out.push_str(&format!("    {}\n", line));
    }
    out.push_str(
        "\nTip: team tasks are controlled through `/team` — `/tasks stop` only \
         applies to tool tasks.\n",
    );
    out
}

fn stop_task(id: &str) -> String {
    if let Some(entry) = global_store().stop(id) {
        return format!(
            "Stopped tool task '{}' (was {}).",
            entry.subject,
            entry.status.as_str()
        );
    }
    if InProcessBackend::task_snapshots()
        .iter()
        .any(|t| t.id == id)
    {
        return format!(
            "'{}' is a team task — use `/team kill <name>` to stop a teammate, \
             `/tasks stop` only applies to tool tasks.",
            id
        );
    }
    format!(
        "No task with id '{}' — run `/tasks` to see the current list.",
        id
    )
}

fn format_timestamp(ts: i64) -> String {
    match Local.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => {
            // Chrono can return `None` or `Ambiguous` on DST edges; fall
            // back to the raw timestamp so we never crash the command.
            let as_local: DateTime<Local> = Local::now();
            format!("unknown (now ~{})", as_local.format("%Y-%m-%d %H:%M"))
        }
    }
}

fn first_line(s: &str) -> String {
    const MAX: usize = 80;
    let line = s.lines().next().unwrap_or(s).trim();
    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let truncated: String = line.chars().take(MAX).collect();
    format!("{}…", truncated)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn list_always_has_both_buckets() {
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Tool tasks"));
                assert!(s.contains("Team tasks"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_without_id_returns_usage() {
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("Usage: /tasks show")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_missing_id_reports_missing() {
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("show missing-id", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("No task with id")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn stop_without_id_returns_usage() {
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("stop", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("Usage: /tasks stop")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_shows_usage() {
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("banana", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /tasks"));
                assert!(s.contains("Usage"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn stop_marks_tool_task_stopped() {
        let store = global_store();
        let task = store.create("unit", "created-by-test");
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler
            .execute(&format!("stop {}", task.id), &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Stopped tool task"));
                let refreshed = store.get(&task.id).unwrap();
                assert_eq!(refreshed.status, ToolTaskStatus::Stopped);
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn show_prints_tool_detail_fields() {
        let store = global_store();
        let task = store.create("detail-test", "detail description");
        let handler = TasksHandler;
        let mut ctx = make_ctx();
        let result = handler
            .execute(&format!("show {}", task.id), &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Tool task"));
                assert!(s.contains("detail-test"));
                assert!(s.contains("detail description"));
                assert!(s.contains("Status:"));
            }
            _ => panic!("expected Output"),
        }
    }
}
