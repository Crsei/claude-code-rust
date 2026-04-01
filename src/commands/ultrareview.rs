//! `/ultrareview` command -- remote bug finder (requires Claude Code on the web).
//!
//! Ultra review is a long-running remote analysis feature that requires
//! Claude Code on the web (CCR). In standalone mode we show an informational
//! message pointing users to `/review` for local code review.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/ultrareview` slash command.
pub struct UltrareviewHandler;

#[async_trait]
impl CommandHandler for UltrareviewHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut lines = Vec::new();
        lines.push("Ultra review runs in Claude Code on the web (~10-20 min bug finder).".to_string());
        lines.push(String::new());
        lines.push(
            "This feature requires Claude Code on the web. Use /review for local code review."
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
    async fn test_ultrareview_shows_info() {
        let handler = UltrareviewHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Ultra review runs in Claude Code on the web"));
                assert!(text.contains("~10-20 min bug finder"));
                assert!(text.contains("Use /review for local code review"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_ultrareview_ignores_args() {
        let handler = UltrareviewHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("src/main.rs --deep", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("This feature requires Claude Code on the web"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
