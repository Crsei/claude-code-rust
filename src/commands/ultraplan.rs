//! `/ultraplan` command -- remote multi-agent planning (requires Claude Code on the web).
//!
//! Ultra plan is a long-running multi-agent exploration feature that requires
//! Claude Code on the web (CCR). In standalone mode we show an informational
//! message pointing users to `/plan` for local planning mode.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/ultraplan` slash command.
pub struct UltraplanHandler;

#[async_trait]
impl CommandHandler for UltraplanHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut lines = Vec::new();
        lines.push(
            "Ultra plan uses multi-agent exploration on Claude Code on the web (~30 min)."
                .to_string(),
        );
        lines.push(String::new());
        lines.push(
            "This feature requires Claude Code on the web. Use /plan for local planning mode."
                .to_string(),
        );

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    async fn test_ultraplan_shows_info() {
        let handler = UltraplanHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Ultra plan uses multi-agent exploration"));
                assert!(text.contains("~30 min"));
                assert!(text.contains("Use /plan for local planning mode"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_ultraplan_ignores_args() {
        let handler = UltraplanHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("design a new auth system", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("This feature requires Claude Code on the web"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
