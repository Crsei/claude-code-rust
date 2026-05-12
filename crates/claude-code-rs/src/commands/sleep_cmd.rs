//! `/sleep` command -- set proactive sleep duration.
//!
//! Schedules a sleep period (in seconds) during which the proactive
//! tick loop pauses autonomous actions.
//!
//! Requires `FEATURE_PROACTIVE=1` (implied by `FEATURE_KAIROS=1`).

use anyhow::Result;
use async_trait::async_trait;

use crate::config::features::{self, Feature};
use cc_commands::{CommandContext, CommandHandler, CommandResult};

/// Minimum sleep duration in seconds.
const MIN_SLEEP_SECS: u64 = 1;
/// Maximum sleep duration in seconds (1 hour).
const MAX_SLEEP_SECS: u64 = 3600;

pub struct SleepCmdHandler;

#[async_trait]
impl CommandHandler for SleepCmdHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        if !features::enabled(Feature::Proactive) {
            return Ok(CommandResult::Output(
                "Sleep command requires FEATURE_PROACTIVE=1".into(),
            ));
        }

        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Ok(CommandResult::Output(format!(
                "Usage: /sleep <seconds> (range: {}-{})\n\
                 Current tick interval: {}",
                MIN_SLEEP_SECS,
                MAX_SLEEP_SECS,
                match ctx.app_state.autonomous_tick_ms {
                    Some(ms) => format!("{}ms", ms),
                    None => "disabled".to_string(),
                }
            )));
        }

        let secs: u64 = match trimmed.parse() {
            Ok(v) => v,
            Err(_) => {
                return Ok(CommandResult::Output(format!(
                    "Invalid number: '{}'. Usage: /sleep <seconds> ({}-{})",
                    trimmed, MIN_SLEEP_SECS, MAX_SLEEP_SECS
                )));
            }
        };

        if !(MIN_SLEEP_SECS..=MAX_SLEEP_SECS).contains(&secs) {
            return Ok(CommandResult::Output(format!(
                "Sleep duration must be between {} and {} seconds.",
                MIN_SLEEP_SECS, MAX_SLEEP_SECS
            )));
        }

        let sleep_state =
            crate::daemon::process_state::write_sleep_state(secs, "slash command /sleep")?;

        Ok(CommandResult::Output(format!(
            "Sleep scheduled until {} ({} second{}). Proactive actions paused.",
            sleep_state.sleeping_until.to_rfc3339(),
            secs,
            if secs == 1 { "" } else { "s" }
        )))
    }
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
        let handler = SleepCmdHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("10", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("FEATURE_PROACTIVE")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_all_args_gated() {
        // Feature is not enabled in test env, so all invocations hit the gate.
        let handler = SleepCmdHandler;
        let mut ctx = test_ctx();

        for input in &["", "abc", "0", "9999", "60"] {
            let result = handler.execute(input, &mut ctx).await.unwrap();
            match result {
                CommandResult::Output(text) => assert!(
                    text.contains("FEATURE_PROACTIVE"),
                    "expected gate message for input '{}'",
                    input
                ),
                _ => panic!("Expected Output for input '{}'", input),
            }
        }
    }
}
