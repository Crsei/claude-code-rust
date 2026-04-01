//! `/passes` command -- referral program information.
//!
//! Displays information about the referral / free passes program.
//! This is a simple informational command — the actual referral
//! backend integration is not yet implemented.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Referral program URL (placeholder).
const REFERRAL_URL: &str = "https://claude.ai/referrals";

pub struct PassesHandler;

#[async_trait]
impl CommandHandler for PassesHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "" | "info" => show_info(),
            "help" => show_info(),
            _ => Ok(CommandResult::Output(format!(
                "Unknown argument: '{}'\n\
                 Usage: /passes — show referral program information",
                subcmd
            ))),
        }
    }
}

/// Display referral program information.
fn show_info() -> Result<CommandResult> {
    Ok(CommandResult::Output(format!(
        "Referral Program — Share Free Passes\n\
         ====================================\n\n\
         Share Claude Code with friends and colleagues! When they sign up\n\
         using your referral link, both of you receive free usage passes.\n\n\
         How it works:\n\
         1. Visit your referral dashboard: {}\n\
         2. Copy your unique referral link\n\
         3. Share it with others\n\
         4. Both you and the referee receive free passes when they sign up\n\n\
         Note: Referral program availability depends on your account type\n\
         and region. Visit the URL above for current details.",
        REFERRAL_URL
    )))
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
    async fn test_passes_info() {
        let handler = PassesHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Referral Program"));
                assert!(text.contains("Share"));
                assert!(text.contains(REFERRAL_URL));
                assert!(text.contains("free passes"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_passes_help() {
        let handler = PassesHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("help", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Referral Program"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_passes_unknown_arg() {
        let handler = PassesHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown argument")),
            _ => panic!("Expected Output"),
        }
    }
}
