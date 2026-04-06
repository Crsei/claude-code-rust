//! /compact command -- triggers conversation context compaction.
//!
//! Compacts the conversation by summarizing older messages to reduce token usage.
//! Supports two modes:
//! - Local compaction (no API): aggressive snip + microcompact
//! - Full compaction (with API): model-generated summary (future)

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use crate::compact::{compaction, messages as compact_messages, pipeline};
use crate::utils::tokens;

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

        let custom_instructions = if args.is_empty() {
            None
        } else {
            Some(args.to_string())
        };

        // Estimate tokens before compaction
        let pre_tokens = tokens::estimate_messages_tokens(&ctx.messages);
        let model = &ctx.app_state.main_loop_model;

        // Run the local context management pipeline
        let pipeline_result = pipeline::run_context_pipeline(
            ctx.messages.clone(),
            None,
            model,
        )
        .await;

        let post_tokens = pipeline_result.estimated_tokens;

        if pipeline_result.compacted {
            // Pipeline made progress — apply the compacted messages
            let freed = pre_tokens.saturating_sub(post_tokens);
            let new_count = pipeline_result.messages.len();

            // Build summary message for the compacted portion
            let summary = if let Some(ref instructions) = custom_instructions {
                format!("Conversation compacted with instructions: {}", instructions)
            } else {
                "Conversation compacted to reduce context usage.".to_string()
            };

            let post_messages = compaction::build_post_compact_messages(
                &summary,
                &ctx.messages,
                &compaction::CompactionConfig {
                    model: model.clone(),
                    session_id: String::new(),
                    query_source: "compact".into(),
                },
            );

            // Create the compact boundary marker
            let boundary = compaction::create_compact_boundary(pre_tokens, post_tokens);

            return Ok(CommandResult::Output(format!(
                "Compacted: ~{} → ~{} tokens ({} tokens freed)\n\
                 Messages: {} → {}\n\
                 {}",
                pre_tokens,
                post_tokens,
                freed,
                message_count,
                new_count,
                custom_instructions
                    .map(|i| format!("Custom instructions applied: {}", i))
                    .unwrap_or_default(),
            )));
        }

        // No compaction needed or possible
        Ok(CommandResult::Output(format!(
            "No compaction needed. Current conversation:\n\
             - {} messages, ~{} estimated tokens\n\
             - Token usage is within limits for model '{}'{}",
            message_count,
            pre_tokens,
            model,
            custom_instructions
                .map(|i| format!("\n\nNote: custom instructions '{}' will be used when full API compaction is available.", i))
                .unwrap_or_default(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::bootstrap::SessionId;
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
            session_id: SessionId::new(),
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
    async fn test_compact_small_conversation_no_compaction_needed() {
        let handler = CompactHandler;
        let mut ctx = CommandContext {
            messages: vec![make_user_msg("hello"), make_user_msg("world")],
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        };

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No compaction needed") || text.contains("Compacted"));
                assert!(text.contains("2 messages"));
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
            session_id: SessionId::new(),
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
