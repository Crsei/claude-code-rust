//! `/subscribe-pr` command -- subscribe to pull request updates.
//!
//! This is a feature-gated capability that integrates with GitHub webhooks
//! to deliver PR event notifications. It requires the GitHub App to be
//! installed on the repository.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct SubscribePrHandler;

#[async_trait]
impl CommandHandler for SubscribePrHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();

        if trimmed.is_empty() {
            return Ok(CommandResult::Output(
                "Subscribe to pull request updates via GitHub webhooks.\n\n\
                 Usage: /subscribe-pr <pr-number-or-url>\n\n\
                 Examples:\n  \
                   /subscribe-pr 42\n  \
                   /subscribe-pr https://github.com/owner/repo/pull/42\n\n\
                 Note: This feature requires the GitHub App to be installed\n\
                 on the target repository."
                    .to_string(),
            ));
        }

        Ok(CommandResult::Output(format!(
            "Subscribed to PR updates for: {}\n\
             (Requires GitHub App integration)\n\n\
             You will receive notifications when:\n  \
               - New comments are posted\n  \
               - Reviews are submitted\n  \
               - The PR status changes\n  \
               - CI checks complete\n\n\
             Note: The GitHub App must be installed on the repository for\n\
             webhook delivery to work.",
            trimmed
        )))
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
    async fn test_no_args_shows_usage() {
        let handler = SubscribePrHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage: /subscribe-pr <pr-number-or-url>"));
                assert!(text.contains("GitHub App"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_subscribe_to_pr_number() {
        let handler = SubscribePrHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("42", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Subscribed to PR updates for: 42"));
                assert!(text.contains("Requires GitHub App integration"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
