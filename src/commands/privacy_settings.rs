//! `/privacy-settings` command -- show and toggle telemetry settings.
//!
//! Manages privacy-related configuration such as telemetry
//! and data collection preferences.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global telemetry enabled flag.
static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);

pub struct PrivacySettingsHandler;

#[async_trait]
impl CommandHandler for PrivacySettingsHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "telemetry on" | "telemetry enable" => {
                TELEMETRY_ENABLED.store(true, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Telemetry enabled. Anonymous usage data will be collected.".to_string(),
                ))
            }
            "telemetry off" | "telemetry disable" => {
                TELEMETRY_ENABLED.store(false, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Telemetry disabled. No usage data will be collected.".to_string(),
                ))
            }
            "" | "show" => show_settings(),
            _ => Ok(CommandResult::Output(format!(
                "Unknown argument: '{}'\n\n{}",
                subcmd,
                help_text()
            ))),
        }
    }
}

fn show_settings() -> Result<CommandResult> {
    let telemetry = if TELEMETRY_ENABLED.load(Ordering::SeqCst) {
        "enabled"
    } else {
        "disabled"
    };

    let mut lines = Vec::new();
    lines.push("Privacy Settings".to_string());
    lines.push("─".repeat(30));
    lines.push(format!("Telemetry:       {}", telemetry));
    lines.push("Data retention:  session only (not persisted)".to_string());
    lines.push("API logging:     controlled by provider".to_string());
    lines.push(String::new());
    lines.push("Use `/privacy-settings telemetry on|off` to toggle.".to_string());

    Ok(CommandResult::Output(lines.join("\n")))
}

fn help_text() -> String {
    "Usage: /privacy-settings [subcommand]\n\n\
     Subcommands:\n  \
       show               Show current privacy settings (default)\n  \
       telemetry on       Enable anonymous telemetry\n  \
       telemetry off      Disable telemetry"
        .to_string()
}

/// Check if telemetry is currently enabled. Exposed for other modules.
pub fn is_telemetry_enabled() -> bool {
    TELEMETRY_ENABLED.load(Ordering::SeqCst)
}

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
    async fn test_show_settings() {
        let handler = PrivacySettingsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Privacy Settings"));
                assert!(text.contains("Telemetry:"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_toggle_telemetry() {
        let handler = PrivacySettingsHandler;
        let mut ctx = test_ctx();

        // Enable
        let result = handler.execute("telemetry on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("enabled")),
            _ => panic!("Expected Output"),
        }
        assert!(is_telemetry_enabled());

        // Disable
        let result = handler.execute("telemetry off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
        assert!(!is_telemetry_enabled());
    }
}
