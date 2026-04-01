//! `/brief` command -- toggle brief output mode.
//!
//! Brief mode instructs the assistant to give shorter, more concise responses.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global brief mode flag.
static BRIEF_MODE: AtomicBool = AtomicBool::new(false);

pub struct BriefHandler;

#[async_trait]
impl CommandHandler for BriefHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => {
                BRIEF_MODE.store(true, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Brief mode enabled. Responses will be more concise.".to_string(),
                ))
            }
            "off" | "disable" => {
                BRIEF_MODE.store(false, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Brief mode disabled. Normal response length restored.".to_string(),
                ))
            }
            "status" => {
                let status = if BRIEF_MODE.load(Ordering::SeqCst) {
                    "enabled"
                } else {
                    "disabled"
                };
                Ok(CommandResult::Output(format!("Brief mode: {}", status)))
            }
            "" => {
                let was_enabled = BRIEF_MODE.fetch_xor(true, Ordering::SeqCst);
                let now = if was_enabled { "disabled" } else { "enabled" };
                Ok(CommandResult::Output(format!("Brief mode {}.", now)))
            }
            _ => Ok(CommandResult::Output(
                "Usage: /brief [on|off|status]\n\nToggles brief mode without arguments."
                    .to_string(),
            )),
        }
    }
}

/// Check if brief mode is currently enabled. Exposed for the query loop.
pub fn is_brief_mode() -> bool {
    BRIEF_MODE.load(Ordering::SeqCst)
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
    async fn test_toggle_on() {
        // Reset to known state
        BRIEF_MODE.store(false, Ordering::SeqCst);

        let handler = BriefHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_toggle_off() {
        let handler = BriefHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_status() {
        BRIEF_MODE.store(false, Ordering::SeqCst);

        let handler = BriefHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Brief mode:"));
                assert!(text.contains("disabled"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
