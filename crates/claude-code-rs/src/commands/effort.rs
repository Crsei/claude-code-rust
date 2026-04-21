//! `/effort` command -- set the thinking effort level.
//!
//! Controls the reasoning depth for the model by mapping a label or numeric
//! override to the `thinking.budget_tokens` value sent on the next request.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::engine::effort::{
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
        Ok(CommandResult::Output(format!(
            "Effort set to: {}",
            budget_summary(Some(&stored))
        )))
    }
}

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
    async fn test_effort_set_numeric_override() {
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
    async fn test_effort_set_valid_level() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("high", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("high")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
    }

    #[tokio::test]
    async fn test_effort_set_auto() {
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
    async fn test_effort_set_max() {
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
    async fn test_effort_case_insensitive() {
        let handler = EffortHandler;
        let mut ctx = test_ctx();
        let _ = handler.execute("HIGH", &mut ctx).await.unwrap();
        assert_eq!(ctx.app_state.effort_value.as_deref(), Some("high"));
    }
}
