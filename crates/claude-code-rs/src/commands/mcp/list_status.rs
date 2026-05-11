use anyhow::Result;

use super::settings::describe_entry;
use super::{CommandContext, CommandResult};
use crate::ipc::subsystem_types::McpServerConfigEntry;

// ---------------------------------------------------------------------------
// list / status
// ---------------------------------------------------------------------------

pub(super) async fn handle_list(ctx: &CommandContext) -> Result<CommandResult> {
    crate::ipc::runtime_adapters::ensure_installed();
    let entries = cc_ipc::subsystem_handlers::build_mcp_server_config_entries(&ctx.cwd);
    let status =
        cc_ipc::subsystem_handlers::build_mcp_server_info_list_for_cwd_async(&ctx.cwd).await;

    if entries.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers discovered.\n\n\
             Add servers to ~/.cc-rust/settings.json or .cc-rust/settings.json, or run:\n  \
               /mcp add <name> --command=<cmd> [--arg=<arg> …]"
                .to_string(),
        ));
    }

    let browser_count = entries
        .iter()
        .filter(|e| {
            e.browser_mcp.unwrap_or(false) || crate::browser::detection::is_browser_server(&e.name)
        })
        .count();

    let mut lines = Vec::new();
    if browser_count > 0 {
        lines.push(format!(
            "Discovered MCP servers ({}; {} browser):",
            entries.len(),
            browser_count
        ));
    } else {
        lines.push(format!("Discovered MCP servers ({}):", entries.len()));
    }
    lines.push(String::new());

    // Group by scope label for readability.
    let mut by_scope: Vec<(String, Vec<&McpServerConfigEntry>)> = Vec::new();
    for entry in &entries {
        let label = entry.scope.label();
        if let Some(bucket) = by_scope.iter_mut().find(|(l, _)| *l == label) {
            bucket.1.push(entry);
        } else {
            by_scope.push((label, vec![entry]));
        }
    }

    for (label, bucket) in &by_scope {
        lines.push(format!("[{}]", label));
        for entry in bucket {
            let state = status
                .iter()
                .find(|s| s.name == entry.name)
                .map(|s| s.state.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let desc = describe_entry(entry);
            let tag = if entry.browser_mcp.unwrap_or(false)
                || crate::browser::detection::is_browser_server(&entry.name)
            {
                " [browser]"
            } else {
                ""
            };
            lines.push(format!("  {}{} -- {} -- {}", entry.name, tag, state, desc));
        }
        lines.push(String::new());
    }

    if browser_count > 0 {
        lines.push(
            "Browser-tagged servers expose browser-automation tools (navigate, \
             read_page, click, …). See docs/reference/browser-mcp-config.md."
                .to_string(),
        );
    }

    Ok(CommandResult::Output(
        lines.join("\n").trim_end().to_string(),
    ))
}

pub(super) async fn handle_status(ctx: &CommandContext) -> Result<CommandResult> {
    crate::ipc::runtime_adapters::ensure_installed();
    let status =
        cc_ipc::subsystem_handlers::build_mcp_server_info_list_for_cwd_async(&ctx.cwd).await;
    if status.is_empty() {
        return Ok(CommandResult::Output(
            "No MCP servers discovered.".to_string(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("MCP server status ({}):", status.len()));
    lines.push(String::new());
    for info in &status {
        let err = info
            .error
            .as_ref()
            .map(|e| format!(" -- {}", e))
            .unwrap_or_default();
        lines.push(format!(
            "  {} -- {} ({} tools, {} resources){}",
            info.name, info.state, info.tools_count, info.resources_count, err
        ));
    }
    Ok(CommandResult::Output(lines.join("\n")))
}
