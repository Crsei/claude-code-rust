//! `/effort` command — set the thinking effort level.
//!
//! Controls the reasoning depth for the model. Valid values are
//! "low", "medium", "high", or a numeric budget token count.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct EffortHandler;

const VALID_LEVELS: &[&str] = &["low", "medium", "high"];

#[async_trait]
impl CommandHandler for EffortHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        if arg.is_empty() {
            let current = ctx
                .app_state
                .effort_value
                .as_deref()
                .unwrap_or("(not set — default)");

            return Ok(CommandResult::Output(format!(
                "Current effort level: {}\n\n\
                 Usage: /effort <level>\n\
                 Valid levels: low, medium, high",
                current
            )));
        }

        if VALID_LEVELS.contains(&arg.as_str()) {
            ctx.app_state.effort_value = Some(arg.clone());
            Ok(CommandResult::Output(format!(
                "Effort level set to: {}",
                arg
            )))
        } else {
            Ok(CommandResult::Output(format!(
                "Invalid effort level: '{}'\nValid levels: {}",
                arg,
                VALID_LEVELS.join(", ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_effort_no_args_shows_current() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current effort level"));
                assert!(text.contains("not set"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_effort_set_valid_level() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("high", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("high")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
    }

    #[tokio::test]
    async fn test_effort_invalid_level() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("ultra", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Invalid"));
                assert!(text.contains("ultra"));
            }
            _ => panic!("Expected Output"),
        }
        assert!(ctx.app_state.effort_value.is_none());
    }

    #[tokio::test]
    async fn test_effort_case_insensitive() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let _ = handler.execute("HIGH", &mut ctx).await.unwrap();
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
    }
}
