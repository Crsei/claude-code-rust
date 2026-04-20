//! /model command -- switch the active model.
//!
//! Subcommands:
//! - `/model`            -- show the currently active model
//! - `/model <name>`     -- switch to the named model
//!
//! Multi-vendor note: aliases are intentionally minimal because this fork
//! talks to several upstream vendors. Users supply full model identifiers;
//! `availableModels` (from settings) gates which IDs are accepted.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Minimal alias set shared with `/model` and web `set_model`.
/// Kept short on purpose: this fork supports non-Anthropic providers where
/// larger Anthropic-specific alias sets would be misleading.
pub const MODEL_ALIASES: &[(&str, &str)] = &[
    ("opus", "claude-opus-4-20250514"),
    ("sonnet", "claude-sonnet-4-20250514"),
    ("haiku", "claude-haiku-3-5-20241022"),
];

/// Resolve a model alias to its full identifier, or return the input as-is.
pub fn resolve_model_alias(name: &str) -> String {
    let trimmed = name.trim();
    for (alias, full) in MODEL_ALIASES {
        if trimmed.eq_ignore_ascii_case(alias) {
            return full.to_string();
        }
    }
    trimmed.to_string()
}

fn available_model_matches(allowed: &str, resolved_target: &str) -> bool {
    let trimmed = allowed.trim();
    !trimmed.is_empty() && resolve_model_alias(trimmed) == resolved_target
}

/// Check whether `model` is allowed by the configured `availableModels` list.
///
/// Returns `Ok(())` when the list is empty (no restriction) or `model`
/// appears in it. Returns `Err(message)` describing the violation otherwise.
pub fn check_available(model: &str, available: &[String]) -> Result<(), String> {
    if available.is_empty() {
        return Ok(());
    }
    if available.iter().any(|m| available_model_matches(m, model)) {
        return Ok(());
    }
    Err(format!(
        "Model '{}' is not in availableModels.\nAllowed: {}",
        model,
        available.join(", "),
    ))
}

/// Resolve a user-provided model selection and validate it against
/// `availableModels`.
pub fn resolve_and_validate_model(name: &str, available: &[String]) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("model name required".to_string());
    }

    let resolved = resolve_model_alias(trimmed);
    check_available(&resolved, available)?;
    Ok(resolved)
}

/// Handler for the `/model` slash command.
pub struct ModelHandler;

#[async_trait]
impl CommandHandler for ModelHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let target = args.trim();
        let available = ctx.app_state.settings.available_models.clone();

        // No arguments: show the current model, aliases, and any restriction.
        if target.is_empty() {
            let mut lines = Vec::new();
            lines.push(format!("Current model: {}", ctx.app_state.main_loop_model));
            lines.push(String::new());
            lines.push("Aliases:".into());
            for (alias, full) in MODEL_ALIASES {
                lines.push(format!("  {} -> {}", alias, full));
            }
            lines.push(String::new());
            if available.is_empty() {
                lines.push("availableModels: (not restricted)".into());
            } else {
                lines.push("availableModels (allowed):".into());
                for m in &available {
                    lines.push(format!("  - {}", m));
                }
            }
            lines.push(String::new());
            lines.push("Usage: /model <model-id or alias>".into());
            return Ok(CommandResult::Output(lines.join("\n")));
        }

        let resolved = match resolve_and_validate_model(target, &available) {
            Ok(model) => model,
            Err(msg) => return Ok(CommandResult::Output(format!("Rejected: {}", msg))),
        };

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

    #[test]
    fn test_check_available_empty_allows_anything() {
        assert!(check_available("any-model-id", &[]).is_ok());
    }

    #[test]
    fn test_check_available_accepts_alias_entries() {
        let allowed = vec!["opus".to_string(), "gpt-4o".to_string()];
        assert!(check_available("claude-opus-4-20250514", &allowed).is_ok());
    }

    #[test]
    fn test_check_available_accepts_full_id_entries_for_alias_input() {
        let allowed = vec!["claude-opus-4-20250514".to_string()];
        assert!(check_available(&resolve_model_alias("opus"), &allowed).is_ok());
    }

    #[test]
    fn test_check_available_rejects_unlisted() {
        let allowed = vec!["claude-opus-4-20250514".to_string(), "gpt-4o".to_string()];
        let err = check_available("deepseek-chat", &allowed).unwrap_err();
        assert!(err.contains("not in availableModels"));
        assert!(err.contains("claude-opus-4-20250514"));
    }

    #[test]
    fn test_check_available_accepts_listed() {
        let allowed = vec!["gpt-4o".to_string()];
        assert!(check_available("gpt-4o", &allowed).is_ok());
    }

    #[test]
    fn test_resolve_and_validate_model_rejects_blank_input() {
        let err = resolve_and_validate_model("   ", &[]).unwrap_err();
        assert!(err.contains("model name required"));
    }

    #[tokio::test]
    async fn test_model_switch_rejected_by_available_models() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.settings.available_models = vec!["gpt-4o".to_string()];
        let result = handler.execute("opus", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Rejected"));
                assert!(text.contains("not in availableModels"));
            }
            _ => panic!("Expected Output"),
        }
        assert_ne!(ctx.app_state.main_loop_model, "claude-opus-4-20250514");
    }

    #[tokio::test]
    async fn test_model_switch_allowed_when_in_available_models() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.settings.available_models = vec!["opus".to_string()];
        let result = handler
            .execute("claude-opus-4-20250514", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Model changed")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "claude-opus-4-20250514");
    }

    #[tokio::test]
    async fn test_model_show_lists_available_models() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.settings.available_models = vec!["alpha".to_string(), "beta".to_string()];
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("availableModels (allowed)"));
                assert!(text.contains("- alpha"));
                assert!(text.contains("- beta"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
