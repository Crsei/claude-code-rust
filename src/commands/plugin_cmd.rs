//! /plugin command -- plugin management.
//!
//! Subcommands:
//! - `/plugin list` -- list installed plugins
//! - `/plugin`      -- show usage help

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::plugins;

/// Handler for the `/plugin` slash command.
pub struct PluginHandler;

#[async_trait]
impl CommandHandler for PluginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("list") | Some("ls") => handle_list(),
            None => handle_help(),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown plugin subcommand: '{}'\n\
                 Usage:\n  \
                   /plugin list -- list installed plugins",
                sub
            ))),
        }
    }
}

/// Show usage help.
fn handle_help() -> Result<CommandResult> {
    Ok(CommandResult::Output(
        "Plugin management.\n\n\
         Usage:\n  \
           /plugin list -- list installed plugins\n\n\
         Plugins are stored in ~/.cc-rust/plugins/.\n\
         Install metadata is in ~/.cc-rust/plugins/installed_plugins.json."
            .to_string(),
    ))
}

/// List installed plugins.
fn handle_list() -> Result<CommandResult> {
    let installed = plugins::loader::load_installed_plugins();

    if installed.is_empty() {
        return Ok(CommandResult::Output(
            "No plugins installed.\n\n\
             Plugin installation metadata is stored in:\n  \
               ~/.cc-rust/plugins/installed_plugins.json"
                .to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("Installed plugins ({}):", installed.len()));
    lines.push(String::new());

    for plugin in &installed {
        let status = match &plugin.status {
            plugins::PluginStatus::Installed => "installed",
            plugins::PluginStatus::Disabled => "disabled",
            plugins::PluginStatus::NotInstalled => "not installed",
            plugins::PluginStatus::Error(e) => e.as_str(),
        };

        lines.push(format!(
            "  {} v{} [{}]",
            plugin.name, plugin.version, status
        ));

        if !plugin.description.is_empty() {
            lines.push(format!("    {}", plugin.description));
        }

        if !plugin.tools.is_empty() {
            lines.push(format!("    Tools: {}", plugin.tools.join(", ")));
        }
        if !plugin.skills.is_empty() {
            lines.push(format!("    Skills: {}", plugin.skills.join(", ")));
        }
        if !plugin.mcp_servers.is_empty() {
            lines.push(format!(
                "    MCP servers: {}",
                plugin.mcp_servers.join(", ")
            ));
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
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
    async fn test_plugin_no_args_shows_help() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Plugin management"));
                assert!(text.contains("/plugin list"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_plugin_list() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("plugin") || text.contains("Plugin"),
                    "Unexpected: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_plugin_unknown_subcommand() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown plugin subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
