//! /status command -- shows session status.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::Message;

pub struct StatusHandler;

#[async_trait]
impl CommandHandler for StatusHandler {
    async fn execute(&self, _args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let message_count = ctx.messages.len();

        let user_count = ctx
            .messages
            .iter()
            .filter(|m| matches!(m, Message::User(_)))
            .count();
        let assistant_count = ctx
            .messages
            .iter()
            .filter(|m| matches!(m, Message::Assistant(_)))
            .count();

        let model = &ctx.app_state.main_loop_model;

        let fast_mode = if ctx.app_state.fast_mode {
            "enabled"
        } else {
            "disabled"
        };

        let effort = ctx
            .app_state
            .effort_value
            .as_deref()
            .unwrap_or("default");

        let permission_mode = format!("{:?}", ctx.app_state.tool_permission_context.mode);

        let mut lines = Vec::new();
        lines.push("Session Status".to_string());
        lines.push("─".repeat(30));
        lines.push(format!("Messages:    {} total ({} user, {} assistant)",
            message_count, user_count, assistant_count));
        lines.push(format!("Model:       {}", model));
        lines.push(format!("Fast mode:   {}", fast_mode));
        lines.push(format!("Effort:      {}", effort));
        lines.push(format!("Permissions: {}", permission_mode));

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

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
    async fn test_status_output() {
        let handler = StatusHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Session Status"));
                assert!(text.contains("Model:"));
                assert!(text.contains("Fast mode:"));
                assert!(text.contains("Effort:"));
                assert!(text.contains("Permissions:"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
