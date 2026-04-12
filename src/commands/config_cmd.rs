//! /config command -- show and modify configuration settings.
//!
//! Subcommands:
//! - `/config show`         -- display current configuration
//! - `/config set <key> <value>` -- set a configuration value
//! - `/config reset`        -- reset to defaults

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings;

/// Handler for the `/config` slash command.
pub struct ConfigHandler;

#[async_trait]
impl CommandHandler for ConfigHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("show") | None => handle_show(ctx),
            Some("set") => handle_set(&parts[1..], ctx),
            Some("reset") => handle_reset(ctx),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown config subcommand: '{}'\n\
                 Usage:\n  \
                   /config show          -- show current settings\n  \
                   /config set <key> <value> -- set a value\n  \
                   /config reset         -- reset to defaults",
                sub
            ))),
        }
    }
}

/// Display the current configuration.
fn handle_show(ctx: &CommandContext) -> Result<CommandResult> {
    let state = &ctx.app_state;
    let mut lines = Vec::new();

    lines.push("Current configuration:".into());
    lines.push(String::new());
    lines.push(format!("  model:         {}", state.main_loop_model));
    lines.push(format!(
        "  theme:         {}",
        state.settings.theme.as_deref().unwrap_or("(default)")
    ));
    lines.push(format!("  verbose:       {}", state.verbose));
    lines.push(format!(
        "  thinking:      {}",
        state
            .thinking_enabled
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(auto)".into())
    ));
    lines.push(format!("  fast_mode:     {}", state.fast_mode));
    lines.push(format!(
        "  effort:        {}",
        state.effort_value.as_deref().unwrap_or("(default)")
    ));
    lines.push(format!(
        "  permission_mode: {:?}",
        state.tool_permission_context.mode
    ));

    // Try to show the config file locations.
    if let Ok(global_dir) = settings::global_claude_dir() {
        lines.push(String::new());
        lines.push(format!(
            "  Global config: {}",
            global_dir.join("settings.json").display()
        ));
    }
    lines.push(format!(
        "  Project dir:   {}",
        ctx.cwd.join(".cc-rust").join("settings.json").display()
    ));

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Set a single configuration value.
fn handle_set(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.len() < 2 {
        return Ok(CommandResult::Output(
            "Usage: /config set <key> <value>\n\
             Available keys: model, theme, verbose"
                .into(),
        ));
    }

    let key = parts[0];
    let value = parts[1..].join(" ");

    match key {
        "model" => {
            ctx.app_state.main_loop_model = value.clone();
            Ok(CommandResult::Output(format!("Model set to: {}", value)))
        }
        "theme" => {
            ctx.app_state.settings.theme = Some(value.clone());
            Ok(CommandResult::Output(format!("Theme set to: {}", value)))
        }
        "verbose" => {
            let v = value == "true" || value == "1";
            ctx.app_state.verbose = v;
            ctx.app_state.settings.verbose = Some(v);
            Ok(CommandResult::Output(format!("Verbose set to: {}", v)))
        }
        _ => Ok(CommandResult::Output(format!(
            "Unknown config key: '{}'\nAvailable keys: model, theme, verbose",
            key
        ))),
    }
}

/// Reset configuration to defaults.
fn handle_reset(ctx: &mut CommandContext) -> Result<CommandResult> {
    let default_state = crate::types::app_state::AppState::default();
    ctx.app_state.settings = default_state.settings;
    ctx.app_state.main_loop_model = default_state.main_loop_model;
    ctx.app_state.verbose = default_state.verbose;
    ctx.app_state.fast_mode = default_state.fast_mode;
    ctx.app_state.effort_value = default_state.effort_value;
    ctx.app_state.thinking_enabled = default_state.thinking_enabled;

    Ok(CommandResult::Output(
        "Configuration reset to defaults.".into(),
    ))
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
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_config_show() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("model:"));
                assert!(text.contains("verbose:"));
                assert!(text.contains("permission_mode:"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_config_set_model() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("set model claude-opus", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("claude-opus"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "claude-opus");
    }

    #[tokio::test]
    async fn test_config_set_verbose() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("set verbose true", &mut ctx).await.unwrap();
        assert!(ctx.app_state.verbose);
    }

    #[tokio::test]
    async fn test_config_reset() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        ctx.app_state.main_loop_model = "custom-model".into();
        ctx.app_state.verbose = true;

        let result = handler.execute("reset", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("reset to defaults"));
            }
            _ => panic!("Expected Output result"),
        }
        assert!(!ctx.app_state.verbose);
    }

    #[tokio::test]
    async fn test_config_unknown_subcommand() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("delete", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown config subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_config_no_args_shows_config() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current configuration"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
