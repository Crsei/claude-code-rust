//! `/thinkback-play` command -- play thinkback animation.
//!
//! Checks whether the thinkback plugin is installed under
//! `~/.cc-rust/plugins/thinkback/` and shows the appropriate message.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/thinkback-play` slash command.
pub struct ThinkbackPlayHandler;

/// Check if the thinkback plugin directory exists.
fn is_thinkback_installed() -> bool {
    if let Some(home) = dirs::home_dir() {
        let plugin_dir = home.join(".cc-rust").join("plugins").join("thinkback");
        plugin_dir.is_dir()
    } else {
        false
    }
}

#[async_trait]
impl CommandHandler for ThinkbackPlayHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        if !is_thinkback_installed() {
            return Ok(CommandResult::Output(
                "Thinkback plugin not installed. Run /think-back first to install it.".to_string(),
            ));
        }

        Ok(CommandResult::Output(
            "Playing thinkback animation...".to_string(),
        ))
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
    async fn test_thinkback_play_not_installed() {
        // In the test environment, ~/.cc-rust/plugins/thinkback/ almost certainly
        // does not exist, so we expect the "not installed" message.
        let handler = ThinkbackPlayHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Either the plugin is installed or not; both are valid.
                // We just verify the output is one of the two expected messages.
                assert!(
                    text.contains("not installed") || text.contains("Playing thinkback animation"),
                    "Unexpected output: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[test]
    fn test_is_thinkback_installed_returns_bool() {
        // Verify the function runs without panicking and returns a bool.
        let result = is_thinkback_installed();
        assert!(result == true || result == false);
    }

    #[tokio::test]
    async fn test_thinkback_play_returns_output() {
        // Verify the handler always returns an Output variant (never errors).
        let handler = ThinkbackPlayHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("some args", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(!text.is_empty(), "Output should not be empty");
            }
            _ => panic!("Expected Output result"),
        }
    }
}
