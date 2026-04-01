//! `/stats` command — show usage statistics for the current session.
//!
//! Displays message counts, token usage, tool call counts, and
//! session duration.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{ContentBlock, Message, MessageContent};

pub struct StatsHandler;

fn count_content(content: &MessageContent) -> (usize, u32, u32) {
    let mut chars = 0usize;
    let mut tool_uses = 0u32;
    let mut tool_results = 0u32;

    match content {
        MessageContent::Text(text) => {
            chars += text.len();
        }
        MessageContent::Blocks(blocks) => {
            for block in blocks {
                match block {
                    ContentBlock::Text { text } => chars += text.len(),
                    ContentBlock::ToolUse { .. } => tool_uses += 1,
                    ContentBlock::ToolResult { .. } => tool_results += 1,
                    _ => {}
                }
            }
        }
    }
    (chars, tool_uses, tool_results)
}

#[async_trait]
impl CommandHandler for StatsHandler {
    async fn execute(&self, _args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut user_messages = 0u32;
        let mut assistant_messages = 0u32;
        let mut tool_uses = 0u32;
        let mut tool_results = 0u32;
        let mut total_text_chars = 0usize;

        for msg in &ctx.messages {
            match msg {
                Message::User(u) => {
                    user_messages += 1;
                    let (chars, tu, tr) = count_content(&u.content);
                    total_text_chars += chars;
                    tool_uses += tu;
                    tool_results += tr;
                }
                Message::Assistant(a) => {
                    assistant_messages += 1;
                    let content = MessageContent::Blocks(a.content.clone());
                    let (chars, tu, tr) = count_content(&content);
                    total_text_chars += chars;
                    tool_uses += tu;
                    tool_results += tr;
                }
                _ => {}
            }
        }

        let total_messages = user_messages + assistant_messages;
        let estimated_tokens = total_text_chars / 4; // rough estimate

        let mut lines = Vec::new();
        lines.push("Session Statistics".to_string());
        lines.push("─".repeat(30));
        lines.push(format!("Messages:     {} total", total_messages));
        lines.push(format!("  User:       {}", user_messages));
        lines.push(format!("  Assistant:  {}", assistant_messages));
        lines.push(format!("Tool uses:    {}", tool_uses));
        lines.push(format!("Tool results: {}", tool_results));
        lines.push(format!("Text chars:   {}", total_text_chars));
        lines.push(format!("Est. tokens:  ~{}", estimated_tokens));

        Ok(CommandResult::Output(lines.join("\n")))
    }
}
