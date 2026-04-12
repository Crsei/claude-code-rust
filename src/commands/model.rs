//! /model command -- switch the active model.
//!
//! Subcommands:
//! - `/model`            -- show the currently active model
//! - `/model <name>`     -- switch to the named model
//!
//! In the TypeScript version this opens an interactive model picker (React).
//! In the Rust CLI we accept the model name directly as an argument.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Well-known model aliases, mirroring TypeScript's MODEL_ALIASES.
const MODEL_ALIASES: &[(&str, &str)] = &[
    ("opus", "claude-opus-4-20250514"),
    ("sonnet", "claude-sonnet-4-20250514"),
    ("haiku", "claude-haiku-3-5-20241022"),
];

/// Resolve a model alias to its full identifier, or return the input as-is.
fn resolve_model_alias(name: &str) -> String {
    for (alias, full) in MODEL_ALIASES {
        if name.eq_ignore_ascii_case(alias) {
            return full.to_string();
        }
    }
    name.to_string()
}

/// Handler for the `/model` slash command.
pub struct ModelHandler;

#[async_trait]
impl CommandHandler for ModelHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let target = args.trim();

        // No arguments: show the current model and available aliases.
        if target.is_empty() {
            let mut lines = Vec::new();
            lines.push(format!("Current model: {}", ctx.app_state.main_loop_model));
            lines.push(String::new());
            lines.push("Available aliases:".into());
            for (alias, full) in MODEL_ALIASES {
                lines.push(format!("  {} -> {}", alias, full));
            }
            lines.push(String::new());
            lines.push("Usage: /model <name or alias>".into());
            return Ok(CommandResult::Output(lines.join("\n")));
        }

        let resolved = resolve_model_alias(target);
        let previous = ctx.app_state.main_loop_model.clone();
        ctx.app_state.main_loop_model = resolved.clone();

        Ok(CommandResult::Output(format!(
            "Model changed: {} -> {}",
            previous, resolved
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
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_model_show_current() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current model:"));
                assert!(text.contains(&ctx.app_state.main_loop_model));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_model_switch_by_alias() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("opus", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("claude-opus-4-20250514"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "claude-opus-4-20250514");
    }

    #[tokio::test]
    async fn test_model_switch_by_full_name() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("custom-model-v2", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("custom-model-v2"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "custom-model-v2");
    }

    #[test]
    fn test_resolve_alias() {
        assert_eq!(resolve_model_alias("opus"), "claude-opus-4-20250514");
        assert_eq!(resolve_model_alias("Sonnet"), "claude-sonnet-4-20250514");
        assert_eq!(resolve_model_alias("unknown"), "unknown");
    }
}
