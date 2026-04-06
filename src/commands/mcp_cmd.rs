//! /mcp command -- MCP server management.
//!
//! Subcommands:
//! - `/mcp list`   -- list configured MCP servers from settings
//! - `/mcp status` -- show connection status of MCP servers
//! - `/mcp`        -- show usage help

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings;

/// Handler for the `/mcp` slash command.
pub struct McpHandler;

#[async_trait]
impl CommandHandler for McpHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("list") | Some("ls") => handle_list(),
            Some("status") => handle_status(),
            None => handle_help(),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown mcp subcommand: '{}'\n\
                 Usage:\n  \
                   /mcp list    -- list configured MCP servers\n  \
                   /mcp status  -- show connection status",
                sub
            ))),
        }
    }
}

/// Show usage help.
fn handle_help() -> Result<CommandResult> {
    Ok(CommandResult::Output(
        "MCP (Model Context Protocol) server management.\n\n\
         Usage:\n  \
           /mcp list    -- list configured MCP servers\n  \
           /mcp status  -- show connection status\n\n\
         MCP servers are configured in ~/.cc-rust/settings.json under the \"mcpServers\" key.\n\n\
         Example settings.json:\n\
         {\n  \
           \"mcpServers\": {\n    \
             \"my-server\": {\n      \
               \"command\": \"npx\",\n      \
               \"args\": [\"-y\", \"my-mcp-server\"]\n    \
             }\n  \
           }\n\
         }"
            .to_string(),
    ))
}

/// List configured MCP servers from settings.
fn handle_list() -> Result<CommandResult> {
    let servers = load_mcp_servers();

    if servers.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers configured.\n\n\
             Add MCP servers in ~/.cc-rust/settings.json under the \"mcpServers\" key."
                .to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("Configured MCP servers ({}):", servers.len()));
    lines.push(String::new());

    for (name, config) in &servers {
        let command = config
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");
        let args = config
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        if args.is_empty() {
            lines.push(format!("  {} -- command: {}", name, command));
        } else {
            lines.push(format!("  {} -- command: {} {}", name, command, args));
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Show connection status of MCP servers.
fn handle_status() -> Result<CommandResult> {
    let servers = load_mcp_servers();

    if servers.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers configured.".to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("MCP server status ({}):", servers.len()));
    lines.push(String::new());

    for (name, _config) in &servers {
        // In a full implementation, we would check actual connection state.
        // For now, report as "not connected" since we don't have a running
        // MCP client manager yet.
        lines.push(format!("  {} -- not connected (runtime not active)", name));
    }

    lines.push(String::new());
    lines.push(
        "Note: MCP connections are established when tools from a server are first used."
            .to_string(),
    );

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Load MCP server configuration from `~/.cc-rust/settings.json`.
fn load_mcp_servers() -> std::collections::BTreeMap<String, serde_json::Value> {
    let path = match settings::global_claude_dir() {
        Ok(dir) => dir.join("settings.json"),
        Err(_) => return std::collections::BTreeMap::new(),
    };

    if !path.exists() {
        return std::collections::BTreeMap::new();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return std::collections::BTreeMap::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return std::collections::BTreeMap::new(),
    };

    match parsed.get("mcpServers") {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        _ => std::collections::BTreeMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    async fn test_mcp_no_args_shows_help() {
        let handler = McpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("MCP"));
                assert!(text.contains("mcpServers"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_mcp_list() {
        let handler = McpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Either "No MCP servers" or a listing -- both are valid.
                assert!(
                    text.contains("MCP") || text.contains("No MCP"),
                    "Unexpected: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_mcp_status() {
        let handler = McpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("MCP") || text.contains("No MCP"),
                    "Unexpected: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_mcp_unknown_subcommand() {
        let handler = McpHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown mcp subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
