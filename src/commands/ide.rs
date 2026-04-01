//! `/ide` command -- IDE integration info.
//!
//! Shows information about integrating cc-rust with popular IDEs.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct IdeHandler;

#[async_trait]
impl CommandHandler for IdeHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "vscode" | "code" => Ok(CommandResult::Output(vscode_info())),
            "jetbrains" | "idea" | "intellij" => Ok(CommandResult::Output(jetbrains_info())),
            "" => Ok(CommandResult::Output(general_info())),
            _ => Ok(CommandResult::Output(format!(
                "Unknown IDE: '{}'\n\nSupported: vscode, jetbrains",
                subcmd
            ))),
        }
    }
}

fn general_info() -> String {
    let mut lines = Vec::new();
    lines.push("IDE Integration".to_string());
    lines.push("─".repeat(30));
    lines.push(String::new());
    lines.push("Supported IDEs:".to_string());
    lines.push(String::new());
    lines.push("  VS Code / Cursor".to_string());
    lines.push("    Use `/ide vscode` for setup instructions.".to_string());
    lines.push(String::new());
    lines.push("  JetBrains (IntelliJ, WebStorm, PyCharm, etc.)".to_string());
    lines.push("    Use `/ide jetbrains` for setup instructions.".to_string());
    lines.push(String::new());
    lines.push("All IDEs can use cc-rust via the terminal integration.".to_string());
    lines.push("Run cc-rust in the IDE's built-in terminal for the best experience.".to_string());
    lines.join("\n")
}

fn vscode_info() -> String {
    let mut lines = Vec::new();
    lines.push("VS Code Integration".to_string());
    lines.push("─".repeat(30));
    lines.push(String::new());
    lines.push("Option 1: Built-in Terminal".to_string());
    lines.push("  Open the integrated terminal (Ctrl+`) and run `cc-rust`.".to_string());
    lines.push(String::new());
    lines.push("Option 2: Task Runner".to_string());
    lines.push("  Add to .vscode/tasks.json:".to_string());
    lines.push("  {".to_string());
    lines.push("    \"label\": \"cc-rust\",".to_string());
    lines.push("    \"type\": \"shell\",".to_string());
    lines.push("    \"command\": \"cc-rust\"".to_string());
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("The VS Code terminal provides full color and interactive support.".to_string());
    lines.join("\n")
}

fn jetbrains_info() -> String {
    let mut lines = Vec::new();
    lines.push("JetBrains IDE Integration".to_string());
    lines.push("─".repeat(30));
    lines.push(String::new());
    lines.push("Option 1: Built-in Terminal".to_string());
    lines.push("  Open the terminal tool window (Alt+F12) and run `cc-rust`.".to_string());
    lines.push(String::new());
    lines.push("Option 2: External Tools".to_string());
    lines.push("  Settings > Tools > External Tools > Add:".to_string());
    lines.push("    Program: cc-rust".to_string());
    lines.push("    Working directory: $ProjectFileDir$".to_string());
    lines.push(String::new());
    lines.push("Works with IntelliJ IDEA, WebStorm, PyCharm, CLion, and more.".to_string());
    lines.join("\n")
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
    async fn test_general_info() {
        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("IDE Integration"));
                assert!(text.contains("VS Code"));
                assert!(text.contains("JetBrains"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_vscode_info() {
        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("vscode", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("VS Code Integration"));
                assert!(text.contains("terminal"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_jetbrains_info() {
        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("jetbrains", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("JetBrains"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
