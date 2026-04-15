//! /mcp command - MCP server management.
//!
//! Subcommands:
//! - `/mcp list`   - list discovered MCP servers (settings + plugins)
//! - `/mcp status` - show current status view for discovered servers
//! - `/mcp`        - show usage help

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::mcp::{self, McpServerConfig};

/// Handler for the `/mcp` slash command.
pub struct McpHandler;

#[async_trait]
impl CommandHandler for McpHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("list") | Some("ls") => handle_list(ctx),
            Some("status") => handle_status(ctx),
            None => handle_help(),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown mcp subcommand: '{}'\n\
                 Usage:\n  \
                   /mcp list    -- list discovered MCP servers\n  \
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
           /mcp list    -- list discovered MCP servers\n  \
           /mcp status  -- show connection status\n\n\
         Discovery sources:\n\
         - plugin-contributed MCP servers\n\
         - ~/.cc-rust/settings.json (mcpServers)\n\
         - .cc-rust/settings.json in the current project\n\n\
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

/// List discovered MCP servers.
fn handle_list(ctx: &CommandContext) -> Result<CommandResult> {
    let servers = discover_servers(ctx);

    if servers.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers discovered.\n\n\
             Add mcpServers in ~/.cc-rust/settings.json or .cc-rust/settings.json,\n\
             or install plugins that contribute MCP servers."
                .to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("Discovered MCP servers ({}):", servers.len()));
    lines.push(String::new());

    for server in &servers {
        let command = server.command.as_deref().unwrap_or("(unknown)");
        let args = server.args.clone().unwrap_or_default().join(" ");
        if args.is_empty() {
            lines.push(format!(
                "  {} -- transport: {} -- command: {}",
                server.name, server.transport, command
            ));
        } else {
            lines.push(format!(
                "  {} -- transport: {} -- command: {} {}",
                server.name, server.transport, command, args
            ));
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Show connection status of discovered servers.
fn handle_status(ctx: &CommandContext) -> Result<CommandResult> {
    let servers = discover_servers(ctx);

    if servers.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers discovered.".to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("MCP server status ({}):", servers.len()));
    lines.push(String::new());

    for server in &servers {
        // `/mcp` is currently a discovery/introspection surface.
        // Live connection state is available via SystemStatus / headless IPC.
        lines.push(format!(
            "  {} -- pending (runtime status via SystemStatus)",
            server.name
        ));
    }

    lines.push(String::new());
    lines.push("Note: MCP servers are connected during startup and tool registration.".to_string());

    Ok(CommandResult::Output(lines.join("\n")))
}

fn discover_servers(ctx: &CommandContext) -> Vec<McpServerConfig> {
    mcp::discovery::discover_mcp_servers(&ctx.cwd).unwrap_or_default()
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
