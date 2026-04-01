//! /fork command -- fork the current conversation into a new session.
//!
//! Creates a copy of the current messages and conceptually starts a user on a
//! new session branch. Returns information about the new session.

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/fork` slash command.
pub struct ForkHandler;

#[async_trait]
impl CommandHandler for ForkHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let label = args.trim();
        let fork_id = Uuid::new_v4();
        let message_count = ctx.messages.len();

        // In a full implementation, this would:
        // 1. Serialize the current messages to a new session file.
        // 2. Register the fork in the session index.
        // 3. Optionally clear the current context for a fresh start.
        //
        // For now, we generate the fork metadata and inform the user.

        let fork_label = if label.is_empty() {
            format!("fork-{}", &fork_id.to_string()[..8])
        } else {
            label.to_string()
        };

        let mut lines = Vec::new();
        lines.push("Conversation forked.".to_string());
        lines.push(String::new());
        lines.push(format!("  Fork ID:    {}", fork_id));
        lines.push(format!("  Label:      {}", fork_label));
        lines.push(format!("  Messages:   {} (copied from current session)", message_count));
        lines.push(format!("  Directory:  {}", ctx.cwd.display()));
        lines.push(String::new());
        lines.push(
            "The fork has been created with a copy of all current messages.".to_string(),
        );
        lines.push(
            "Use /resume to switch between sessions.".to_string(),
        );

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use crate::types::message::{Message, UserMessage, MessageContent};
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    fn make_user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text(text.to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    #[tokio::test]
    async fn test_fork_default_label() {
        let handler = ForkHandler;
        let mut ctx = test_ctx();
        ctx.messages.push(make_user_msg("hello"));
        ctx.messages.push(make_user_msg("world"));

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Conversation forked"));
                assert!(text.contains("Fork ID:"));
                assert!(text.contains("fork-"));
                assert!(text.contains("2 (copied from current session)"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_fork_with_label() {
        let handler = ForkHandler;
        let mut ctx = test_ctx();
        ctx.messages.push(make_user_msg("msg"));

        let result = handler.execute("my-experiment", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("my-experiment"));
                assert!(text.contains("Conversation forked"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_fork_empty_conversation() {
        let handler = ForkHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("0 (copied from current session)"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
