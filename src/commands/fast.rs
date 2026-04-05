//! `/fast` command -- toggle fast mode.
//!
//! Fast mode uses the same model (Opus 4.6) with faster output via
//! `speed: "fast"` API parameter + `fast-mode-2026-02-01` beta header.
//!
//! Implementation:
//! - Toggle `app_state.fast_mode`
//! - Validate model compatibility (only Opus 4.6 supports fast mode)
//! - Auto-switch model if needed
//! - Show current status

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// The model required for fast mode.
const FAST_MODE_MODEL: &str = "claude-opus-4-6-20250414";
/// Alternative model ID patterns that support fast mode.
const FAST_MODE_MODEL_PREFIXES: &[&str] = &["claude-opus-4-6", "claude-opus-4"];

pub struct FastHandler;

#[async_trait]
impl CommandHandler for FastHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => enable_fast_mode(ctx),
            "off" | "disable" => disable_fast_mode(ctx),
            "status" => show_status(ctx),
            "" => toggle_fast_mode(ctx),
            _ => Ok(CommandResult::Output(format!(
                "Unknown argument: '{}'\n\
                 Usage:\n  \
                   /fast          -- toggle fast mode\n  \
                   /fast on       -- enable fast mode\n  \
                   /fast off      -- disable fast mode\n  \
                   /fast status   -- show current status",
                subcmd
            ))),
        }
    }
}

/// Check if the current model supports fast mode.
fn model_supports_fast(model: &str) -> bool {
    let lower = model.to_lowercase();
    FAST_MODE_MODEL_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Toggle fast mode on/off.
fn toggle_fast_mode(ctx: &mut CommandContext) -> Result<CommandResult> {
    if ctx.app_state.fast_mode {
        disable_fast_mode(ctx)
    } else {
        enable_fast_mode(ctx)
    }
}

/// Enable fast mode, auto-switching model if necessary.
fn enable_fast_mode(ctx: &mut CommandContext) -> Result<CommandResult> {
    if ctx.app_state.fast_mode {
        return Ok(CommandResult::Output(
            "Fast mode is already enabled.".to_string(),
        ));
    }

    let previous_model = ctx.app_state.main_loop_model.clone();
    let mut switched_model = false;

    // Auto-switch to Opus 4.6 if current model doesn't support fast mode
    if !model_supports_fast(&ctx.app_state.main_loop_model) {
        ctx.app_state.main_loop_model = FAST_MODE_MODEL.to_string();
        ctx.app_state.settings.model = Some(FAST_MODE_MODEL.to_string());
        switched_model = true;
    }

    ctx.app_state.fast_mode = true;

    let mut msg = "Fast mode enabled. Output will be generated faster using the same model.".to_string();
    if switched_model {
        msg.push_str(&format!(
            "\nModel switched from '{}' to '{}' (fast mode requires Opus 4.6).",
            previous_model, FAST_MODE_MODEL
        ));
    }

    // Note: The query loop reads fast_mode to set:
    //   - speed: "fast" in API request
    //   - anthropic-beta: fast-mode-2026-02-01 header

    Ok(CommandResult::Output(msg))
}

/// Disable fast mode (keeps the current model).
fn disable_fast_mode(ctx: &mut CommandContext) -> Result<CommandResult> {
    if !ctx.app_state.fast_mode {
        return Ok(CommandResult::Output(
            "Fast mode is already disabled.".to_string(),
        ));
    }

    ctx.app_state.fast_mode = false;
    Ok(CommandResult::Output(
        "Fast mode disabled. Normal output speed restored.".to_string(),
    ))
}

/// Show current fast mode status.
fn show_status(ctx: &CommandContext) -> Result<CommandResult> {
    let status = if ctx.app_state.fast_mode {
        "enabled"
    } else {
        "disabled"
    };
    let model = &ctx.app_state.main_loop_model;
    let compatible = model_supports_fast(model);

    Ok(CommandResult::Output(format!(
        "Fast mode: {}\n\
         Model: {}\n\
         Model compatible: {}",
        status, model, compatible
    )))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: "test-session".to_string(),
        }
    }

    #[test]
    fn test_model_supports_fast() {
        assert!(model_supports_fast("claude-opus-4-6-20250414"));
        assert!(model_supports_fast("claude-opus-4-6"));
        assert!(model_supports_fast("claude-opus-4-something"));
        assert!(!model_supports_fast("claude-sonnet-4-20250514"));
        assert!(!model_supports_fast("claude-haiku-4-5"));
    }

    #[tokio::test]
    async fn test_toggle_enables_fast_mode() {
        let handler = FastHandler;
        let mut ctx = test_ctx();
        assert!(!ctx.app_state.fast_mode);

        let result = handler.execute("", &mut ctx).await.unwrap();
        assert!(ctx.app_state.fast_mode);
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_toggle_disables_fast_mode() {
        let handler = FastHandler;
        let mut ctx = test_ctx();
        ctx.app_state.fast_mode = true;

        let result = handler.execute("", &mut ctx).await.unwrap();
        assert!(!ctx.app_state.fast_mode);
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_enable_auto_switches_model() {
        let handler = FastHandler;
        let mut ctx = test_ctx();
        // Default model is sonnet, not opus
        assert!(!model_supports_fast(&ctx.app_state.main_loop_model));

        let result = handler.execute("on", &mut ctx).await.unwrap();
        assert!(ctx.app_state.fast_mode);
        assert_eq!(ctx.app_state.main_loop_model, FAST_MODE_MODEL);
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("switched"));
                assert!(text.contains("Opus 4.6"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_enable_already_enabled() {
        let handler = FastHandler;
        let mut ctx = test_ctx();
        ctx.app_state.fast_mode = true;

        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("already enabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_status_command() {
        let handler = FastHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("disabled"));
                assert!(text.contains("Model:"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_disable_already_disabled() {
        let handler = FastHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("already disabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_unknown_argument() {
        let handler = FastHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("turbo", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown argument")),
            _ => panic!("Expected Output"),
        }
    }
}
