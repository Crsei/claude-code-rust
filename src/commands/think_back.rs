//! /think-back command -- review model's thinking from the conversation.
//!
//! Scans the conversation messages for any thinking/reasoning blocks
//! (ContentBlock::Thinking) and displays them.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{ContentBlock, Message};

/// Handler for the `/think-back` slash command.
pub struct ThinkBackHandler;

#[async_trait]
impl CommandHandler for ThinkBackHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let limit: usize = args
            .trim()
            .parse()
            .unwrap_or(0); // 0 means show all

        let thinking_blocks = extract_thinking(&ctx.messages);

        if thinking_blocks.is_empty() {
            return Ok(CommandResult::Output(
                "No thinking blocks found in the conversation.\n\
                 Thinking blocks appear when extended thinking is enabled."
                    .to_string(),
            ));
        }

        let displayed: Vec<&ThinkingEntry> = if limit > 0 {
            thinking_blocks.iter().rev().take(limit).collect::<Vec<_>>().into_iter().rev().collect()
        } else {
            thinking_blocks.iter().collect()
        };

        let mut lines = Vec::new();
        lines.push(format!(
            "Thinking blocks ({} total, showing {}):",
            thinking_blocks.len(),
            displayed.len()
        ));

        for (i, entry) in displayed.iter().enumerate() {
            lines.push(String::new());
            lines.push(format!("--- Thinking #{} ---", i + 1));

            // Truncate very long thinking blocks for display.
            let text = &entry.thinking;
            if text.len() > 2000 {
                lines.push(format!("{}...", &text[..2000]));
                lines.push(format!(
                    "(truncated, {} total characters)",
                    text.len()
                ));
            } else {
                lines.push(text.clone());
            }
        }

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

/// A thinking entry extracted from an assistant message.
struct ThinkingEntry {
    thinking: String,
}

/// Extract all thinking blocks from the conversation messages.
fn extract_thinking(messages: &[Message]) -> Vec<ThinkingEntry> {
    let mut entries = Vec::new();

    for msg in messages {
        if let Message::Assistant(assistant) = msg {
            for block in &assistant.content {
                if let ContentBlock::Thinking { thinking, .. } = block {
                    if !thinking.is_empty() {
                        entries.push(ThinkingEntry {
                            thinking: thinking.clone(),
                        });
                    }
                }
            }
        }
    }

    entries
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
    use uuid::Uuid;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    fn make_thinking_message(thinking: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::Thinking {
                    thinking: thinking.to_string(),
                    signature: None,
                },
                ContentBlock::Text {
                    text: "response".to_string(),
                },
            ],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    #[tokio::test]
    async fn test_think_back_no_thinking() {
        let handler = ThinkBackHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No thinking blocks"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_think_back_with_thinking() {
        let handler = ThinkBackHandler;
        let mut ctx = test_ctx();
        ctx.messages.push(make_thinking_message("I need to analyze this code carefully."));
        ctx.messages.push(make_thinking_message("Let me reconsider the approach."));

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Thinking blocks (2 total"));
                assert!(text.contains("analyze this code"));
                assert!(text.contains("reconsider"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_think_back_with_limit() {
        let handler = ThinkBackHandler;
        let mut ctx = test_ctx();
        ctx.messages.push(make_thinking_message("First thought."));
        ctx.messages.push(make_thinking_message("Second thought."));
        ctx.messages.push(make_thinking_message("Third thought."));

        let result = handler.execute("1", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("3 total, showing 1"));
                assert!(text.contains("Third thought"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
