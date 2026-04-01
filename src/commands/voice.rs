//! `/voice` command -- toggle voice mode.
//!
//! Voice mode enables speech-to-text input via microphone recording.
//! Requires external audio recording tools (sox/arecord) to be installed.
//!
//! Subcommands: on, off, status, (empty = toggle)

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global voice mode flag.
static VOICE_MODE: AtomicBool = AtomicBool::new(false);

pub struct VoiceHandler;

#[async_trait]
impl CommandHandler for VoiceHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => enable_voice(),
            "off" | "disable" => disable_voice(),
            "status" => show_status(),
            "" => toggle_voice(),
            _ => Ok(CommandResult::Output(
                "Usage: /voice [on|off|status]\n\n\
                 Toggles voice mode without arguments.\n\
                 Voice mode requires audio recording tools (sox/arecord)."
                    .to_string(),
            )),
        }
    }
}

/// Enable voice mode after checking prerequisites.
fn enable_voice() -> Result<CommandResult> {
    if VOICE_MODE.load(Ordering::SeqCst) {
        return Ok(CommandResult::Output(
            "Voice mode is already enabled.".to_string(),
        ));
    }

    // Check basic prerequisites: sox or arecord should be available.
    // This is a best-effort check -- we don't block if neither is found,
    // but we warn the user.
    let has_sox = which_exists("sox");
    let has_arecord = which_exists("arecord");

    VOICE_MODE.store(true, Ordering::SeqCst);

    let mut msg =
        "Voice mode enabled. Hold Space to record.\n\
         Note: Voice mode requires audio recording tools (sox/arecord)."
            .to_string();

    if !has_sox && !has_arecord {
        msg.push_str(
            "\n\nWarning: Neither 'sox' nor 'arecord' was found on PATH. \
             Voice recording may not work. Install sox (recommended) or \
             arecord (Linux) to use voice input.",
        );
    }

    Ok(CommandResult::Output(msg))
}

/// Disable voice mode.
fn disable_voice() -> Result<CommandResult> {
    if !VOICE_MODE.load(Ordering::SeqCst) {
        return Ok(CommandResult::Output(
            "Voice mode is already disabled.".to_string(),
        ));
    }

    VOICE_MODE.store(false, Ordering::SeqCst);
    Ok(CommandResult::Output(
        "Voice mode disabled.".to_string(),
    ))
}

/// Toggle voice mode on/off.
fn toggle_voice() -> Result<CommandResult> {
    if VOICE_MODE.load(Ordering::SeqCst) {
        disable_voice()
    } else {
        enable_voice()
    }
}

/// Show current voice mode status.
fn show_status() -> Result<CommandResult> {
    let enabled = VOICE_MODE.load(Ordering::SeqCst);
    let status = if enabled { "enabled" } else { "disabled" };

    let has_sox = which_exists("sox");
    let has_arecord = which_exists("arecord");

    let recorder = if has_sox {
        "sox (found)"
    } else if has_arecord {
        "arecord (found)"
    } else {
        "none found (install sox or arecord)"
    };

    Ok(CommandResult::Output(format!(
        "Voice mode: {}\n\
         Audio recorder: {}",
        status, recorder
    )))
}

/// Check if a command exists on PATH.
fn which_exists(cmd: &str) -> bool {
    std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if voice mode is currently enabled. Exposed for the query loop.
pub fn is_voice_mode() -> bool {
    VOICE_MODE.load(Ordering::SeqCst)
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
        }
    }

    #[tokio::test]
    async fn test_enable_and_disable_voice() {
        let handler = VoiceHandler;
        let mut ctx = test_ctx();

        // Force off first to ensure clean state
        handler.execute("off", &mut ctx).await.unwrap();

        // Enable
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("enabled"));
                assert!(text.contains("Hold Space"));
                assert!(text.contains("sox/arecord"));
            }
            _ => panic!("Expected Output"),
        }
        assert!(is_voice_mode());

        // Disable
        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
        assert!(!is_voice_mode());
    }

    #[tokio::test]
    async fn test_toggle_voice() {
        // Use explicit on/off to avoid dependence on global state from other tests
        let handler = VoiceHandler;
        let mut ctx = test_ctx();

        // Ensure off first
        handler.execute("off", &mut ctx).await.unwrap();

        // Toggle on
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
        assert!(is_voice_mode());

        // Toggle off
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
        assert!(!is_voice_mode());
    }

    #[tokio::test]
    async fn test_status() {
        let handler = VoiceHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Voice mode:"));
                assert!(text.contains("Audio recorder:"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_already_enabled() {
        let handler = VoiceHandler;
        let mut ctx = test_ctx();

        // Force enable first
        handler.execute("on", &mut ctx).await.unwrap();

        // Try enabling again
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("already enabled")),
            _ => panic!("Expected Output"),
        }

        // Clean up
        handler.execute("off", &mut ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_unknown_subcommand() {
        let handler = VoiceHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("loud", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Usage")),
            _ => panic!("Expected Output"),
        }
    }
}
