//! /output-style command -- configure output formatting.
//!
//! Subcommands:
//! - `/output-style concise`  -- set concise output mode
//! - `/output-style detailed` -- set detailed output mode
//! - `/output-style default`  -- reset to default output mode
//! - `/output-style`          -- show current setting

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// The available output styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputStyle {
    Default,
    Concise,
    Detailed,
}

impl OutputStyle {
    fn as_str(self) -> &'static str {
        match self {
            OutputStyle::Default => "default",
            OutputStyle::Concise => "concise",
            OutputStyle::Detailed => "detailed",
        }
    }

    fn description(self) -> &'static str {
        match self {
            OutputStyle::Default => "Balanced output with moderate detail",
            OutputStyle::Concise => "Brief, to-the-point responses",
            OutputStyle::Detailed => "Verbose responses with full explanations",
        }
    }
}

/// Global output style state.
static CURRENT_STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn load_style() -> OutputStyle {
    match CURRENT_STYLE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => OutputStyle::Concise,
        2 => OutputStyle::Detailed,
        _ => OutputStyle::Default,
    }
}

fn store_style(style: OutputStyle) {
    let val = match style {
        OutputStyle::Default => 0,
        OutputStyle::Concise => 1,
        OutputStyle::Detailed => 2,
    };
    CURRENT_STYLE.store(val, std::sync::atomic::Ordering::Relaxed);
}

/// Handler for the `/output-style` slash command.
pub struct OutputStyleHandler;

#[async_trait]
impl CommandHandler for OutputStyleHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "concise" => {
                store_style(OutputStyle::Concise);
                Ok(CommandResult::Output(format!(
                    "Output style set to: concise\n{}",
                    OutputStyle::Concise.description()
                )))
            }
            "detailed" | "verbose" => {
                store_style(OutputStyle::Detailed);
                Ok(CommandResult::Output(format!(
                    "Output style set to: detailed\n{}",
                    OutputStyle::Detailed.description()
                )))
            }
            "default" | "reset" => {
                store_style(OutputStyle::Default);
                Ok(CommandResult::Output(format!(
                    "Output style set to: default\n{}",
                    OutputStyle::Default.description()
                )))
            }
            "" => show_current(),
            _ => Ok(CommandResult::Output(format!(
                "Unknown output style: '{}'\n\
                 Usage:\n  \
                   /output-style              -- show current style\n  \
                   /output-style concise      -- brief responses\n  \
                   /output-style detailed     -- verbose responses\n  \
                   /output-style default      -- balanced responses",
                subcmd
            ))),
        }
    }
}

/// Show the current output style.
fn show_current() -> Result<CommandResult> {
    let style = load_style();
    Ok(CommandResult::Output(format!(
        "Current output style: {}\n{}\n\n\
         Available styles:\n  \
           concise   -- {}\n  \
           detailed  -- {}\n  \
           default   -- {}",
        style.as_str(),
        style.description(),
        OutputStyle::Concise.description(),
        OutputStyle::Detailed.description(),
        OutputStyle::Default.description(),
    )))
}

/// Query the current output style name (for use by other modules).
pub fn current_output_style() -> &'static str {
    load_style().as_str()
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
    async fn test_output_style_show_default() {
        store_style(OutputStyle::Default);
        let handler = OutputStyleHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("default"));
                assert!(text.contains("Available styles"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_output_style_set_concise() {
        let handler = OutputStyleHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("concise", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("concise"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_output_style_set_detailed() {
        let handler = OutputStyleHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("detailed", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("detailed"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_output_style_reset() {
        let handler = OutputStyleHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("default", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("default"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_output_style_unknown() {
        let handler = OutputStyleHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("rainbow", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown output style"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
