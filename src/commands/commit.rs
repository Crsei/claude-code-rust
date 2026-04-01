//! `/commit` command — create a git commit from staged changes.
//!
//! Stages all changes, generates a commit message summary, and commits.
//! Optionally accepts a custom message via arguments.

#![allow(unused)]

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::utils::git;

pub struct CommitHandler;

#[async_trait]
impl CommandHandler for CommitHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        // Check if we're in a git repo
        if !git::is_git_repo(&ctx.cwd) {
            return Ok(CommandResult::Output(
                "Error: not in a git repository.".to_string(),
            ));
        }

        let status = git::get_status(&ctx.cwd)?;

        if status.staged.is_empty() && status.unstaged.is_empty() && status.untracked.is_empty() {
            return Ok(CommandResult::Output(
                "Nothing to commit — working tree clean.".to_string(),
            ));
        }

        // Build a summary of what would be committed
        let mut lines = Vec::new();
        lines.push("**Current changes:**".to_string());

        if !status.staged.is_empty() {
            lines.push(format!("  Staged: {} file(s)", status.staged.len()));
            for f in &status.staged {
                lines.push(format!("    {:?} {}", f.status, f.path));
            }
        }
        if !status.unstaged.is_empty() {
            lines.push(format!("  Unstaged: {} file(s)", status.unstaged.len()));
            for f in &status.unstaged {
                lines.push(format!("    {:?} {}", f.status, f.path));
            }
        }
        if !status.untracked.is_empty() {
            lines.push(format!("  Untracked: {} file(s)", status.untracked.len()));
        }

        if args.trim().is_empty() {
            // No message provided — ask the model to help draft one
            lines.push(String::new());
            lines.push("Please review the changes and create a git commit. Stage the relevant files and write a clear commit message summarizing the changes.".to_string());

            let prompt = lines.join("\n");
            let msg = crate::types::message::Message::User(crate::types::message::UserMessage {
                uuid: uuid::Uuid::new_v4(),
                role: "user".to_string(),
                content: crate::types::message::MessageContent::Text(prompt),
                timestamp: chrono::Utc::now().timestamp(),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            });
            Ok(CommandResult::Query(vec![msg]))
        } else {
            // Message provided — commit directly
            let output = std::process::Command::new("git")
                .args(["commit", "-m", args.trim()])
                .current_dir(&ctx.cwd)
                .output()
                .context("Failed to run git commit")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                Ok(CommandResult::Output(format!(
                    "Committed successfully.\n{}",
                    stdout.trim()
                )))
            } else {
                Ok(CommandResult::Output(format!(
                    "Commit failed:\n{}{}",
                    stdout,
                    stderr
                )))
            }
        }
    }
}
