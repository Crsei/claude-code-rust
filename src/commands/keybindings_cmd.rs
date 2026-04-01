//! /keybindings command -- show current key bindings.
//!
//! Displays the default key bindings and notes where custom bindings can be
//! configured (`~/.cc-rust/keybindings.json`).

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings;

/// Handler for the `/keybindings` slash command.
pub struct KeybindingsHandler;

#[async_trait]
impl CommandHandler for KeybindingsHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut lines = Vec::new();

        lines.push("Key bindings:".to_string());
        lines.push(String::new());
        lines.push("  Default bindings:".to_string());
        lines.push("    Enter          -- Submit input (when input is non-empty)".to_string());
        lines.push("    Shift+Enter    -- Insert a newline".to_string());
        lines.push("    Ctrl+C         -- Cancel current operation / clear input".to_string());
        lines.push("    Ctrl+D         -- Exit (EOF)".to_string());
        lines.push("    Ctrl+L         -- Clear screen".to_string());
        lines.push("    Up/Down        -- Navigate input history".to_string());
        lines.push("    Tab            -- Accept autocomplete suggestion".to_string());
        lines.push("    Esc            -- Dismiss autocomplete / cancel".to_string());

        // Show path to custom keybindings file.
        let keybindings_path = match settings::global_claude_dir() {
            Ok(dir) => dir.join("keybindings.json"),
            Err(_) => {
                lines.push(String::new());
                lines.push(
                    "  Custom bindings: ~/.cc-rust/keybindings.json (could not resolve path)"
                        .to_string(),
                );
                return Ok(CommandResult::Output(lines.join("\n")));
            }
        };

        lines.push(String::new());

        if keybindings_path.exists() {
            lines.push(format!(
                "  Custom bindings: {} (loaded)",
                keybindings_path.display()
            ));
            // Try to show the contents.
            if let Ok(content) = std::fs::read_to_string(&keybindings_path) {
                lines.push(String::new());
                lines.push("  Custom keybindings content:".to_string());
                for line in content.lines().take(20) {
                    lines.push(format!("    {}", line));
                }
            }
        } else {
            lines.push(format!(
                "  Custom bindings: {} (not found)",
                keybindings_path.display()
            ));
            lines.push(
                "  Create this file to override default key bindings.".to_string(),
            );
        }

        Ok(CommandResult::Output(lines.join("\n")))
    }
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
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_keybindings_shows_defaults() {
        let handler = KeybindingsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Key bindings"));
                assert!(text.contains("Enter"));
                assert!(text.contains("Ctrl+C"));
                assert!(text.contains("keybindings.json"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
