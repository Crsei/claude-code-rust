//! /add-dir command -- adds a directory to the workspace.

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::tool::AdditionalWorkingDirectory;

pub struct AddDirHandler;

#[async_trait]
impl CommandHandler for AddDirHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let path_arg = args.trim();

        if path_arg.is_empty() {
            return Ok(CommandResult::Output(
                "Usage: /add-dir <path>\n\n\
                 Adds a directory to the workspace so tools can read and write files in it."
                    .to_string(),
            ));
        }

        // Resolve relative paths against the current working directory
        let resolved = if PathBuf::from(path_arg).is_absolute() {
            PathBuf::from(path_arg)
        } else {
            ctx.cwd.join(path_arg)
        };

        if !resolved.is_dir() {
            return Ok(CommandResult::Output(format!(
                "Directory not found: {}\n\
                 Please provide a path to an existing directory.",
                resolved.display()
            )));
        }

        let canonical = resolved
            .canonicalize()
            .unwrap_or_else(|_| resolved.clone());
        let key = canonical.to_string_lossy().to_string();

        // Check if already added
        if ctx
            .app_state
            .tool_permission_context
            .additional_working_directories
            .contains_key(&key)
        {
            return Ok(CommandResult::Output(format!(
                "Directory already in workspace: {}",
                key
            )));
        }

        ctx.app_state
            .tool_permission_context
            .additional_working_directories
            .insert(
                key.clone(),
                AdditionalWorkingDirectory {
                    path: key.clone(),
                    read_only: false,
                },
            );

        Ok(CommandResult::Output(format!(
            "Added directory to workspace: {}",
            key
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_empty_args_shows_usage() {
        let handler = AddDirHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Usage")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_nonexistent_dir() {
        let handler = AddDirHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("/nonexistent_dir_xyz_123", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("not found")),
            _ => panic!("Expected Output"),
        }
    }
}
