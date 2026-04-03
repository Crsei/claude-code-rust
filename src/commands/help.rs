//! /help command -- displays available commands and their descriptions.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{get_all_commands, CommandContext, CommandHandler, CommandResult};

/// Handler for the `/help` slash command.
pub struct HelpHandler;

#[async_trait]
impl CommandHandler for HelpHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let commands = get_all_commands();

        // If a specific command name is given, show detailed help for that command.
        if !args.is_empty() {
            let target = args.trim();
            if let Some(cmd) = commands.iter().find(|c| {
                c.name == target || c.aliases.iter().any(|a| a == target)
            }) {
                let aliases = if cmd.aliases.is_empty() {
                    String::new()
                } else {
                    format!("  Aliases: {}", cmd.aliases.join(", "))
                };
                return Ok(CommandResult::Output(format!(
                    "/{} -- {}\n{}",
                    cmd.name, cmd.description, aliases
                )));
            } else {
                return Ok(CommandResult::Output(format!(
                    "Unknown command: '{}'. Type /help to see all commands.",
                    target
                )));
            }
        }

        // Build the full help listing.
        let mut lines: Vec<String> = Vec::new();
        lines.push("Available commands:".into());
        lines.push(String::new());

        // Find the longest command name for alignment.
        let max_len = commands
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or(0);

        for cmd in &commands {
            let padding = " ".repeat(max_len - cmd.name.len() + 2);
            let mut line = format!("  /{}{}{}", cmd.name, padding, cmd.description);
            if !cmd.aliases.is_empty() {
                let alias_str = cmd
                    .aliases
                    .iter()
                    .map(|a| format!("/{}", a))
                    .collect::<Vec<_>>()
                    .join(", ");
                line.push_str(&format!(" ({})", alias_str));
            }
            lines.push(line);
        }

        lines.push(String::new());
        lines.push("Type /help <command> for more information about a specific command.".into());

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::types::app_state::AppState;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_help_lists_all_commands() {
        let handler = HelpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("/help"));
                assert!(text.contains("/clear"));
                assert!(text.contains("/commit"));
                assert!(text.contains("/config"));
                assert!(text.contains("/diff"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_help_specific_command() {
        let handler = HelpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("clear", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("/clear"));
                assert!(text.contains("Clear"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_help_unknown_command() {
        let handler = HelpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("nonexistent", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown command"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
