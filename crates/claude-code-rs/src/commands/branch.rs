//! `/branch` command — show or manage git branches.
//!
//! Without arguments: lists all local branches with current branch marked.
//! With arguments: creates or switches to a branch.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::utils::git;

pub struct BranchHandler;

#[async_trait]
impl CommandHandler for BranchHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        if !git::is_git_repo(&ctx.cwd) {
            return Ok(CommandResult::Output(
                "Error: not in a git repository.".to_string(),
            ));
        }

        let args = args.trim();

        if args.is_empty() {
            // List branches
            let branches = git::list_branches(&ctx.cwd)?;

            if branches.is_empty() {
                return Ok(CommandResult::Output("No branches found.".to_string()));
            }

            let mut lines = Vec::new();
            for b in &branches {
                let marker = if b.is_head { "* " } else { "  " };
                lines.push(format!("{}{}", marker, b.name));
            }

            Ok(CommandResult::Output(lines.join("\n")))
        } else {
            // Switch or create branch via git
            let output = std::process::Command::new("git")
                .args(["checkout", args])
                .current_dir(&ctx.cwd)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let msg = String::from_utf8_lossy(&out.stdout);
                    let err = String::from_utf8_lossy(&out.stderr);
                    // git checkout prints to stderr
                    let display = if !err.trim().is_empty() {
                        err.trim().to_string()
                    } else {
                        msg.trim().to_string()
                    };
                    Ok(CommandResult::Output(format!(
                        "Switched to branch '{}'.\n{}",
                        args, display
                    )))
                }
                Ok(_out) => {
                    // Branch doesn't exist — try creating it
                    let create = std::process::Command::new("git")
                        .args(["checkout", "-b", args])
                        .current_dir(&ctx.cwd)
                        .output();

                    match create {
                        Ok(c) if c.status.success() => Ok(CommandResult::Output(format!(
                            "Created and switched to new branch '{}'.",
                            args
                        ))),
                        Ok(c) => {
                            let stderr = String::from_utf8_lossy(&c.stderr);
                            Ok(CommandResult::Output(format!(
                                "Failed to switch/create branch '{}':\n{}",
                                args,
                                stderr.trim()
                            )))
                        }
                        Err(e) => Ok(CommandResult::Output(format!("Failed to run git: {}", e))),
                    }
                }
                Err(e) => Ok(CommandResult::Output(format!("Failed to run git: {}", e))),
            }
        }
    }
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

    fn test_ctx(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_branch_not_in_git_repo_list() {
        let handler = BranchHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("not in a git repository"),
                    "expected error message, got: {}",
                    text
                );
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_branch_not_in_git_repo_with_name() {
        let handler = BranchHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result = handler
            .execute("feature/my-branch", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("not in a git repository"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
