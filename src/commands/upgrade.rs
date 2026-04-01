//! `/upgrade` command -- check for upgrades.
//!
//! Shows the current version and directs users to check GitHub for the latest release.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct UpgradeHandler;

#[async_trait]
impl CommandHandler for UpgradeHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let version = env!("CARGO_PKG_VERSION");
        let lines = vec![
            format!("Current version: cc-rust {}", version),
            String::new(),
            "Check GitHub for the latest version:".to_string(),
            "  https://github.com/anthropics/claude-code".to_string(),
            String::new(),
            "To upgrade, pull the latest source and rebuild:".to_string(),
            "  git pull && cargo build --release".to_string(),
        ];

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
    async fn test_upgrade_output() {
        let handler = UpgradeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current version:"));
                assert!(text.contains("Check GitHub"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
