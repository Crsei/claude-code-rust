//! /tasks command -- lists current tasks from the task store.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::tools::tasks::{TaskStatus, TaskStore};

pub struct TasksHandler;

#[async_trait]
impl CommandHandler for TasksHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let store = TaskStore::new();
        let tasks = store.list();

        if tasks.is_empty() {
            let subcmd = args.trim();
            if subcmd == "help" || subcmd == "-h" {
                return Ok(CommandResult::Output(
                    "Usage: /tasks\n\n\
                     Lists all tasks in the current session.\n\
                     Tasks are created and managed via the TaskCreate, TaskUpdate, and TaskList tools."
                        .to_string(),
                ));
            }
            return Ok(CommandResult::Output(
                "No tasks in the current session.\n\n\
                 Use the TaskCreate tool to create a new task, or ask the assistant \
                 to create tasks for you."
                    .to_string(),
            ));
        }

        let mut lines = Vec::new();
        lines.push(format!("Tasks ({} total)", tasks.len()));
        lines.push("─".repeat(50));

        for task in &tasks {
            let status_icon = match task.status {
                TaskStatus::Pending => "[ ]",
                TaskStatus::InProgress => "[~]",
                TaskStatus::Completed => "[x]",
                TaskStatus::Stopped => "[-]",
            };
            lines.push(format!(
                "{} {} ({})",
                status_icon,
                task.subject,
                task.status.as_str()
            ));
            if !task.description.is_empty() {
                lines.push(format!("    {}", task.description));
            }
        }

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_no_tasks() {
        let handler = TasksHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("No tasks")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_help_subcommand() {
        let handler = TasksHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("help", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Usage")),
            _ => panic!("Expected Output"),
        }
    }
}
