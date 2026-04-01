//! /color command -- toggle color mode.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct ColorHandler;

/// Track color mode in a simple process-level static.
///
/// A full implementation would store this in AppState or settings;
/// for now we use a lightweight atomic flag.
static COLOR_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[async_trait]
impl CommandHandler for ColorHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        match arg.as_str() {
            "on" | "enable" => {
                COLOR_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(CommandResult::Output(
                    "Color mode: enabled".to_string(),
                ))
            }
            "off" | "disable" => {
                COLOR_ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
                Ok(CommandResult::Output(
                    "Color mode: disabled".to_string(),
                ))
            }
            "" => {
                // Toggle
                let was = COLOR_ENABLED.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
                let now = !was;
                let label = if now { "enabled" } else { "disabled" };
                Ok(CommandResult::Output(format!("Color mode: {}", label)))
            }
            _ => Ok(CommandResult::Output(
                "Usage: /color [on|off]\n\nToggles color mode without arguments.".to_string(),
            )),
        }
    }
}

/// Query the current color mode (for use by rendering code).
pub fn is_color_enabled() -> bool {
    COLOR_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
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
    async fn test_toggle_color() {
        let handler = ColorHandler;
        let mut ctx = test_ctx();

        // Toggle returns a result containing "enabled" or "disabled"
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("enabled") || text.contains("disabled"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_explicit_on_off() {
        let handler = ColorHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }

        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_unknown_arg() {
        let handler = ColorHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("rainbow", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Usage")),
            _ => panic!("Expected Output"),
        }
    }
}
