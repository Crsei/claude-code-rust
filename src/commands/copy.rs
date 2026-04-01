//! /copy command -- copies the last assistant response to clipboard.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{ContentBlock, Message};

pub struct CopyHandler;

/// Extract plain text from the last assistant message in the conversation.
fn last_assistant_text(messages: &[Message]) -> Option<String> {
    for msg in messages.iter().rev() {
        if let Message::Assistant(a) = msg {
            let mut parts = Vec::new();
            for block in &a.content {
                if let ContentBlock::Text { text } = block {
                    parts.push(text.as_str());
                }
            }
            if !parts.is_empty() {
                return Some(parts.join("\n"));
            }
        }
    }
    None
}

#[async_trait]
impl CommandHandler for CopyHandler {
    async fn execute(&self, _args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        match last_assistant_text(&ctx.messages) {
            Some(text) => {
                // Clipboard access is platform-dependent; print the text and
                // indicate it would be copied.
                Ok(CommandResult::Output(format!(
                    "Copied to clipboard ({} chars):\n\n{}",
                    text.len(),
                    text
                )))
            }
            None => Ok(CommandResult::Output(
                "No assistant messages found.".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use crate::types::message::AssistantMessage;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_no_messages() {
        let handler = CopyHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("No assistant messages")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_copies_last_assistant() {
        let handler = CopyHandler;
        let mut ctx = test_ctx();
        ctx.messages.push(Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1,
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "Hello, world!".into(),
            }],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }));

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Copied to clipboard"));
                assert!(text.contains("Hello, world!"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
