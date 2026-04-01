//! `/buddy` command -- AI buddy companion mode.
//!
//! Buddy mode provides a persistent AI companion that monitors your work,
//! offering proactive suggestions and context-aware assistance. This is
//! a feature-gated capability that is not yet available in standalone mode.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct BuddyHandler;

#[async_trait]
impl CommandHandler for BuddyHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim().to_lowercase();

        match trimmed.as_str() {
            "" => Ok(CommandResult::Output(overview())),
            "on" | "enable" => Ok(CommandResult::Output(
                "Buddy mode is not yet available in standalone mode.\n\n\
                 This feature requires the daemon process to be running\n\
                 in the background to monitor your work."
                    .to_string(),
            )),
            "off" | "disable" => Ok(CommandResult::Output(
                "Buddy mode is not yet available in standalone mode.\n\n\
                 Nothing to disable."
                    .to_string(),
            )),
            "status" => Ok(CommandResult::Output(
                "Buddy mode: not available\n\n\
                 The buddy daemon is not running. Buddy mode requires\n\
                 multi-session support and the background daemon process."
                    .to_string(),
            )),
            _ => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\n{}",
                trimmed,
                overview()
            ))),
        }
    }
}

fn overview() -> String {
    "Buddy mode provides a persistent AI companion that monitors your work.\n\n\
     Usage: /buddy <subcommand>\n\n\
     Subcommands:\n  \
       on | enable     Enable buddy mode\n  \
       off | disable   Disable buddy mode\n  \
       status          Show buddy mode status\n\n\
     When active, the buddy watches your editing activity and offers:\n  \
       - Proactive suggestions for improvements\n  \
       - Warnings about potential issues\n  \
       - Context-aware documentation hints\n  \
       - Automated code review on save\n\n\
     Note: Buddy mode requires the background daemon process and\n\
     is not yet available in standalone mode."
        .to_string()
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
    async fn test_default_shows_overview() {
        let handler = BuddyHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("persistent AI companion that monitors your work"));
                assert!(text.contains("on | enable"));
                assert!(text.contains("off | disable"));
                assert!(text.contains("status"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_enable_not_available() {
        let handler = BuddyHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("not yet available in standalone mode"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
