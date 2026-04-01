//! `/statusline` command -- set up status line via a subagent.
//!
//! This is a "prompt" type command that returns `CommandResult::Query`
//! asking the model to create a statusline-setup subagent.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

/// Handler for the `/statusline` slash command.
pub struct StatuslineHandler;

#[async_trait]
impl CommandHandler for StatuslineHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let prompt = if args.trim().is_empty() {
            "Configure my statusLine from my shell PS1 configuration".to_string()
        } else {
            args.trim().to_string()
        };

        let message_text = format!(
            "Create an Agent with subagent_type 'statusline-setup' and the prompt '{}'",
            prompt
        );

        let msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text(message_text),
            timestamp: chrono::Utc::now().timestamp(),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });

        Ok(CommandResult::Query(vec![msg]))
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
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_statusline_default_prompt() {
        let handler = StatuslineHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                assert_eq!(msgs.len(), 1);
                if let Message::User(user_msg) = &msgs[0] {
                    if let MessageContent::Text(text) = &user_msg.content {
                        assert!(text.contains("statusline-setup"));
                        assert!(text.contains("Configure my statusLine from my shell PS1 configuration"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }
            }
            _ => panic!("Expected Query result"),
        }
    }

    #[tokio::test]
    async fn test_statusline_custom_prompt() {
        let handler = StatuslineHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("Show git branch and time", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                assert_eq!(msgs.len(), 1);
                if let Message::User(user_msg) = &msgs[0] {
                    if let MessageContent::Text(text) = &user_msg.content {
                        assert!(text.contains("statusline-setup"));
                        assert!(text.contains("Show git branch and time"));
                        assert!(!text.contains("Configure my statusLine from my shell PS1"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }
            }
            _ => panic!("Expected Query result"),
        }
    }

    #[tokio::test]
    async fn test_statusline_trims_whitespace() {
        let handler = StatuslineHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("  custom setup  ", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                if let Message::User(user_msg) = &msgs[0] {
                    if let MessageContent::Text(text) = &user_msg.content {
                        assert!(text.contains("'custom setup'"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }
            }
            _ => panic!("Expected Query result"),
        }
    }
}
