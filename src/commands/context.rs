//! /context command -- display context usage information.
//!
//! Shows an overview of the conversation context: message count, estimated
//! token usage, and model information. In the TypeScript version this calls
//! `analyzeContextUsage()` with full system prompt analysis. The Rust CLI
//! provides a simplified local estimate since full token counting requires
//! an API connection.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::Message;

/// Handler for the `/context` slash command.
pub struct ContextHandler;

/// Rough estimate of tokens in a message for display purposes.
///
/// Uses a simple heuristic of ~4 characters per token. The real
/// implementation requires a tokenizer (tiktoken / API-based counting).
fn estimate_message_tokens(msg: &Message) -> u64 {
    let text_len = match msg {
        Message::User(u) => match &u.content {
            crate::types::message::MessageContent::Text(t) => t.len(),
            crate::types::message::MessageContent::Blocks(blocks) => {
                blocks.iter().map(|b| estimate_block_chars(b)).sum()
            }
        },
        Message::Assistant(a) => a
            .content
            .iter()
            .map(|b| estimate_block_chars(b))
            .sum(),
        Message::System(s) => s.content.len(),
        Message::Progress(p) => p.data.to_string().len(),
        Message::Attachment(a) => {
            // Rough estimate for attachment metadata.
            50
        }
    };

    // ~4 chars per token is a common rough estimate.
    (text_len as u64 / 4).max(1)
}

/// Estimate character count for a content block.
fn estimate_block_chars(block: &crate::types::message::ContentBlock) -> usize {
    match block {
        crate::types::message::ContentBlock::Text { text } => text.len(),
        crate::types::message::ContentBlock::ToolUse { name, input, .. } => {
            name.len() + input.to_string().len()
        }
        crate::types::message::ContentBlock::ToolResult { content, .. } => match content {
            crate::types::message::ToolResultContent::Text(t) => t.len(),
            crate::types::message::ToolResultContent::Blocks(blocks) => {
                blocks.iter().map(|b| estimate_block_chars(b)).sum()
            }
        },
        crate::types::message::ContentBlock::Thinking { thinking, .. } => thinking.len(),
        crate::types::message::ContentBlock::RedactedThinking { data } => data.len(),
        crate::types::message::ContentBlock::Image { .. } => 1000, // Images use many tokens.
    }
}

/// Format a token count for display.
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[async_trait]
impl CommandHandler for ContextHandler {
    async fn execute(&self, _args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let messages = &ctx.messages;
        let model = &ctx.app_state.main_loop_model;

        if messages.is_empty() {
            return Ok(CommandResult::Output(
                "Context is empty -- no messages in the conversation.".into(),
            ));
        }

        let mut user_msgs = 0u64;
        let mut assistant_msgs = 0u64;
        let mut system_msgs = 0u64;
        let mut total_tokens: u64 = 0;

        for msg in messages {
            match msg {
                Message::User(_) => user_msgs += 1,
                Message::Assistant(_) => assistant_msgs += 1,
                Message::System(_) => system_msgs += 1,
                _ => {}
            }
            total_tokens += estimate_message_tokens(msg);
        }

        let mut lines = Vec::new();
        lines.push("## Context Usage".into());
        lines.push(String::new());
        lines.push(format!("**Model:** {}", model));
        lines.push(format!(
            "**Tokens:** ~{} (estimated, local heuristic)",
            format_tokens(total_tokens)
        ));
        lines.push(String::new());
        lines.push("### Message breakdown".into());
        lines.push(String::new());
        lines.push(format!("  User messages:      {}", user_msgs));
        lines.push(format!("  Assistant messages:  {}", assistant_msgs));
        lines.push(format!("  System messages:     {}", system_msgs));
        lines.push(format!("  Total messages:      {}", messages.len()));
        lines.push(String::new());
        lines.push(
            "Note: Accurate token counts require an API connection. \
             Counts shown here are rough estimates."
                .into(),
        );

        Ok(CommandResult::Output(lines.join("\n")))
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

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_context_empty() {
        let handler = ContextHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("empty"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_context_with_messages() {
        let handler = ContextHandler;
        let mut ctx = test_ctx();
        ctx.messages = vec![
            make_user_msg("Hello, world!"),
            make_user_msg("Another message here."),
        ];
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Context Usage"));
                assert!(text.contains("User messages:"));
                assert!(text.contains("2"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }
}
