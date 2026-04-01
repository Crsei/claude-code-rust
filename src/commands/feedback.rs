//! /feedback command -- show how to give feedback.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/feedback` slash command.
pub struct FeedbackHandler;

#[async_trait]
impl CommandHandler for FeedbackHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        Ok(CommandResult::Output(
            "To provide feedback, please visit: https://github.com/anthropics/claude-code/issues"
                .to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_feedback_output() {
        let handler = FeedbackHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("https://github.com/anthropics/claude-code/issues"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
