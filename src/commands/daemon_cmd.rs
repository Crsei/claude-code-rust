//! `/daemon` command -- view/control the KAIROS daemon process.
//!
//! Subcommands:
//! - `status` (default): show daemon URL and running state
//! - `stop`: request daemon shutdown
//!
//! Requires `FEATURE_KAIROS=1`.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::features::{self, Feature};

/// Default daemon check URL.
const DAEMON_CHECK_URL: &str = "http://127.0.0.1:3579/health";

pub struct DaemonCmdHandler;

#[async_trait]
impl CommandHandler for DaemonCmdHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        if !features::enabled(Feature::Kairos) {
            return Ok(CommandResult::Output(
                "Daemon command requires FEATURE_KAIROS=1".into(),
            ));
        }

        match args.trim().to_lowercase().as_str() {
            "" | "status" => show_status(ctx),
            "stop" => request_stop(ctx),
            other => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\
                 Usage:\n  \
                   /daemon          -- show daemon status\n  \
                   /daemon status   -- show daemon status\n  \
                   /daemon stop     -- request daemon shutdown",
                other
            ))),
        }
    }
}

/// Show daemon status information.
fn show_status(ctx: &CommandContext) -> Result<CommandResult> {
    let active = ctx.app_state.kairos_active;
    Ok(CommandResult::Output(format!(
        "=== Daemon Status ===\n\
         Running:    {}\n\
         Health URL: {}",
        if active { "yes" } else { "no" },
        DAEMON_CHECK_URL
    )))
}

/// Request daemon to stop.
fn request_stop(ctx: &CommandContext) -> Result<CommandResult> {
    if !ctx.app_state.kairos_active {
        return Ok(CommandResult::Output(
            "Daemon is not currently running.".into(),
        ));
    }

    // Actual shutdown will be handled by the daemon module once implemented.
    Ok(CommandResult::Output("Daemon stop requested.".into()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_feature_gate() {
        let handler = DaemonCmdHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("FEATURE_KAIROS")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_all_args_gated() {
        // Feature is not enabled in test env, so all invocations hit the gate.
        let handler = DaemonCmdHandler;
        let mut ctx = test_ctx();

        for input in &["status", "stop", "restart"] {
            let result = handler.execute(input, &mut ctx).await.unwrap();
            match result {
                CommandResult::Output(text) => assert!(
                    text.contains("FEATURE_KAIROS"),
                    "expected gate message for input '{}'",
                    input
                ),
                _ => panic!("Expected Output for input '{}'", input),
            }
        }
    }
}
