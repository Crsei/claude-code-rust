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
use cc_config::runtime_settings::SettingsJson;

use crate::{CommandContext, CommandHandler, CommandResult};

pub use cc_models::resolve_model_alias;

fn neutral_model_alias(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.eq_ignore_ascii_case("SOTA") {
        Some("SOTA")
    } else if trimmed.eq_ignore_ascii_case("MOTA") {
        Some("MOTA")
    } else if trimmed.eq_ignore_ascii_case("FOTA") {
        Some("FOTA")
    } else {
        None
    }
}

fn anthropic_alias_model(alias: &str) -> Option<String> {
    let (env_name, legacy_env_name) = match neutral_model_alias(alias)? {
        "SOTA" => (
            "ANTHROPIC_DEFAULT_SOTA_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ),
        "MOTA" => (
            "ANTHROPIC_DEFAULT_MOTA_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
        ),
        "FOTA" => (
            "ANTHROPIC_DEFAULT_FOTA_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        ),
        _ => return None,
    };
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var(legacy_env_name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

pub fn settings_alias_model(alias: &str, settings: &SettingsJson) -> Option<String> {
    let value = match neutral_model_alias(alias)? {
        "SOTA" => settings.sota_model.as_deref(),
        "MOTA" => settings.mota_model.as_deref(),
        "FOTA" => settings.fota_model.as_deref(),
        _ => None,
    }?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn resolve_model_alias_with_settings(name: &str, settings: &SettingsJson) -> String {
    let trimmed = name.trim();
    if let Some(model) = settings_alias_model(trimmed, settings) {
        return model;
    }
    resolve_model_alias(trimmed)
}

fn anthropic_provider_selected(ctx: &CommandContext) -> bool {
    if let Some(provider) = ctx
        .app_state
        .settings
        .api_provider
        .as_deref()
        .and_then(cc_config::settings::normalize_api_provider)
    {
        return provider == cc_config::settings::API_PROVIDER_ANTHROPIC;
    }

    ctx.app_state.settings.api_provider.is_none()
        && (std::env::var_os("ANTHROPIC_BASE_URL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_SOTA_MODEL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_MOTA_MODEL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_FOTA_MODEL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_OPUS_MODEL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_SONNET_MODEL").is_some()
            || std::env::var_os("ANTHROPIC_DEFAULT_HAIKU_MODEL").is_some())
}

fn configured_model_label(model: &str, ctx: &CommandContext) -> String {
    if let Some(alias) = neutral_model_alias(model) {
        if let Some(settings_model) = settings_alias_model(alias, &ctx.app_state.settings) {
            return format!("{alias} -> {settings_model}");
        }
    }
    if anthropic_provider_selected(ctx) {
        if let Some(alias) = neutral_model_alias(model) {
            if let Some(provider_model) = anthropic_alias_model(alias) {
                return format!("{alias} -> {provider_model}");
            }
        }
    }
    model.to_string()
}

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

fn available_model_matches_with_settings(
    allowed: &str,
    resolved_target: &str,
    settings: &SettingsJson,
) -> bool {
    let trimmed = allowed.trim();
    !trimmed.is_empty()
        && !is_removed_legacy_model_alias(trimmed)
        && resolve_model_alias_with_settings(trimmed, settings) == resolved_target
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

pub fn check_available_with_settings(
    model: &str,
    available: &[String],
    settings: &SettingsJson,
) -> Result<(), String> {
    if is_removed_legacy_model_alias(model) {
        return Err(removed_legacy_model_alias_error(model));
    }
    if available.is_empty() {
        return Ok(());
    }
    if available
        .iter()
        .any(|m| available_model_matches_with_settings(m, model, settings))
    {
        return Ok(());
    }
    Err(format!(
        "Model '{}' is not in availableModels.\nAllowed: {}",
        model,
        available.join(", "),
    ))
}

fn check_available_anthropic_alias(alias: &str, available: &[String]) -> Result<(), String> {
    if available.is_empty() {
        return Ok(());
    }
    if available
        .iter()
        .any(|entry| neutral_model_alias(entry.as_str()) == Some(alias))
    {
        return Ok(());
    }
    Err(format!(
        "Model '{}' is not in availableModels.\nAllowed: {}",
        alias,
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

fn resolve_and_validate_model_for_context(
    name: &str,
    available: &[String],
    ctx: &CommandContext,
) -> Result<String, String> {
    if anthropic_provider_selected(ctx) {
        if let Some(alias) = neutral_model_alias(name) {
            check_available_anthropic_alias(alias, available)?;
            if let Some(settings_model) = settings_alias_model(alias, &ctx.app_state.settings) {
                return Ok(settings_model);
            }
            return Ok(alias.to_string());
        }
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("model name required".to_string());
    }
    if is_removed_legacy_model_alias(trimmed) {
        return Err(removed_legacy_model_alias_error(trimmed));
    }
    let resolved = resolve_model_alias_with_settings(trimmed, &ctx.app_state.settings);
    check_available_with_settings(&resolved, available, &ctx.app_state.settings)?;
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
                    lines.push(format!("  - {}", configured_model_label(m, ctx)));
                }
            }
            lines.push(String::new());
            lines.push("Aliases: SOTA, MOTA, FOTA".into());
            lines.push("Usage: /model <model-id|SOTA|MOTA|FOTA>".into());
            return Ok(CommandResult::Output(lines.join("\n")));
        }

        let resolved = match resolve_and_validate_model_for_context(target, &available, ctx) {
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
    use cc_bootstrap::SessionId;
    use cc_engine::types::app_state::AppState;
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
        ctx.app_state.settings.api_provider = Some("openai-codex".to_string());
        let result = handler.execute("SOTA", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains(cc_models::SOTA_MODEL_ID));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.main_loop_model, cc_models::SOTA_MODEL_ID);
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
        assert_eq!(resolve_model_alias("SOTA"), cc_models::SOTA_MODEL_ID);
        assert_eq!(resolve_model_alias("mota"), cc_models::MOTA_MODEL_ID);
        assert_eq!(resolve_model_alias("opus"), "opus");
        assert_eq!(resolve_model_alias("unknown"), "unknown");
    }

    #[test]
    fn test_resolve_alias_prefers_settings_model() {
        let mut settings = cc_config::runtime_settings::SettingsJson::default();
        settings.sota_model = Some("custom-sota".to_string());
        settings.mota_model = Some("custom-mota".to_string());
        settings.fota_model = Some("custom-fota".to_string());

        assert_eq!(
            resolve_model_alias_with_settings("SOTA", &settings),
            "custom-sota"
        );
        assert_eq!(
            resolve_model_alias_with_settings("mota", &settings),
            "custom-mota"
        );
        assert_eq!(
            resolve_model_alias_with_settings("FOTA", &settings),
            "custom-fota"
        );
        assert_eq!(
            resolve_model_alias_with_settings("gpt-4o", &settings),
            "gpt-4o"
        );
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
        assert!(check_available(cc_models::SOTA_MODEL_ID, &allowed).is_ok());
    }

    #[test]
    fn test_check_available_accepts_full_id_entries_for_alias_input() {
        let allowed = vec![cc_models::SOTA_MODEL_ID.to_string()];
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
        ctx.app_state.main_loop_model = "initial-model".to_string();
        ctx.app_state.settings.available_models = vec!["gpt-4o".to_string()];
        let result = handler.execute("SOTA", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Rejected"));
                assert!(text.contains("not in availableModels"));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "initial-model");
    }

    #[tokio::test]
    async fn test_model_switch_allowed_when_in_available_models() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.settings.available_models = vec!["SOTA".to_string()];
        let result = handler
            .execute(cc_models::SOTA_MODEL_ID, &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Model changed")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.main_loop_model, cc_models::SOTA_MODEL_ID);
    }

    #[tokio::test]
    async fn test_model_switch_uses_settings_alias_model() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.settings.sota_model = Some("custom-sota".to_string());
        ctx.app_state.settings.available_models = vec!["SOTA".to_string()];

        let result = handler.execute("SOTA", &mut ctx).await.unwrap();

        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Model changed"));
                assert!(text.contains("custom-sota"));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "custom-sota");
    }

    #[tokio::test]
    async fn test_model_switch_preserves_anthropic_alias_for_provider_mapping() {
        let handler = ModelHandler;
        let mut ctx = test_ctx();
        ctx.app_state.main_loop_model = "initial-model".to_string();
        ctx.app_state.settings.api_provider = Some("anthropic".to_string());
        ctx.app_state.settings.available_models =
            vec!["SOTA".to_string(), "MOTA".to_string(), "FOTA".to_string()];

        let result = handler.execute("MOTA", &mut ctx).await.unwrap();

        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Model changed"));
                assert!(text.contains("MOTA"));
                assert!(!text.contains(cc_models::MOTA_MODEL_ID));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "MOTA");
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
