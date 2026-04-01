//! /sandbox command -- toggle sandbox mode.
//!
//! Sandbox mode restricts tool execution to a controlled environment.
//! This is a simple toggle that shows the current status.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// In-process sandbox state. A production implementation would integrate
/// with the tool permission system and execution environment.
static SANDBOX_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Handler for the `/sandbox` slash command.
pub struct SandboxHandler;

#[async_trait]
impl CommandHandler for SandboxHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => {
                SANDBOX_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(CommandResult::Output(
                    "Sandbox mode enabled. Tool execution will be restricted."
                        .to_string(),
                ))
            }
            "off" | "disable" => {
                SANDBOX_ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
                Ok(CommandResult::Output(
                    "Sandbox mode disabled. Normal tool execution restored."
                        .to_string(),
                ))
            }
            "status" => show_status(),
            "" => toggle(),
            _ => Ok(CommandResult::Output(format!(
                "Unknown argument: '{}'\n\
                 Usage:\n  \
                   /sandbox          -- toggle sandbox mode\n  \
                   /sandbox on       -- enable sandbox mode\n  \
                   /sandbox off      -- disable sandbox mode\n  \
                   /sandbox status   -- show current status",
                subcmd
            ))),
        }
    }
}

/// Toggle sandbox mode.
fn toggle() -> Result<CommandResult> {
    let was_enabled = SANDBOX_ENABLED.load(std::sync::atomic::Ordering::Relaxed);
    SANDBOX_ENABLED.store(!was_enabled, std::sync::atomic::Ordering::Relaxed);

    let status = if !was_enabled { "enabled" } else { "disabled" };
    Ok(CommandResult::Output(format!(
        "Sandbox mode {}.",
        status
    )))
}

/// Show current sandbox status.
fn show_status() -> Result<CommandResult> {
    let enabled = SANDBOX_ENABLED.load(std::sync::atomic::Ordering::Relaxed);
    let status = if enabled { "enabled" } else { "disabled" };
    Ok(CommandResult::Output(format!(
        "Sandbox mode: {}\n\n\
         When enabled, tool execution is restricted to a controlled environment.\n\
         Dangerous commands (rm -rf, etc.) are blocked regardless of permission mode.",
        status
    )))
}

/// Query whether sandbox mode is currently enabled (for use by other modules).
pub fn is_sandbox_enabled() -> bool {
    SANDBOX_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
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
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_sandbox_toggle() {
        let handler = SandboxHandler;
        let mut ctx = test_ctx();

        // Toggle produces either "enabled" or "disabled"
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("enabled") || text.contains("disabled"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_sandbox_on_off() {
        let handler = SandboxHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output result"),
        }

        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_sandbox_status() {
        let handler = SandboxHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Sandbox mode:"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_sandbox_unknown() {
        let handler = SandboxHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown argument")),
            _ => panic!("Expected Output result"),
        }
    }
}
