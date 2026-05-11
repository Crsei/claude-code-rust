//! /model command -- switch the active model.
//!
//! Subcommands:
//! - `/model`            -- show the currently active model
//! - `/model <name>`     -- switch to the named model
//!
//! Multi-vendor note: aliases are intentionally minimal and neutral because
//! this fork talks to several upstream vendors. Users can supply full model
//! identifiers; `availableModels` (from settings) gates which IDs are accepted.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub use cc_models::resolve_model_alias;

pub fn removed_legacy_model_alias_error(name: &str) -> String {
    cc_models::removed_legacy_model_alias_error(name)
}

pub fn is_removed_legacy_model_alias(name: &str) -> bool {
    cc_models::is_removed_legacy_model_alias(name)
}

fn available_model_matches(allowed: &str, resolved_target: &str) -> bool {
    let trimmed = allowed.trim();
    !trimmed.is_empty()
        && !is_removed_legacy_model_alias(trimmed)
        && resolve_model_alias(trimmed) == resolved_target
}

pub fn resolve_model_list_entry(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    if trimmed.is_empty() || is_removed_legacy_model_alias(trimmed) {
        None
    } else {
        Some(resolve_model_alias(trimmed))
    }
}

/// Check whether `model` is allowed by the configured `availableModels` list.
///
/// Returns `Ok(())` when the list is empty (no restriction) or `model`
/// appears in it. Returns `Err(message)` describing the violation otherwise.
pub fn check_available(model: &str, available: &[String]) -> Result<(), String> {
    if is_removed_legacy_model_alias(model) {
        return Err(removed_legacy_model_alias_error(model));
    }
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
    if is_removed_legacy_model_alias(trimmed) {
        return Err(removed_legacy_model_alias_error(trimmed));
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

        // No arguments: show the current model and configured model list.
        if target.is_empty() {
            let mut lines = Vec::new();
            lines.push(format!("Current model: {}", ctx.app_state.main_loop_model));
            lines.push(String::new());
            if available.is_empty() {
                lines.push("Configured models: (not set; any model id is accepted)".into());
            } else {
                lines.push("Configured models:".into());
                for m in &available {
                    lines.push(format!("  - {}", m));
                }
            }
            lines.push(String::new());
            lines.push("Aliases: SOTA, MOTA, FOTA".into());
            lines.push("Usage: /model <model-id|SOTA|MOTA|FOTA>".into());
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
                assert!(!text.contains("opus ->"));
                assert!(!text.contains("sonnet ->"));
                assert!(!text.contains("haiku ->"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_model_switch_by_alias() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("SOTA", &mut ctx).await.unwrap();
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
        assert_eq!(resolve_model_alias("SOTA"), "claude-opus-4-20250514");
        assert_eq!(resolve_model_alias("mota"), "claude-sonnet-4-20250514");
        assert_eq!(resolve_model_alias("opus"), "opus");
        assert_eq!(resolve_model_alias("unknown"), "unknown");
    }

    #[test]
    fn test_check_available_empty_allows_anything() {
        assert!(check_available("any-model-id", &[]).is_ok());
    }

    #[test]
    fn test_resolve_and_validate_model_rejects_removed_legacy_aliases() {
        for legacy in ["opus", "sonnet", "haiku"] {
            let err = resolve_and_validate_model(legacy, &[]).unwrap_err();
            assert!(err.contains("Legacy model alias"));
            assert!(err.contains("SOTA") || err.contains("MOTA") || err.contains("FOTA"));
        }
    }

    #[test]
    fn test_removed_legacy_available_models_do_not_match_literals() {
        let allowed = vec!["sonnet".to_string(), "haiku".to_string()];
        assert!(check_available("sonnet", &allowed).is_err());
        assert!(check_available("claude-sonnet-4-20250514", &allowed).is_err());
        assert_eq!(resolve_model_list_entry("sonnet"), None);
    }

    #[test]
    fn test_check_available_accepts_alias_entries() {
        let allowed = vec!["SOTA".to_string(), "gpt-4o".to_string()];
        assert!(check_available("claude-opus-4-20250514", &allowed).is_ok());
    }

    #[test]
    fn test_check_available_accepts_full_id_entries_for_alias_input() {
        let allowed = vec!["claude-opus-4-20250514".to_string()];
        assert!(check_available(&resolve_model_alias("SOTA"), &allowed).is_ok());
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
        let result = handler.execute("SOTA", &mut ctx).await.unwrap();
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
        ctx.app_state.settings.available_models = vec!["SOTA".to_string()];
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
                assert!(text.contains("Configured models"));
                assert!(text.contains("- alpha"));
                assert!(text.contains("- beta"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
