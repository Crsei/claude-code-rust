//! `/review` command — request a code review of current changes.
//!
//! Collects the git diff and sends it to the model for review.
//! Optionally accepts a custom review focus via arguments.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::utils::git;

pub struct ReviewHandler;

#[async_trait]
impl CommandHandler for ReviewHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        if !git::is_git_repo(&ctx.cwd) {
            return Ok(CommandResult::Output(
                "Error: not in a git repository.".to_string(),
            ));
        }

        // Collect diffs
        let staged = git::diff_staged(&ctx.cwd).unwrap_or_default();
        let unstaged = git::diff_unstaged(&ctx.cwd).unwrap_or_default();

        if staged.is_empty() && unstaged.is_empty() {
            return Ok(CommandResult::Output(
                "No changes to review.".to_string(),
            ));
        }

        let mut summary_parts = Vec::new();

        if !staged.is_empty() {
            let files: Vec<&str> = staged.iter().map(|e| e.path.as_str()).collect();
            summary_parts.push(format!(
                "**Staged changes** ({} file(s)): {}",
                staged.len(),
                files.join(", ")
            ));
        }

        if !unstaged.is_empty() {
            let files: Vec<&str> = unstaged.iter().map(|e| e.path.as_str()).collect();
            summary_parts.push(format!(
                "**Unstaged changes** ({} file(s)): {}",
                unstaged.len(),
                files.join(", ")
            ));
        }

        let focus = if args.trim().is_empty() {
            String::new()
        } else {
            format!("\n\nFocus area: {}", args.trim())
        };

        let prompt = format!(
            "Please review the following code changes and provide feedback on correctness, \
             style, potential bugs, and suggestions for improvement.\n\n{}{}\n\n\
             Use `git diff` and `git diff --cached` to see the actual diff content.",
            summary_parts.join("\n"),
            focus
        );

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
    }
}
