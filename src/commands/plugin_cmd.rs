//! /plugin command - plugin registry management.
//!
//! Subcommands:
//! - `/plugin list`               - list registered plugins
//! - `/plugin status`             - status summary
//! - `/plugin enable <plugin-id>` - enable plugin in installed_plugins.json
//! - `/plugin disable <plugin-id>`- disable plugin in installed_plugins.json
//! - `/plugin`                    - show usage help

use anyhow::{bail, Result};
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::plugins;

/// Handler for `/plugin`.
pub struct PluginHandler;

#[async_trait]
impl CommandHandler for PluginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("list") | Some("ls") => handle_list(),
            Some("status") => handle_status(),
            Some("enable") => {
                let id = parts.get(1).copied().unwrap_or("");
                handle_set_enabled(id, true)
            }
            Some("disable") => {
                let id = parts.get(1).copied().unwrap_or("");
                handle_set_enabled(id, false)
            }
            None => handle_help(),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown plugin subcommand: '{}'\n\
                 Usage:\n  \
                   /plugin list\n  \
                   /plugin status\n  \
                   /plugin enable <plugin-id>\n  \
                   /plugin disable <plugin-id>",
                sub
            ))),
        }
    }
}

fn handle_help() -> Result<CommandResult> {
    Ok(CommandResult::Output(
        "Plugin management.\n\n\
         Usage:\n  \
           /plugin list                -- list registered plugins\n  \
           /plugin status              -- show status summary\n  \
           /plugin enable <plugin-id>  -- enable plugin\n  \
           /plugin disable <plugin-id> -- disable plugin\n\n\
         Plugin metadata is persisted at ~/.cc-rust/plugins/installed_plugins.json."
            .to_string(),
    ))
}

fn handle_list() -> Result<CommandResult> {
    let plugins_list = plugins::get_all_plugins();
    if plugins_list.is_empty() {
        return Ok(CommandResult::Output("No plugins registered.".to_string()));
    }

    let mut lines = Vec::new();
    lines.push(format!("Plugins ({}):", plugins_list.len()));
    lines.push(String::new());

    let mut sorted = plugins_list;
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for plugin in sorted {
        lines.push(format!(
            "  {} -- {} (v{})",
            plugin.id,
            format_status(&plugin.status),
            plugin.version
        ));
        if !plugin.skills.is_empty() {
            lines.push(format!("    skills: {}", plugin.skills.join(", ")));
        }
        if !plugin.tools.is_empty() {
            lines.push(format!("    tools: {}", plugin.tools.join(", ")));
        }
        if !plugin.mcp_servers.is_empty() {
            lines.push(format!("    mcp: {}", plugin.mcp_servers.join(", ")));
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

fn handle_status() -> Result<CommandResult> {
    let plugins_list = plugins::get_all_plugins();
    if plugins_list.is_empty() {
        return Ok(CommandResult::Output("No plugins registered.".to_string()));
    }

    let mut installed = 0usize;
    let mut disabled = 0usize;
    let mut errored = 0usize;
    let mut not_installed = 0usize;

    for plugin in &plugins_list {
        match plugin.status {
            plugins::PluginStatus::Installed => installed += 1,
            plugins::PluginStatus::Disabled => disabled += 1,
            plugins::PluginStatus::Error(_) => errored += 1,
            plugins::PluginStatus::NotInstalled => not_installed += 1,
        }
    }

    Ok(CommandResult::Output(format!(
        "Plugin status summary:\n\
         - total: {}\n\
         - installed: {}\n\
         - disabled: {}\n\
         - error: {}\n\
         - not_installed: {}",
        plugins_list.len(),
        installed,
        disabled,
        errored,
        not_installed
    )))
}

fn handle_set_enabled(plugin_id: &str, enable: bool) -> Result<CommandResult> {
    if plugin_id.trim().is_empty() {
        let action = if enable { "enable" } else { "disable" };
        bail!("Usage: /plugin {} <plugin-id>", action);
    }

    let mut installed_plugins = plugins::loader::load_installed_plugins();
    let Some(persisted) = installed_plugins.iter_mut().find(|p| p.id == plugin_id) else {
        return Ok(CommandResult::Output(format!(
            "Plugin '{}' not found in installed plugins.",
            plugin_id
        )));
    };

    persisted.status = if enable {
        plugins::PluginStatus::Installed
    } else {
        plugins::PluginStatus::Disabled
    };

    plugins::loader::save_installed_plugins(&installed_plugins)?;

    // Keep in-memory state in sync for current session.
    let new_status = if enable {
        plugins::PluginStatus::Installed
    } else {
        plugins::PluginStatus::Disabled
    };

    if plugins::set_plugin_status(plugin_id, new_status.clone()).is_none() {
        // If the plugin wasn't in-memory yet, refresh registry from disk.
        plugins::clear_plugins();
        plugins::init_plugins();
    }

    let action_done = if enable { "enabled" } else { "disabled" };
    Ok(CommandResult::Output(format!(
        "Plugin '{}' {}.",
        plugin_id, action_done
    )))
}

fn format_status(status: &plugins::PluginStatus) -> &'static str {
    match status {
        plugins::PluginStatus::NotInstalled => "not_installed",
        plugins::PluginStatus::Installed => "installed",
        plugins::PluginStatus::Disabled => "disabled",
        plugins::PluginStatus::Error(_) => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn plugin_help_works() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Plugin management")),
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn plugin_unknown_subcommand() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("wat", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown plugin subcommand")),
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn plugin_enable_missing_id_errors() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("enable", &mut ctx).await;
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Usage: /plugin enable"));
    }
}
