//! /insights command -- session-history analytics and reporting.
//!
//! Usage:
//! - `/insights`                -- all sessions, with usage stats
//! - `/insights this`           -- only sessions in the current workspace
//! - `/insights recent [days]`  -- filter to last `days` (default 30)
//! - `/insights fast`           -- skip session-body scan for a quick metadata sweep
//!
//! Heavy lifting lives in `services::session_analytics`; this handler parses
//! arguments, calls the service, and formats the textual report.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::services::session_analytics::{self, InsightsFilter, Scope};

/// Handler for the `/insights` slash command.
pub struct InsightsHandler;

#[async_trait]
impl CommandHandler for InsightsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        let mut filter = InsightsFilter::default();
        let mut label = String::from("Session insights (all workspaces)");

        match parts.as_slice() {
            [] => { /* defaults */ }
            ["this"] | ["workspace"] | ["ws"] => {
                filter.scope = Scope::Workspace(ctx.cwd.clone());
                label = format!("Session insights ({})", ctx.cwd.display());
            }
            ["fast"] => {
                filter.include_usage = false;
                label = "Session insights (fast — metadata only)".into();
            }
            ["recent", days_arg] => {
                let days = days_arg.parse::<i64>().map_err(|_| {
                    anyhow::anyhow!("expected a positive integer for days, got: {}", days_arg)
                })?;
                if days <= 0 {
                    return Ok(CommandResult::Output(
                        "'days' must be a positive integer.".into(),
                    ));
                }
                filter.since = Some(chrono::Utc::now().timestamp() - days * 86_400);
                label = format!("Session insights (last {} days)", days);
            }
            ["recent"] => {
                let days: i64 = 30;
                filter.since = Some(chrono::Utc::now().timestamp() - days * 86_400);
                label = format!("Session insights (last {} days)", days);
            }
            _ => {
                return Ok(CommandResult::Output(format!(
                    "Unknown /insights args: '{}'\n\n{}",
                    args.trim(),
                    USAGE_HELP
                )));
            }
        }

        let report = session_analytics::compute_insights(&filter)?;
        if report.session_count == 0 {
            return Ok(CommandResult::Output(
                "No sessions match the current filter. Try /insights or /session list.".into(),
            ));
        }
        Ok(CommandResult::Output(session_analytics::format_report(
            &report, &label,
        )))
    }
}

const USAGE_HELP: &str = "Usage:\n  \
   /insights                     -- all sessions\n  \
   /insights this                -- current workspace only\n  \
   /insights recent [days]       -- last N days (default 30)\n  \
   /insights fast                -- metadata-only sweep (skips token/cost scan)";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::session::storage::{self, SerializableMessage, SessionFile};
    use crate::types::app_state::AppState;
    use std::path::{Path, PathBuf};

    struct HomeGuard {
        previous: Option<String>,
    }

    impl HomeGuard {
        fn set(path: &Path) -> Self {
            let previous = std::env::var("CC_RUST_HOME").ok();
            std::env::set_var("CC_RUST_HOME", path);
            Self { previous }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("CC_RUST_HOME", v),
                None => std::env::remove_var("CC_RUST_HOME"),
            }
        }
    }

    fn test_ctx(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::from_string("curr"),
        }
    }

    fn write_session(id: &str, cwd: &str, last_modified: i64) {
        let messages = vec![
            SerializableMessage {
                msg_type: "user".into(),
                uuid: format!("{:0>8}-0000-0000-0000-000000000001", id),
                timestamp: last_modified,
                data: serde_json::json!({ "content": "hi", "is_meta": false }),
            },
            SerializableMessage {
                msg_type: "assistant".into(),
                uuid: format!("{:0>8}-0000-0000-0000-000000000002", id),
                timestamp: last_modified,
                data: serde_json::json!({
                    "content": [],
                    "usage": {
                        "input_tokens": 10u64,
                        "output_tokens": 5u64,
                        "cache_read_input_tokens": 0u64,
                        "cache_creation_input_tokens": 0u64,
                    },
                    "stop_reason": "end_turn",
                    "cost_usd": 0.001,
                }),
            },
        ];
        let file = SessionFile {
            session_id: id.into(),
            created_at: last_modified,
            last_modified,
            cwd: cwd.into(),
            custom_title: None,
            messages,
        };
        std::fs::create_dir_all(storage::get_session_dir()).unwrap();
        std::fs::write(
            storage::get_session_file(id),
            serde_json::to_string_pretty(&file).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_insights_no_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let _g = HomeGuard::set(temp.path());

        let mut ctx = test_ctx(temp.path().to_path_buf());
        let result = InsightsHandler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(t) => assert!(t.contains("No sessions")),
            _ => panic!(),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_insights_all_scope_counts_every_session() {
        let temp = tempfile::tempdir().unwrap();
        let _g = HomeGuard::set(temp.path());

        write_session("aa", "/p1", 1_700_000_100);
        write_session("bb", "/p2", 1_700_000_200);

        let mut ctx = test_ctx(PathBuf::from("/some/where"));
        let result = InsightsHandler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Session insights (all workspaces)"));
                assert!(text.contains("Sessions included:   2"));
                assert!(text.contains("Token usage"));
            }
            _ => panic!(),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_insights_recent_rejects_bad_days() {
        let temp = tempfile::tempdir().unwrap();
        let _g = HomeGuard::set(temp.path());

        let mut ctx = test_ctx(temp.path().to_path_buf());
        match InsightsHandler.execute("recent abc", &mut ctx).await {
            Ok(_) => panic!("expected an error for non-numeric days"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("days"), "got: {}", msg);
            }
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_insights_fast_skips_usage() {
        let temp = tempfile::tempdir().unwrap();
        let _g = HomeGuard::set(temp.path());

        write_session("fast1", "/p1", 1_700_000_100);

        let mut ctx = test_ctx(temp.path().to_path_buf());
        let result = InsightsHandler.execute("fast", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("fast"));
                assert!(text.contains("Usage stats skipped"));
            }
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn test_insights_unknown_subcommand_shows_help() {
        let mut ctx = test_ctx(PathBuf::from("/x"));
        let result = InsightsHandler
            .execute("nonsense foo", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown /insights args"));
                assert!(text.contains("Usage:"));
            }
            _ => panic!(),
        }
    }
}
