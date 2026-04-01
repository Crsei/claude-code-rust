//! /force-snip command -- force snip conversation history.
//!
//! Removes all but the last N messages (default 8, i.e. 4 user-assistant pairs).
//! This is more aggressive than /compact -- it discards messages outright rather
//! than summarizing them.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Default number of messages to keep.
const DEFAULT_KEEP: usize = 8;

/// Handler for the `/force-snip` slash command.
pub struct ForceSnipHandler;

#[async_trait]
impl CommandHandler for ForceSnipHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let keep: usize = args
            .trim()
            .parse()
            .unwrap_or(DEFAULT_KEEP);

        if keep == 0 {
            return Ok(CommandResult::Output(
                "Cannot keep 0 messages. Use /clear to remove all history.".to_string(),
            ));
        }

        let total = ctx.messages.len();

        if total <= keep {
            return Ok(CommandResult::Output(format!(
                "Conversation has only {} message(s), nothing to snip (keeping {}).",
                total, keep
            )));
        }

        let removed = total - keep;
        ctx.messages = ctx.messages.split_off(removed);

        Ok(CommandResult::Output(format!(
            "Snipped {} message(s) from history. {} message(s) remaining.",
            removed,
            ctx.messages.len()
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use crate::types::message::{Message, UserMessage, MessageContent};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    fn make_user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text(text.to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    #[tokio::test]
    async fn test_force_snip_default() {
        let handler = ForceSnipHandler;
        let mut ctx = test_ctx();

        // Add 12 messages.
        for i in 0..12 {
            ctx.messages.push(make_user_msg(&format!("msg {}", i)));
        }

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Snipped 4 message(s)"));
                assert!(text.contains("8 message(s) remaining"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.messages.len(), 8);
    }

    #[tokio::test]
    async fn test_force_snip_custom_keep() {
        let handler = ForceSnipHandler;
        let mut ctx = test_ctx();

        for i in 0..10 {
            ctx.messages.push(make_user_msg(&format!("msg {}", i)));
        }

        let result = handler.execute("3", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Snipped 7 message(s)"));
                assert!(text.contains("3 message(s) remaining"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.messages.len(), 3);
    }

    #[tokio::test]
    async fn test_force_snip_nothing_to_snip() {
        let handler = ForceSnipHandler;
        let mut ctx = test_ctx();

        ctx.messages.push(make_user_msg("only one"));

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("nothing to snip"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_force_snip_zero_keep() {
        let handler = ForceSnipHandler;
        let mut ctx = test_ctx();

        ctx.messages.push(make_user_msg("msg"));

        let result = handler.execute("0", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Cannot keep 0"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
