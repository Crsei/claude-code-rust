//! /rewind command -- remove the last N message pairs.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct RewindHandler;

#[async_trait]
impl CommandHandler for RewindHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim();

        let count: usize = if arg.is_empty() {
            1
        } else {
            match arg.parse::<usize>() {
                Ok(0) => {
                    return Ok(CommandResult::Output(
                        "Nothing to rewind (count is 0).".to_string(),
                    ));
                }
                Ok(n) => n,
                Err(_) => {
                    return Ok(CommandResult::Output(format!(
                        "Invalid count: '{}'\nUsage: /rewind [N]  (default: 1)",
                        arg
                    )));
                }
            }
        };

        let to_remove = count * 2;

        if ctx.messages.is_empty() {
            return Ok(CommandResult::Output(
                "No messages to rewind.".to_string(),
            ));
        }

        let actually_removed = to_remove.min(ctx.messages.len());
        let new_len = ctx.messages.len() - actually_removed;
        ctx.messages.truncate(new_len);

        let pairs_removed = (actually_removed + 1) / 2;

        Ok(CommandResult::Output(format!(
            "Rewound {} message pair(s) ({} messages removed). {} messages remaining.",
            pairs_removed,
            actually_removed,
            ctx.messages.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use crate::types::message::{
        AssistantMessage, ContentBlock, Message, MessageContent, UserMessage,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_ctx_with_messages(n: usize) -> CommandContext {
        let mut messages = Vec::new();
        for i in 0..n {
            if i % 2 == 0 {
                messages.push(Message::User(UserMessage {
                    uuid: Uuid::new_v4(),
                    timestamp: i as i64,
                    role: "user".into(),
                    content: MessageContent::Text(format!("user msg {}", i)),
                    is_meta: false,
                    tool_use_result: None,
                    source_tool_assistant_uuid: None,
                }));
            } else {
                messages.push(Message::Assistant(AssistantMessage {
                    uuid: Uuid::new_v4(),
                    timestamp: i as i64,
                    role: "assistant".into(),
                    content: vec![ContentBlock::Text {
                        text: format!("assistant msg {}", i),
                    }],
                    usage: None,
                    stop_reason: None,
                    is_api_error_message: false,
                    api_error: None,
                    cost_usd: 0.0,
                }));
            }
        }

        CommandContext {
            messages,
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_rewind_default() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(6);
        assert_eq!(ctx.messages.len(), 6);

        let result = handler.execute("", &mut ctx).await.unwrap();
        assert_eq!(ctx.messages.len(), 4);
        match result {
            CommandResult::Output(text) => assert!(text.contains("1 message pair")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_rewind_multiple() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(10);
        assert_eq!(ctx.messages.len(), 10);

        let result = handler.execute("3", &mut ctx).await.unwrap();
        assert_eq!(ctx.messages.len(), 4);
        match result {
            CommandResult::Output(text) => assert!(text.contains("3 message pair")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_rewind_empty() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(0);
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("No messages")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_rewind_zero() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(4);
        let result = handler.execute("0", &mut ctx).await.unwrap();
        assert_eq!(ctx.messages.len(), 4);
        match result {
            CommandResult::Output(text) => assert!(text.contains("Nothing to rewind")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_rewind_invalid() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(4);
        let result = handler.execute("abc", &mut ctx).await.unwrap();
        assert_eq!(ctx.messages.len(), 4);
        match result {
            CommandResult::Output(text) => assert!(text.contains("Invalid count")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_rewind_more_than_available() {
        let handler = RewindHandler;
        let mut ctx = test_ctx_with_messages(2);
        let result = handler.execute("5", &mut ctx).await.unwrap();
        assert_eq!(ctx.messages.len(), 0);
        match result {
            CommandResult::Output(text) => assert!(text.contains("0 messages remaining")),
            _ => panic!("Expected Output"),
        }
    }
}
