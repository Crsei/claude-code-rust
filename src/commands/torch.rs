//! `/torch` command -- hand off context to another session.
//!
//! Torch passes the current conversation context to a new or existing
//! session, enabling seamless handoff between Claude Code instances.
//! This is a feature-gated capability that requires multi-session support.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct TorchHandler;

#[async_trait]
impl CommandHandler for TorchHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();

        if trimmed.is_empty() {
            return Ok(CommandResult::Output(
                "Torch passes your current context to a new or existing session.\n\n\
                 Usage: /torch <session-id-or-description>\n\n\
                 Examples:\n  \
                   /torch abc-123          Hand off to session abc-123\n  \
                   /torch \"fix the tests\"  Create a new session with context\n\n\
                 The receiving session gets:\n  \
                   - Summary of the current conversation\n  \
                   - List of files modified in this session\n  \
                   - Active task context and goals\n  \
                   - Relevant code snippets\n\n\
                 Note: This feature requires multi-session support."
                    .to_string(),
            ));
        }

        Ok(CommandResult::Output(format!(
            "Torch handoff initiated for: {}\n\
             (Feature requires multi-session support)\n\n\
             In a full installation, this would:\n  \
               1. Serialize current conversation context\n  \
               2. Transfer relevant state to the target session\n  \
               3. Provide the receiving session with a briefing\n\n\
             Multi-session support is not available in standalone mode.",
            trimmed
        )))
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
    async fn test_no_args_shows_usage() {
        let handler = TorchHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage: /torch <session-id-or-description>"));
                assert!(text.contains("multi-session support"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_torch_with_args() {
        let handler = TorchHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("abc-123", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Torch handoff initiated for: abc-123"));
                assert!(text.contains("Feature requires multi-session support"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
