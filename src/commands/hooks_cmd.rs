//! /hooks command -- view and manage tool execution hooks.
//!
//! Subcommands:
//! - `/hooks`             -- show all configured hooks
//! - `/hooks show <name>` -- show details of a specific hook
//!
//! Hooks are user-defined shell commands configured in settings.json that run
//! before or after tool executions. The TypeScript version opens an interactive
//! React-based HooksConfigMenu. In the Rust CLI we display a text listing.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings;

/// Handler for the `/hooks` slash command.
pub struct HooksHandler;

#[async_trait]
impl CommandHandler for HooksHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("show") => handle_show_hook(&parts[1..], ctx),
            None => handle_list(ctx),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown hooks subcommand: '{}'\n\
                 Usage:\n  \
                   /hooks              -- list all configured hooks\n  \
                   /hooks show <name>  -- show details of a specific hook",
                sub
            ))),
        }
    }
}

/// List all configured hooks.
fn handle_list(ctx: &CommandContext) -> Result<CommandResult> {
    // Try to load hooks from the merged configuration.
    let hooks = load_hooks_config(ctx);

    if hooks.is_empty() {
        return Ok(CommandResult::Output(
            "No hooks configured.\n\n\
             Hooks can be configured in settings.json under the \"hooks\" key.\n\
             Available hook points:\n  \
               PreToolUse   -- runs before a tool is executed\n  \
               PostToolUse  -- runs after a tool is executed\n  \
               Notification -- runs when a notification is generated\n  \
               Stop         -- runs when the model stops\n\n\
             Example settings.json:\n\
             {\n  \
               \"hooks\": {\n    \
                 \"PreToolUse\": [{\n      \
                   \"matcher\": \"Bash\",\n      \
                   \"hooks\": [{ \"type\": \"command\", \"command\": \"echo pre-hook\" }]\n    \
                 }]\n  \
               }\n\
             }"
                .into(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("Configured hooks ({}):", hooks.len()));
    lines.push(String::new());

    for (name, value) in &hooks {
        let hook_count = match value {
            serde_json::Value::Array(arr) => arr.len(),
            _ => 1,
        };
        lines.push(format!("  {} ({} rule(s))", name, hook_count));
    }

    lines.push(String::new());
    lines.push("Use /hooks show <name> for details.".into());

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Show details of a specific hook.
fn handle_show_hook(parts: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /hooks show <hook_name>\n\
             Example: /hooks show PreToolUse"
                .into(),
        ));
    }

    let name = parts[0];
    let hooks = load_hooks_config(ctx);

    match hooks.get(name) {
        Some(value) => {
            let pretty =
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
            Ok(CommandResult::Output(format!(
                "Hook: {}\n\n{}",
                name, pretty
            )))
        }
        None => Ok(CommandResult::Output(format!(
            "No hook named '{}' found.\n\
             Use /hooks to see all configured hooks.",
            name
        ))),
    }
}

/// Load hooks configuration from the merged settings.
fn load_hooks_config(
    ctx: &CommandContext,
) -> std::collections::HashMap<String, serde_json::Value> {
    let cwd_str = ctx.cwd.to_string_lossy();
    match settings::load_and_merge(&cwd_str) {
        Ok(merged) => merged.hooks,
        Err(_) => std::collections::HashMap::new(),
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
            cwd: PathBuf::from("/nonexistent/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_hooks_list_empty() {
        let handler = HooksHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Should show either "No hooks" or a listing.
                assert!(
                    text.contains("No hooks") || text.contains("Configured hooks"),
                    "Unexpected output: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_hooks_show_nonexistent() {
        let handler = HooksHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("show NonExistentHook", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No hook named"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_hooks_unknown_subcommand() {
        let handler = HooksHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown hooks subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
