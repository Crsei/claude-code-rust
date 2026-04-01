//! `/btw` command -- quick side question.
//!
//! Sends a concise side question to the model without disrupting the
//! main conversation flow. The model is instructed to answer briefly.
//!
//! Usage: /btw <question>

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

pub struct BtwHandler;

/// System-level instruction prepended to the side question.
const SIDE_QUESTION_SYSTEM: &str =
    "The user has a quick side question. Answer it concisely without \
     disrupting the main conversation flow.";

#[async_trait]
impl CommandHandler for BtwHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let question = args.trim();

        if question.is_empty() {
            return Ok(CommandResult::Output(
                "Usage: /btw <question>\n\n\
                 Ask a quick side question without disrupting the conversation.\n\
                 Example: /btw what does RAII stand for?"
                    .to_string(),
            ));
        }

        // System instruction as a meta user message
        let system_msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text(SIDE_QUESTION_SYSTEM.to_string()),
            timestamp: chrono::Utc::now().timestamp(),
            is_meta: true,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });

        // The actual user question
        let user_msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text(question.to_string()),
            timestamp: chrono::Utc::now().timestamp(),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });

        Ok(CommandResult::Query(vec![system_msg, user_msg]))
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
    async fn test_btw_no_args_shows_usage() {
        let handler = BtwHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage"));
                assert!(text.contains("/btw"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_btw_with_question() {
        let handler = BtwHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("what does RAII stand for?", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Query(messages) => {
                assert_eq!(messages.len(), 2);

                // First message should be the system instruction (meta)
                if let Message::User(ref u) = messages[0] {
                    assert!(u.is_meta);
                    if let MessageContent::Text(ref t) = u.content {
                        assert!(t.contains("side question"));
                    } else {
                        panic!("Expected Text content");
                    }
                } else {
                    panic!("Expected User message");
                }

                // Second message should be the user question
                if let Message::User(ref u) = messages[1] {
                    assert!(!u.is_meta);
                    if let MessageContent::Text(ref t) = u.content {
                        assert_eq!(t, "what does RAII stand for?");
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
    async fn test_btw_whitespace_only_shows_usage() {
        let handler = BtwHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("   ", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Usage")),
            _ => panic!("Expected Output"),
        }
    }
}
