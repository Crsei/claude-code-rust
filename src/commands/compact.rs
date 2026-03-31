//! /compact command -- triggers conversation context compaction.
//!
//! Context compaction summarizes older messages to reduce token usage while
//! preserving important context. This requires an API call to generate the
//! summary, so the current stub implementation returns an informational
//! message until the API client is wired up.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/compact` slash command.
pub struct CompactHandler;

#[async_trait]
impl CommandHandler for CompactHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let message_count = ctx.messages.len();

        if message_count == 0 {
            return Ok(CommandResult::Output(
                "Nothing to compact -- conversation is empty.".into(),
            ));
        }

        // In a full implementation this would:
        // 1. Take all messages before a cut-off point
        // 2. Send them to the model with a "summarize this conversation" prompt
        // 3. Replace the old messages with a single summary message
        // 4. Insert a CompactBoundary system message
        //
        // For now, we report that compaction is not yet available.

        let custom_instructions = if args.is_empty() {
            None
        } else {
            Some(args.to_string())
        };

        Ok(CommandResult::Output(format!(
            "Compaction is not available without an API connection.\n\
             Current conversation has {} message(s).{}",
            message_count,
            custom_instructions
                .map(|i| format!("\nCustom compaction instructions: {}", i))
                .unwrap_or_default(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::types::app_state::AppState;
    use crate::types::message::{Message, UserMessage, MessageContent};
    use uuid::Uuid;

    fn make_user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".into(),
            content: MessageContent::Text(text.into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    #[tokio::test]
    async fn test_compact_empty_conversation() {
        let handler = CompactHandler;
        let mut ctx = CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        };

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("empty"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_compact_with_messages() {
        let handler = CompactHandler;
        let mut ctx = CommandContext {
            messages: vec![make_user_msg("hello"), make_user_msg("world")],
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        };

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("2 message(s)"));
                assert!(text.contains("not available"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_compact_with_custom_instructions() {
        let handler = CompactHandler;
        let mut ctx = CommandContext {
            messages: vec![make_user_msg("hello")],
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        };

        let result = handler.execute("focus on code changes", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("focus on code changes"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
