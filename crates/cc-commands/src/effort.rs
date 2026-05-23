//! `/effort` command -- set the thinking effort level.
//!
//! Controls the reasoning depth for the model by mapping a label or numeric
//! override to the `thinking.budget_tokens` value sent on the next request.

use anyhow::Result;
use async_trait::async_trait;
use cc_config::settings::{self, RawSettings};

use crate::{CommandContext, CommandHandler, CommandResult};
use cc_engine::effort::{
    effort_to_budget_tokens, normalize_effort_value, DEFAULT_THINKING_BUDGET, MAX_THINKING_BUDGET,
};

pub struct EffortHandler;

const VALID_LEVELS: &[&str] = &["low", "medium", "high", "auto", "max"];

fn budget_summary(value: Option<&str>) -> String {
    match value {
        None => format!(
            "(not set - thinking falls back to {} tokens when enabled)",
            DEFAULT_THINKING_BUDGET
        ),
        Some("auto") => format!(
            "auto (model default; {} thinking tokens in the fixed-budget path)",
            DEFAULT_THINKING_BUDGET
        ),
        Some("max") => format!(
            "max (highest fixed budget in this fork: {} thinking tokens)",
            MAX_THINKING_BUDGET
        ),
        Some(s) => match effort_to_budget_tokens(s) {
            Some(tokens) => format!("{} ({} thinking tokens)", s, tokens),
            None => format!("{} (unrecognized - will use default budget)", s),
        },
    }
}

#[async_trait]
impl CommandHandler for EffortHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_string();

        if arg.is_empty() {
            return Ok(CommandResult::Output(format!(
                "Current effort: {}\n\n\
                 Usage: /effort <low|medium|high|auto|max|<token-count>>\n\
                 Valid labels: {}\n\
                 Numeric values are passed through as the thinking budget.",
                budget_summary(ctx.app_state.effort_value.as_deref()),
                VALID_LEVELS.join(", "),
            )));
        }

        let stored = match normalize_effort_value(&arg) {
            Some(value) => value,
            None => {
                return Ok(CommandResult::Output(format!(
                    "Invalid effort: '{}'\nValid labels: {} (or a numeric budget token count)",
                    arg,
                    VALID_LEVELS.join(", ")
                )));
            }
        };

        ctx.app_state.effort_value = Some(stored.clone());
        ctx.app_state.settings.effort_level = Some(stored.clone());
        ctx.app_state
            .settings
            .sources
            .insert("effortLevel".to_string(), settings::SettingsSource::User);

        let persist_msg = persist_user_effort_level(&stored);
        Ok(CommandResult::Output(format!(
            "Effort set to: {}\n{}",
            budget_summary(Some(&stored)),
            persist_msg
        )))
    }
}

fn persist_user_effort_level(value: &str) -> String {
    let path = settings::user_settings_path();
    let mut raw = if path.exists() {
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|txt| serde_json::from_str::<RawSettings>(&txt).map_err(anyhow::Error::from))
        {
            Ok(raw) => raw,
            Err(error) => {
                return format!(
                    "Effort updated for this session, but user settings were not updated: {}",
                    error
                );
            }
        }
    } else {
        RawSettings::default()
    };

    raw.effort_level = Some(value.to_string());
    match settings::write_user_settings(&raw) {
        Ok(path) => format!("-> persisted effortLevel={} to {}", value, path.display()),
        Err(error) => format!(
            "Effort updated for this session, but user settings were not updated: {}",
            error
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_bootstrap::SessionId;
    use cc_engine::types::app_state::AppState;
    use std::path::PathBuf;

    struct HomeGuard {
        previous: Option<String>,
    }

    impl HomeGuard {
        fn temp() -> (tempfile::TempDir, Self) {
            let dir = tempfile::tempdir().unwrap();
            let previous = std::env::var("CC_RUST_HOME").ok();
            std::env::set_var("CC_RUST_HOME", dir.path());
            (dir, Self { previous })
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var("CC_RUST_HOME", value),
                None => std::env::remove_var("CC_RUST_HOME"),
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
    async fn test_effort_no_args_shows_current() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current effort"));
                assert!(text.contains("not set"));
                assert!(text.contains("thinking tokens") || text.contains("thinking budget"));
                assert!(text.contains("auto"));
                assert!(text.contains("max"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_effort_set_numeric_override() {
        let (_dir, _guard) = HomeGuard::temp();
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("12000", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("12000")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("12000"));
    }

    #[tokio::test]
    async fn test_effort_show_includes_resolved_budget() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        ctx.app_state.effort_value = Some("high".into());
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("high"));
                assert!(text.contains("24576"), "expected high -> 24576: {}", text);
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_effort_set_valid_level() {
        let (dir, _guard) = HomeGuard::temp();
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("high", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("high")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(settings["effortLevel"], "high");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_effort_set_auto() {
        let (_dir, _guard) = HomeGuard::temp();
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("auto", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("auto"));
                assert!(text.contains(&DEFAULT_THINKING_BUDGET.to_string()));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("auto"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_effort_set_max() {
        let (_dir, _guard) = HomeGuard::temp();
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("MAX", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("max"));
                assert!(text.contains(&MAX_THINKING_BUDGET.to_string()));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("max"));
    }

    #[tokio::test]
    async fn test_effort_invalid_level() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("ultra", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Invalid"));
                assert!(text.contains("ultra"));
            }
            _ => panic!("Expected Output"),
        }
        assert!(ctx.app_state.effort_value.is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_effort_case_insensitive() {
        let (_dir, _guard) = HomeGuard::temp();
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let _ = handler.execute("HIGH", &mut ctx).await.unwrap();
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
    }
}
