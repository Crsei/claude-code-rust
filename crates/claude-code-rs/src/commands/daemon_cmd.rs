//! `/daemon` command -- view/control the daemon process.
//!
//! Subcommands:
//! - `status` (default): show daemon URL and running state
//! - `stop`: request daemon shutdown

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::daemon::process_state::{self, DaemonStatusSnapshot};

pub struct DaemonCmdHandler;

#[async_trait]
impl CommandHandler for DaemonCmdHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        match args.trim().to_lowercase().as_str() {
            "" | "status" => show_status(ctx),
            "stop" => request_stop(ctx),
            "start" | "restart" => Ok(CommandResult::Output(
                "Use the shell command `claude daemon start` or `claude daemon restart`.".into(),
            )),
            other => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\
                 Usage:\n  \
                   /daemon          -- show daemon status\n  \
                   /daemon status   -- show daemon status\n  \
                   /daemon stop     -- request daemon shutdown\n  \
                   /daemon start    -- show shell command hint\n  \
                   /daemon restart  -- show shell command hint",
                other
            ))),
        }
    }
}

/// Show daemon status information.
fn show_status(_ctx: &CommandContext) -> Result<CommandResult> {
    let output = match process_state::status_snapshot()? {
        DaemonStatusSnapshot::Running(state) => {
            let workers = if state.workers.is_empty() {
                "Workers:    0".to_string()
            } else {
                let mut lines = vec![format!("Workers:    {}", state.workers.len())];
                for worker in &state.workers {
                    let pid = worker
                        .pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    lines.push(format!(
                        "  - {} kind={} pid={} status={}",
                        worker.worker_id, worker.kind, pid, worker.status
                    ));
                }
                lines.join("\n")
            };
            format!(
                "=== Daemon Status ===\n\
                 Running:    yes\n\
                 PID:        {}\n\
                 Health URL: {}\n\
                 State file: {}\n\
                 {}",
                state.pid,
                state.health_url,
                process_state::state_path().display(),
                workers
            )
        }
        DaemonStatusSnapshot::Stale(state) => format!(
            "=== Daemon Status ===\n\
             Running:    stale\n\
             Last PID:   {}\n\
             State file: {}",
            state.pid,
            process_state::state_path().display()
        ),
        DaemonStatusSnapshot::Stopped => format!(
            "=== Daemon Status ===\n\
             Running:    no\n\
             State file: {}",
            process_state::state_path().display()
        ),
    };
    Ok(CommandResult::Output(output))
}

/// Request daemon to stop.
fn request_stop(_ctx: &CommandContext) -> Result<CommandResult> {
    match process_state::status_snapshot()? {
        DaemonStatusSnapshot::Running(state) => {
            process_state::request_shutdown("slash command /daemon stop")?;
            Ok(CommandResult::Output(format!(
                "Daemon stop requested for PID {}.",
                state.pid
            )))
        }
        DaemonStatusSnapshot::Stale(state) => Ok(CommandResult::Output(format!(
            "Daemon state is stale for PID {}. Run `claude daemon status` from the shell to refresh.",
            state.pid
        ))),
        DaemonStatusSnapshot::Stopped => Ok(CommandResult::Output(
            "Daemon is not currently running.".into(),
        )),
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

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_status_without_state_reports_not_running() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let handler = DaemonCmdHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Running:    no")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_start_and_restart_show_shell_hint() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let handler = DaemonCmdHandler;
        let mut ctx = test_ctx();

        for input in &["start", "restart"] {
            let result = handler.execute(input, &mut ctx).await.unwrap();
            match result {
                CommandResult::Output(text) => assert!(
                    text.contains("claude daemon"),
                    "expected shell hint for input '{}'",
                    input
                ),
                _ => panic!("Expected Output for input '{}'", input),
            }
        }
    }
}
