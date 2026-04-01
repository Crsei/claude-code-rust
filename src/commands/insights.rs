//! `/insights` command -- session analysis.
//!
//! Asks the model to analyze the current conversation and provide
//! insights about the session: summary, key decisions, potential
//! improvements, and patterns observed.
//!
//! Usage:
//!   /insights              — general session analysis
//!   /insights <focus>      — analysis focused on specific areas

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

pub struct InsightsHandler;

#[async_trait]
impl CommandHandler for InsightsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let focus = args.trim();

        let message_count = ctx.messages.len();
        let context_note = if message_count == 0 {
            return Ok(CommandResult::Output(
                "No conversation history to analyze. Start a conversation first."
                    .to_string(),
            ));
        } else {
            format!("The conversation so far contains {} message(s).", message_count)
        };

        let focus_section = if focus.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nPlease pay special attention to the following focus areas: {}",
                focus
            )
        };

        let prompt = format!(
            "Please analyze the current conversation session and provide insights. {}\n\n\
             Include the following in your analysis:\n\
             1. **Session Summary** — What has been accomplished so far\n\
             2. **Key Decisions** — Important choices or directions taken\n\
             3. **Potential Improvements** — Things that could be done better or next steps\n\
             4. **Patterns Observed** — Recurring themes, approaches, or issues{}",
            context_note, focus_section
        );

        let msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text(prompt),
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
    use crate::types::message::{AssistantMessage, ContentBlock};
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    fn test_ctx_with_messages() -> CommandContext {
        let user_msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text("Hello".to_string()),
            timestamp: 1000,
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });

        let assistant_msg = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: "Hi there!".to_string(),
            }],
            timestamp: 1001,
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        });

        CommandContext {
            messages: vec![user_msg, assistant_msg],
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_insights_empty_conversation() {
        let handler = InsightsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No conversation history"));
            }
            _ => panic!("Expected Output for empty conversation"),
        }
    }

    #[tokio::test]
    async fn test_insights_with_conversation() {
        let handler = InsightsHandler;
        let mut ctx = test_ctx_with_messages();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(messages) => {
                assert_eq!(messages.len(), 1);
                if let Message::User(ref u) = messages[0] {
                    if let MessageContent::Text(ref t) = u.content {
                        assert!(t.contains("Session Summary"));
                        assert!(t.contains("Key Decisions"));
                        assert!(t.contains("Potential Improvements"));
                        assert!(t.contains("Patterns Observed"));
                        assert!(t.contains("2 message(s)"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }
            }
            _ => panic!("Expected Query"),
        }
    }

    #[tokio::test]
    async fn test_insights_with_focus() {
        let handler = InsightsHandler;
        let mut ctx = test_ctx_with_messages();
        let result = handler
            .execute("error handling and testing", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Query(messages) => {
                assert_eq!(messages.len(), 1);
                if let Message::User(ref u) = messages[0] {
                    if let MessageContent::Text(ref t) = u.content {
                        assert!(t.contains("error handling and testing"));
                        assert!(t.contains("focus areas"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }
            }
            _ => panic!("Expected Query"),
        }
    }
}
