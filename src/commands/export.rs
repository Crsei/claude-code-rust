//! `/export` command — export the current conversation to a file.
//!
//! Saves conversation messages as JSON or Markdown to a specified
//! file path. Defaults to JSON if no format is specified.

#![allow(unused)]

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct ExportHandler;

#[async_trait]
impl CommandHandler for ExportHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if ctx.messages.is_empty() {
            return Ok(CommandResult::Output(
                "No messages to export.".to_string(),
            ));
        }

        // Determine output format and path
        let (path, format) = if args.is_empty() {
            let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            (format!("conversation_{}.json", ts), ExportFormat::Json)
        } else if args.ends_with(".md") || args.ends_with(".markdown") {
            (args.to_string(), ExportFormat::Markdown)
        } else if args.ends_with(".json") {
            (args.to_string(), ExportFormat::Json)
        } else {
            (format!("{}.json", args), ExportFormat::Json)
        };

        let full_path = if std::path::Path::new(&path).is_absolute() {
            std::path::PathBuf::from(&path)
        } else {
            ctx.cwd.join(&path)
        };

        let content = match format {
            ExportFormat::Json => export_json(&ctx.messages)?,
            ExportFormat::Markdown => export_markdown(&ctx.messages),
        };

        std::fs::write(&full_path, &content)
            .with_context(|| format!("Failed to write to {}", full_path.display()))?;

        Ok(CommandResult::Output(format!(
            "Exported {} message(s) to {}",
            ctx.messages.len(),
            full_path.display()
        )))
    }
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Json,
    Markdown,
}

fn export_json(
    messages: &[crate::types::message::Message],
) -> Result<String> {
    use crate::types::message::{Message, MessageContent};

    let entries: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| match m {
            Message::User(u) => {
                serde_json::json!({
                    "role": "user",
                    "uuid": u.uuid.to_string(),
                    "timestamp": u.timestamp,
                    "content": message_content_to_text(&u.content),
                })
            }
            Message::Assistant(a) => {
                let content = MessageContent::Blocks(a.content.clone());
                serde_json::json!({
                    "role": "assistant",
                    "content": message_content_to_text(&content),
                })
            }
            Message::System(s) => {
                serde_json::json!({
                    "role": "system",
                    "content": s.content,
                })
            }
            _ => serde_json::json!({ "role": "other" }),
        })
        .collect();

    serde_json::to_string_pretty(&entries).context("Failed to serialize messages")
}

fn export_markdown(messages: &[crate::types::message::Message]) -> String {
    use crate::types::message::{Message, MessageContent};

    let mut lines = Vec::new();
    lines.push("# Conversation Export".to_string());
    lines.push(String::new());

    for msg in messages {
        match msg {
            Message::User(u) => {
                lines.push("## User".to_string());
                lines.push(String::new());
                lines.push(message_content_to_text(&u.content));
                lines.push(String::new());
            }
            Message::Assistant(a) => {
                let content = MessageContent::Blocks(a.content.clone());
                lines.push("## Assistant".to_string());
                lines.push(String::new());
                lines.push(message_content_to_text(&content));
                lines.push(String::new());
            }
            Message::System(s) => {
                lines.push("## System".to_string());
                lines.push(String::new());
                lines.push(s.content.clone());
                lines.push(String::new());
            }
            _ => {}
        }
    }

    lines.join("\n")
}

fn message_content_to_text(
    content: &crate::types::message::MessageContent,
) -> String {
    use crate::types::message::{MessageContent, ContentBlock};
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Blocks(blocks) => {
            blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}
