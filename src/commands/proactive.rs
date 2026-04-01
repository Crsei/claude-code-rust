//! `/proactive` command -- toggle proactive suggestions.
//!
//! When enabled, the assistant proactively suggests improvements,
//! potential issues, and related changes beyond what was explicitly asked.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global proactive suggestions flag.
static PROACTIVE_MODE: AtomicBool = AtomicBool::new(false);

pub struct ProactiveHandler;

#[async_trait]
impl CommandHandler for ProactiveHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => {
                PROACTIVE_MODE.store(true, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Proactive suggestions enabled. The assistant will proactively \
                     suggest improvements and related changes."
                        .to_string(),
                ))
            }
            "off" | "disable" => {
                PROACTIVE_MODE.store(false, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Proactive suggestions disabled. The assistant will only respond \
                     to explicit requests."
                        .to_string(),
                ))
            }
            "status" => {
                let status = if PROACTIVE_MODE.load(Ordering::SeqCst) {
                    "enabled"
                } else {
                    "disabled"
                };
                Ok(CommandResult::Output(format!(
                    "Proactive suggestions: {}",
                    status
                )))
            }
            "" => {
                let was_enabled = PROACTIVE_MODE.fetch_xor(true, Ordering::SeqCst);
                let now = if was_enabled { "disabled" } else { "enabled" };
                Ok(CommandResult::Output(format!(
                    "Proactive suggestions {}.",
                    now
                )))
            }
            _ => Ok(CommandResult::Output(
                "Usage: /proactive [on|off|status]\n\n\
                 Toggles proactive suggestions without arguments."
                    .to_string(),
            )),
        }
    }
}

/// Check if proactive mode is currently enabled. Exposed for the query loop.
pub fn is_proactive_mode() -> bool {
    PROACTIVE_MODE.load(Ordering::SeqCst)
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
    async fn test_enable() {
        let handler = ProactiveHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_disable() {
        let handler = ProactiveHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_status() {
        let handler = ProactiveHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Proactive"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
