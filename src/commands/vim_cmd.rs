//! `/vim` command -- toggle vim keybinding mode.
//!
//! When enabled, the input line uses vim-style keybindings
//! (normal mode, insert mode, motions, etc.).

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global vim mode flag.
static VIM_MODE: AtomicBool = AtomicBool::new(false);

pub struct VimHandler;

#[async_trait]
impl CommandHandler for VimHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "on" | "enable" => {
                VIM_MODE.store(true, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Vim mode enabled. Input uses vim-style keybindings.\n\
                     Press Escape for normal mode, i for insert mode."
                        .to_string(),
                ))
            }
            "off" | "disable" => {
                VIM_MODE.store(false, Ordering::SeqCst);
                Ok(CommandResult::Output(
                    "Vim mode disabled. Standard input keybindings restored.".to_string(),
                ))
            }
            "status" => show_status(),
            "" => {
                let was_enabled = VIM_MODE.fetch_xor(true, Ordering::SeqCst);
                if was_enabled {
                    Ok(CommandResult::Output(
                        "Vim mode disabled.".to_string(),
                    ))
                } else {
                    Ok(CommandResult::Output(
                        "Vim mode enabled.".to_string(),
                    ))
                }
            }
            _ => Ok(CommandResult::Output(
                "Usage: /vim [on|off|status]\n\nToggles vim keybinding mode without arguments."
                    .to_string(),
            )),
        }
    }
}

fn show_status() -> Result<CommandResult> {
    let status = if VIM_MODE.load(Ordering::SeqCst) {
        "enabled"
    } else {
        "disabled"
    };

    let mut lines = Vec::new();
    lines.push(format!("Vim mode: {}", status));
    if VIM_MODE.load(Ordering::SeqCst) {
        lines.push(String::new());
        lines.push("Keybindings:".to_string());
        lines.push("  Escape  Enter normal mode".to_string());
        lines.push("  i       Enter insert mode".to_string());
        lines.push("  h/j/k/l Move cursor".to_string());
        lines.push("  w/b     Word forward/backward".to_string());
        lines.push("  dd      Delete line".to_string());
        lines.push("  yy      Yank line".to_string());
        lines.push("  p       Paste".to_string());
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Check if vim mode is currently enabled. Exposed for the TUI input handler.
pub fn is_vim_mode() -> bool {
    VIM_MODE.load(Ordering::SeqCst)
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
    async fn test_enable_vim() {
        VIM_MODE.store(false, Ordering::SeqCst);

        let handler = VimHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("on", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Vim mode enabled"));
                assert!(text.contains("Escape"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_disable_vim() {
        let handler = VimHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("disabled")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_status_enabled() {
        let handler = VimHandler;
        let mut ctx = test_ctx();
        // Just verify status returns output with key info
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Vim mode:"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_toggle() {
        let handler = VimHandler;
        let mut ctx = test_ctx();

        // Toggle returns enabled or disabled
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("enabled") || text.contains("disabled"));
            }
            _ => panic!("Expected Output"),
        }

    }
}
