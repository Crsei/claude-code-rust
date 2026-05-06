//! `/mcp` — MCP server management (issue #44).
//!
//! Subcommands:
//! - `/mcp list`            - list discovered MCP servers grouped by scope
//! - `/mcp status`          - show live connection status for discovered servers
//! - `/mcp add <name> ...`  - create a new stdio config (persists to user scope)
//! - `/mcp edit <name> ...` - update an existing config (auto-detects scope)
//! - `/mcp remove <name>`   - delete a config from its matching editable scope
//! - `/mcp approve <name...>` - approve `.mcp.json` project servers
//! - `/mcp reject <name...>`  - reject `.mcp.json` project servers
//! - `/mcp connect <name>`  - (runtime) connect an existing server
//! - `/mcp disconnect <name>` - (runtime) disconnect a connected server
//! - `/mcp reconnect <name>`  - (runtime) reconnect a server
//! - `/mcp`                 - show usage help
//!
//! The `add` / `edit` variants accept repeatable `--command=VALUE`,
//! `--arg=VALUE`, `--env=K=V`, `--url=VALUE`, `--transport=stdio|sse|streamable-http`,
//! `--scope=user|project`, and `--browser` flags.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::ipc::subsystem_handlers::{
    build_mcp_server_config_entries, build_mcp_server_info_list_for_cwd_async,
    run_mcp_runtime_operation, McpRuntimeOperation,
};
use crate::ipc::subsystem_types::{ConfigScope, McpServerConfigEntry};
use crate::mcp::{McpOAuthConfig, McpServerConfig};

/// Handler for the `/mcp` slash command.
pub struct McpHandler;

#[async_trait]
impl CommandHandler for McpHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            None => Ok(CommandResult::Output(help_text())),
            Some("help") | Some("-h") | Some("--help") => Ok(CommandResult::Output(help_text())),
            Some("list") | Some("ls") => handle_list(ctx).await,
            Some("status") => handle_status(ctx).await,
            Some("add") => handle_add(&parts[1..], ctx),
            Some("edit") | Some("update") => handle_edit(&parts[1..], ctx),
            Some("remove") | Some("rm") | Some("delete") => handle_remove(&parts[1..], ctx),
            Some("approve") | Some("enable") => handle_mcpjson_decision(&parts[1..], ctx, true),
            Some("reject") | Some("disable") => handle_mcpjson_decision(&parts[1..], ctx, false),
            Some("connect") => handle_connect(&parts[1..], ctx).await,
            Some("disconnect") => handle_disconnect(&parts[1..], ctx).await,
            Some("reconnect") => handle_reconnect(&parts[1..], ctx).await,
            Some("auth") => handle_auth(&parts[1..], ctx).await,
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown mcp subcommand: '{}'.\n\n{}",
                sub,
                help_text()
            ))),
        }
    }
}

fn help_text() -> String {
    "MCP (Model Context Protocol) server management (issue #44).\n\n\
     Usage:\n  \
       /mcp list                       list discovered MCP servers grouped by scope\n  \
       /mcp status                     show live connection status\n  \
       /mcp add <name> [flags]         create a new stdio config (user scope by default)\n  \
       /mcp edit <name> [flags]        update an existing config (auto-detects scope)\n  \
       /mcp remove <name> [--scope=..] delete a config from an editable scope\n  \
       /mcp approve <name...> [--all-project] approve .mcp.json project server(s)\n  \
       /mcp reject <name...>           reject .mcp.json project server(s)\n  \
       /mcp connect <name>             connect an existing server\n  \
       /mcp disconnect <name>          disconnect a connected server\n  \
       /mcp reconnect <name>           reconnect a server\n  \
       /mcp auth start <name>          print OAuth authorization URL\n  \
       /mcp auth complete <name> --code=<code> [--state=<state>] store OAuth token\n  \
       /mcp auth status <name>         show redacted OAuth credential status\n  \
       /mcp auth clear <name>          clear stored OAuth token\n\n\
     Flags for add/edit:\n  \
       --command=<cmd>     executable (stdio transport)\n  \
       --arg=<arg>         positional argument (repeatable)\n  \
       --env=<K=V>         environment variable (repeatable)\n  \
       --url=<url>         URL (sse / streamable-http transports)\n  \
       --transport=stdio|sse|streamable-http   transport kind (default: stdio)\n  \
       --scope=user|project    persistence scope (default: user for add, auto for edit)\n  \
       --oauth-auth-server-metadata-url=<url>  OAuth RFC 8414 metadata URL\n  \
       --oauth-client-id=<id>                  OAuth public client id\n  \
       --oauth-callback-port=<port>            OAuth loopback redirect port\n  \
       --oauth-scope=<scope>                   OAuth scope (repeatable)\n  \
       --browser           tag this server as a browser-MCP server\n\n\
     Discovery sources (low → high precedence):\n\
     - plugin-contributed MCP servers\n\
     - ~/.cc-rust/settings.json (user scope)\n\
     - .cc-rust/settings.json in the current project (project scope)\n"
        .to_string()
}

// ---------------------------------------------------------------------------
// list / status
// ---------------------------------------------------------------------------

async fn handle_list(ctx: &CommandContext) -> Result<CommandResult> {
    let entries = build_mcp_server_config_entries(&ctx.cwd);
    let status = build_mcp_server_info_list_for_cwd_async(&ctx.cwd).await;

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

async fn handle_status(ctx: &CommandContext) -> Result<CommandResult> {
    let status = build_mcp_server_info_list_for_cwd_async(&ctx.cwd).await;
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

// ---------------------------------------------------------------------------
// add / edit
// ---------------------------------------------------------------------------

fn handle_add(rest: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let Some(name) = rest.first() else {
        return Ok(CommandResult::Output(
            "Usage: /mcp add <name> [--command=<cmd>] [--arg=<arg>] [--env=K=V] [--scope=user|project]"
                .to_string(),
        ));
    };
    let flags = parse_flags(&rest[1..]);
    if let Some(msg) = &flags.error {
        return Ok(CommandResult::Output(format!("{}\n\n{}", msg, help_text())));
    }

    let scope = flags.scope.clone().unwrap_or(ConfigScope::User);
    let entry = McpServerConfigEntry {
        name: (*name).to_string(),
        scope: scope.clone(),
        transport: flags
            .transport
            .clone()
            .unwrap_or_else(|| "stdio".to_string()),
        command: flags.command.clone(),
        args: (!flags.args.is_empty()).then(|| flags.args.clone()),
        url: flags.url.clone(),
        headers: None,
        oauth: oauth_from_flags(&flags, None),
        env: (!flags.env.is_empty()).then(|| flags.env.clone()),
        browser_mcp: flags.browser,
        disabled: None,
    };
    if entry.transport == "stdio" && entry.command.is_none() {
        return Ok(CommandResult::Output(
            "`stdio` transport requires --command=<cmd>. Use --transport=sse or --transport=streamable-http with --url=<url> for remote servers."
                .to_string(),
        ));
    }
    if matches!(entry.transport.as_str(), "sse" | "streamable-http") && entry.url.is_none() {
        return Ok(CommandResult::Output(format!(
            "`{}` transport requires --url=<url>.",
            entry.transport
        )));
    }

    persist_upsert(&ctx.cwd, entry).map(CommandResult::Output)
}

fn handle_edit(rest: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let Some(name) = rest.first() else {
        return Ok(CommandResult::Output(
            "Usage: /mcp edit <name> [--command=<cmd>] [--arg=<arg>] [--env=K=V] [--scope=user|project]"
                .to_string(),
        ));
    };
    let flags = parse_flags(&rest[1..]);
    if let Some(msg) = &flags.error {
        return Ok(CommandResult::Output(format!("{}\n\n{}", msg, help_text())));
    }

    // Locate the current entry to edit (respect --scope override if supplied).
    let existing = build_mcp_server_config_entries(&ctx.cwd);
    let current = match flags.scope.as_ref() {
        Some(wanted) => existing
            .iter()
            .find(|e| e.name == *name && e.scope == *wanted),
        None => existing.iter().find(|e| e.name == *name),
    };
    let Some(current) = current.cloned() else {
        return Ok(CommandResult::Output(format!(
            "No MCP server named `{}` found{}. Use /mcp add to create one.",
            name,
            flags
                .scope
                .as_ref()
                .map(|s| format!(" in scope {}", s.label()))
                .unwrap_or_default()
        )));
    };
    if !current.scope.is_editable() {
        return Ok(CommandResult::Output(format!(
            "`{}` is contributed by scope `{}`, which is read-only. Edit the owning config instead.",
            name,
            current.scope.label()
        )));
    }

    // Overlay the flags onto the existing config.
    let args = if !flags.args.is_empty() {
        Some(flags.args.clone())
    } else {
        current.args.clone()
    };
    let env = if !flags.env.is_empty() {
        Some(flags.env.clone())
    } else {
        current.env.clone()
    };
    let transport = flags.transport.clone().unwrap_or(current.transport.clone());
    let command = flags.command.clone().or(current.command.clone());
    let url = flags.url.clone().or(current.url.clone());
    let oauth = oauth_from_flags(&flags, current.oauth.as_ref()).or(current.oauth.clone());
    let browser_mcp = flags.browser.or(current.browser_mcp);

    let entry = McpServerConfigEntry {
        name: (*name).to_string(),
        scope: flags.scope.unwrap_or(current.scope.clone()),
        transport,
        command,
        args,
        url,
        headers: current.headers.clone(),
        oauth,
        env,
        browser_mcp,
        disabled: current.disabled,
    };
    if matches!(entry.transport.as_str(), "sse" | "streamable-http") && entry.url.is_none() {
        return Ok(CommandResult::Output(format!(
            "`{}` transport requires --url=<url>.",
            entry.transport
        )));
    }
    persist_upsert(&ctx.cwd, entry).map(CommandResult::Output)
}

fn persist_upsert(cwd: &std::path::Path, entry: McpServerConfigEntry) -> Result<String> {
    let scope_label = entry.scope.label();
    let name = entry.name.clone();
    let path = match &entry.scope {
        ConfigScope::User => cc_config::settings::user_settings_path(),
        // Keep the write path aligned with the scoped discovery layer —
        // see the aside in `ipc::subsystem_handlers::settings_path_for_scope`.
        ConfigScope::Project => cwd.join(".cc-rust").join("settings.json"),
        _ => {
            return Ok(format!(
                "Cannot upsert `{}` into scope `{}` (read-only).",
                name, scope_label
            ));
        }
    };
    let mut value = read_settings_value(&path)?;
    let obj = match value.as_object_mut() {
        Some(obj) => obj,
        None => {
            return Ok(format!("{} is not a JSON object", path.display()));
        }
    };
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let servers_obj = match servers.as_object_mut() {
        Some(m) => m,
        None => {
            return Ok(format!(
                "{} has a non-object `mcpServers` field",
                path.display()
            ));
        }
    };
    servers_obj.insert(name.clone(), entry_to_settings_value(&entry));
    write_settings_value(&path, &value)?;

    let flags_summary = describe_entry(&entry);
    Ok(format!(
        "Upserted MCP server `{}` in scope `{}` (at {}).\n  {}",
        name,
        scope_label,
        path.display(),
        flags_summary
    ))
}

fn handle_remove(rest: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let Some(name) = rest.first() else {
        return Ok(CommandResult::Output(
            "Usage: /mcp remove <name> [--scope=user|project]".to_string(),
        ));
    };
    let flags = parse_flags(&rest[1..]);
    if let Some(msg) = &flags.error {
        return Ok(CommandResult::Output(format!("{}\n\n{}", msg, help_text())));
    }

    let existing = build_mcp_server_config_entries(&ctx.cwd);
    let matches: Vec<&McpServerConfigEntry> = existing
        .iter()
        .filter(|e| {
            e.name == *name
                && match flags.scope.as_ref() {
                    Some(wanted) => e.scope == *wanted,
                    None => true,
                }
        })
        .collect();

    if matches.is_empty() {
        return Ok(CommandResult::Output(format!(
            "No MCP server named `{}` found{}.",
            name,
            flags
                .scope
                .as_ref()
                .map(|s| format!(" in scope {}", s.label()))
                .unwrap_or_default()
        )));
    }

    if matches.len() > 1 && flags.scope.is_none() {
        let labels: Vec<String> = matches.iter().map(|e| e.scope.label()).collect();
        return Ok(CommandResult::Output(format!(
            "`{}` exists in multiple scopes ({}). Re-run with --scope=<scope> to pick one.",
            name,
            labels.join(", ")
        )));
    }

    // Pick the most specific editable match: prefer Project, then User.
    let target = matches
        .iter()
        .find(|e| e.scope == ConfigScope::Project)
        .or_else(|| matches.iter().find(|e| e.scope == ConfigScope::User))
        .or_else(|| matches.first())
        .cloned();
    let Some(target) = target else {
        return Ok(CommandResult::Output(format!(
            "No removable match for `{}`.",
            name
        )));
    };
    if !target.scope.is_editable() {
        return Ok(CommandResult::Output(format!(
            "`{}` in scope `{}` is read-only. Edit the owning config source to remove it.",
            name,
            target.scope.label()
        )));
    }

    let path = match &target.scope {
        ConfigScope::User => cc_config::settings::user_settings_path(),
        ConfigScope::Project => ctx.cwd.join(".cc-rust").join("settings.json"),
        _ => unreachable!("editable check above covers plugin/ide"),
    };
    let mut value = read_settings_value(&path)?;
    let removed = value
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .and_then(|obj| obj.remove(name.to_string().as_str()))
        .is_some();
    if !removed {
        return Ok(CommandResult::Output(format!(
            "Entry `{}` not found in {}; nothing to remove.",
            name,
            path.display()
        )));
    }
    write_settings_value(&path, &value)?;
    Ok(CommandResult::Output(format!(
        "Removed MCP server `{}` from scope `{}` (at {}).",
        name,
        target.scope.label(),
        path.display()
    )))
}

fn handle_mcpjson_decision(
    rest: &[&str],
    ctx: &mut CommandContext,
    approve: bool,
) -> Result<CommandResult> {
    let parsed = parse_mcpjson_decision_args(rest);
    if let Some(error) = parsed.error {
        return Ok(CommandResult::Output(format!(
            "{}\n\n{}",
            error,
            help_text()
        )));
    }
    if parsed.names.is_empty() {
        let verb = if approve { "approve" } else { "reject" };
        return Ok(CommandResult::Output(format!(
            "Usage: /mcp {verb} <name...>{}",
            if approve { " [--all-project]" } else { "" }
        )));
    }

    persist_mcpjson_decision(&ctx.cwd, &parsed.names, approve, parsed.all_project)
        .map(CommandResult::Output)
}

// ---------------------------------------------------------------------------
// connect / disconnect / reconnect
// ---------------------------------------------------------------------------

async fn handle_connect(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Connect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp connect <name>".to_string(),
        )),
    }
}

async fn handle_disconnect(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Disconnect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp disconnect <name>".to_string(),
        )),
    }
}

async fn handle_reconnect(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first() {
        Some(name) => {
            let report =
                run_mcp_runtime_operation(&ctx.cwd, McpRuntimeOperation::Reconnect, name).await;
            Ok(CommandResult::Output(report.text))
        }
        None => Ok(CommandResult::Output(
            "Usage: /mcp reconnect <name>".to_string(),
        )),
    }
}

async fn handle_auth(rest: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    match rest.first().copied() {
        Some("start") => {
            let Some(name) = rest.get(1) else {
                return Ok(CommandResult::Output(
                    "Usage: /mcp auth start <name>".to_string(),
                ));
            };
            let config = match find_mcp_config(&ctx.cwd, name) {
                Ok(config) => config,
                Err(message) => return Ok(CommandResult::Output(message)),
            };
            match crate::mcp::auth::start_authorization(&config).await {
                Ok(start) => Ok(CommandResult::Output(format!(
                    "OAuth authorization started for MCP server `{}`.\n\
                     Open this URL in a browser:\n{}\n\n\
                     Redirect URI: {}\n\
                     Then run: /mcp auth complete {} --code=<code> --state={}\n\
                     Token store: {}",
                    config.name,
                    start.authorization_url,
                    start.redirect_uri,
                    config.name,
                    start.state,
                    start.token_store_path.display()
                ))),
                Err(err) => Ok(CommandResult::Output(format!(
                    "Failed to start OAuth for MCP server `{}`: {}",
                    config.name, err
                ))),
            }
        }
        Some("complete") => {
            let Some(name) = rest.get(1) else {
                return Ok(CommandResult::Output(
                    "Usage: /mcp auth complete <name> --code=<code> [--state=<state>]".to_string(),
                ));
            };
            let parsed = parse_auth_complete_args(&rest[2..]);
            if let Some(error) = parsed.error {
                return Ok(CommandResult::Output(format!(
                    "{}\n\nUsage: /mcp auth complete <name> --code=<code> [--state=<state>]",
                    error
                )));
            }
            let Some(code) = parsed.code else {
                return Ok(CommandResult::Output(
                    "Usage: /mcp auth complete <name> --code=<code> [--state=<state>]".to_string(),
                ));
            };
            let config = match find_mcp_config(&ctx.cwd, name) {
                Ok(config) => config,
                Err(message) => return Ok(CommandResult::Output(message)),
            };
            match crate::mcp::auth::complete_authorization(&config, &code, parsed.state.as_deref())
                .await
            {
                Ok(_) => Ok(CommandResult::Output(format!(
                    "Stored OAuth credentials for MCP server `{}` in {}. Access token: {}",
                    config.name,
                    crate::mcp::auth::token_store_path().display(),
                    crate::mcp::auth::redact_secret(&code)
                ))),
                Err(err) => Ok(CommandResult::Output(format!(
                    "Failed to complete OAuth for MCP server `{}`: {}",
                    config.name, err
                ))),
            }
        }
        Some("clear") => {
            let Some(name) = rest.get(1) else {
                return Ok(CommandResult::Output(
                    "Usage: /mcp auth clear <name>".to_string(),
                ));
            };
            let config = match find_mcp_config(&ctx.cwd, name) {
                Ok(config) => config,
                Err(message) => return Ok(CommandResult::Output(message)),
            };
            match crate::mcp::auth::clear_stored_token(&config) {
                Ok(true) => Ok(CommandResult::Output(format!(
                    "Cleared OAuth credentials for MCP server `{}`.",
                    config.name
                ))),
                Ok(false) => Ok(CommandResult::Output(format!(
                    "No stored OAuth credentials found for MCP server `{}`.",
                    config.name
                ))),
                Err(err) => Ok(CommandResult::Output(format!(
                    "Failed to clear OAuth credentials for MCP server `{}`: {}",
                    config.name, err
                ))),
            }
        }
        Some("status") => {
            let Some(name) = rest.get(1) else {
                return Ok(CommandResult::Output(
                    "Usage: /mcp auth status <name>".to_string(),
                ));
            };
            let config = match find_mcp_config(&ctx.cwd, name) {
                Ok(config) => config,
                Err(message) => return Ok(CommandResult::Output(message)),
            };
            match crate::mcp::auth::credential_status(&config) {
                Ok(status) => Ok(CommandResult::Output(format!(
                    "OAuth status for MCP server `{}`: configured={} authorized={} expired={} refreshable={}\nToken store: {}",
                    config.name,
                    status.configured,
                    status.authorized,
                    status.expired,
                    status.can_refresh,
                    status.token_store_path.display()
                ))),
                Err(err) => Ok(CommandResult::Output(format!(
                    "Failed to read OAuth status for MCP server `{}`: {}",
                    config.name, err
                ))),
            }
        }
        _ => Ok(CommandResult::Output(
            "Usage: /mcp auth start|complete|status|clear <name>".to_string(),
        )),
    }
}

fn find_mcp_config(cwd: &std::path::Path, server_name: &str) -> Result<McpServerConfig, String> {
    let configs = crate::mcp::discovery::discover_mcp_servers(cwd)
        .map_err(|err| format!("Failed to discover MCP servers: {err}"))?;
    configs
        .into_iter()
        .find(|cfg| cfg.name == server_name)
        .ok_or_else(|| format!("No MCP server named `{}` found.", server_name))
}

// ---------------------------------------------------------------------------
// Flag parsing helpers
// ---------------------------------------------------------------------------

#[derive(Default, Debug, Clone)]
struct ParsedFlags {
    command: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    url: Option<String>,
    transport: Option<String>,
    scope: Option<ConfigScope>,
    oauth_auth_server_metadata_url: Option<String>,
    oauth_client_id: Option<String>,
    oauth_callback_port: Option<u16>,
    oauth_scopes: Vec<String>,
    browser: Option<bool>,
    error: Option<String>,
}

#[derive(Default, Debug, Clone)]
struct ParsedMcpjsonDecisionArgs {
    names: Vec<String>,
    all_project: bool,
    error: Option<String>,
}

#[derive(Default, Debug, Clone)]
struct ParsedAuthCompleteArgs {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

impl ParsedFlags {
    fn oauth_touched(&self) -> bool {
        self.oauth_auth_server_metadata_url.is_some()
            || self.oauth_client_id.is_some()
            || self.oauth_callback_port.is_some()
            || !self.oauth_scopes.is_empty()
    }
}

fn parse_flags(rest: &[&str]) -> ParsedFlags {
    let mut out = ParsedFlags::default();

    for raw in rest {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if let Some(stripped) = raw.strip_prefix("--command=") {
            out.command = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--arg=") {
            out.args.push(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--env=") {
            if let Some(eq) = stripped.find('=') {
                let (k, v) = stripped.split_at(eq);
                out.env.insert(k.to_string(), v[1..].to_string());
            } else {
                out.error = Some(format!("malformed --env value: {}", stripped));
            }
        } else if let Some(stripped) = raw.strip_prefix("--url=") {
            out.url = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--transport=") {
            out.transport = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--scope=") {
            out.scope = Some(match stripped {
                "user" => ConfigScope::User,
                "project" => ConfigScope::Project,
                other => {
                    out.error = Some(format!(
                        "invalid --scope `{}` (expected user|project)",
                        other
                    ));
                    ConfigScope::User
                }
            });
        } else if let Some(stripped) = raw.strip_prefix("--oauth-auth-server-metadata-url=") {
            out.oauth_auth_server_metadata_url = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--oauth-client-id=") {
            out.oauth_client_id = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--oauth-callback-port=") {
            match stripped.parse::<u16>() {
                Ok(port) => out.oauth_callback_port = Some(port),
                Err(_) => {
                    out.error = Some(format!(
                        "invalid --oauth-callback-port `{}` (expected 1-65535)",
                        stripped
                    ));
                }
            }
        } else if let Some(stripped) = raw.strip_prefix("--oauth-scope=") {
            if stripped.trim().is_empty() {
                out.error = Some("--oauth-scope must not be empty".to_string());
            } else {
                out.oauth_scopes.push(stripped.to_string());
            }
        } else if raw == "--browser" {
            out.browser = Some(true);
        } else if let Some(stripped) = raw.strip_prefix("--browser=") {
            out.browser = match stripped {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                other => {
                    out.error = Some(format!("invalid --browser value `{}`", other));
                    None
                }
            };
        } else {
            out.error = Some(format!("unknown flag `{}`", raw));
        }
    }

    out
}

fn parse_auth_complete_args(rest: &[&str]) -> ParsedAuthCompleteArgs {
    let mut out = ParsedAuthCompleteArgs::default();
    for raw in rest {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if let Some(stripped) = raw.strip_prefix("--code=") {
            out.code = Some(stripped.to_string());
        } else if let Some(stripped) = raw.strip_prefix("--state=") {
            out.state = Some(stripped.to_string());
        } else if raw.starts_with("--") {
            out.error = Some(format!("unknown flag `{}`", raw));
        } else if out.code.is_none() {
            out.code = Some(raw.to_string());
        } else {
            out.error = Some(format!("unexpected OAuth auth argument `{}`", raw));
        }
    }
    out
}

fn oauth_from_flags(
    flags: &ParsedFlags,
    current: Option<&McpOAuthConfig>,
) -> Option<McpOAuthConfig> {
    if !flags.oauth_touched() {
        return None;
    }
    let mut oauth = current.cloned().unwrap_or_default();
    if let Some(value) = &flags.oauth_auth_server_metadata_url {
        oauth.auth_server_metadata_url = Some(value.clone());
    }
    if let Some(value) = &flags.oauth_client_id {
        oauth.client_id = Some(value.clone());
    }
    if let Some(value) = flags.oauth_callback_port {
        oauth.callback_port = Some(value);
    }
    if !flags.oauth_scopes.is_empty() {
        oauth.scopes = Some(flags.oauth_scopes.clone());
    }
    Some(oauth)
}

fn parse_mcpjson_decision_args(rest: &[&str]) -> ParsedMcpjsonDecisionArgs {
    let mut out = ParsedMcpjsonDecisionArgs::default();

    for raw in rest {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if raw == "--all-project" {
            out.all_project = true;
        } else if raw.starts_with("--") {
            out.error = Some(format!("unknown flag `{}`", raw));
        } else {
            out.names.push(raw.to_string());
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Misc helpers — shared between subcommands
// ---------------------------------------------------------------------------

fn describe_entry(entry: &McpServerConfigEntry) -> String {
    let mut parts = Vec::new();
    parts.push(format!("transport={}", entry.transport));
    if let Some(cmd) = &entry.command {
        if let Some(args) = &entry.args {
            parts.push(format!("command=\"{} {}\"", cmd, args.join(" ")));
        } else {
            parts.push(format!("command=\"{}\"", cmd));
        }
    }
    if let Some(url) = &entry.url {
        parts.push(format!("url=\"{}\"", url));
    }
    if entry.oauth.is_some() {
        parts.push("oauth=configured".to_string());
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            let mut keys: Vec<&String> = env.keys().collect();
            keys.sort();
            parts.push(format!(
                "env=[{}]",
                keys.into_iter().cloned().collect::<Vec<_>>().join(",")
            ));
        }
    }
    parts.join(" ")
}

fn read_settings_value(path: &std::path::Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    Ok(serde_json::from_str(&content)?)
}

fn write_settings_value(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(value)?;
    let tmp: PathBuf = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn persist_mcpjson_decision(
    cwd: &std::path::Path,
    names: &[String],
    approve: bool,
    all_project: bool,
) -> Result<String> {
    let path = cwd.join(".cc-rust").join("settings.json");
    let mut value = read_settings_value(&path)?;
    let obj = match value.as_object_mut() {
        Some(obj) => obj,
        None => {
            return Ok(format!("{} is not a JSON object", path.display()));
        }
    };

    if approve {
        add_string_array_entries(obj, "enabledMcpjsonServers", names);
        remove_string_array_entries(obj, "disabledMcpjsonServers", names);
        if all_project {
            obj.insert(
                "enableAllProjectMcpServers".to_string(),
                serde_json::Value::Bool(true),
            );
        }
    } else {
        add_string_array_entries(obj, "disabledMcpjsonServers", names);
        remove_string_array_entries(obj, "enabledMcpjsonServers", names);
    }

    write_settings_value(&path, &value)?;

    let verb = if approve { "Approved" } else { "Rejected" };
    let mut suffix = String::new();
    if approve && all_project {
        suffix.push_str("\n  Future project MCP servers will be approved automatically.");
    }
    Ok(format!(
        "{} .mcp.json server(s) `{}` in project scope (at {}).{}",
        verb,
        names.join("`, `"),
        path.display(),
        suffix
    ))
}

fn add_string_array_entries(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    names: &[String],
) {
    let value = obj
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if !value.is_array() {
        *value = serde_json::Value::Array(Vec::new());
    }
    let Some(array) = value.as_array_mut() else {
        return;
    };
    for name in names {
        if !array.iter().any(|item| item.as_str() == Some(name)) {
            array.push(serde_json::Value::String(name.clone()));
        }
    }
}

fn remove_string_array_entries(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    names: &[String],
) {
    let Some(array) = obj.get_mut(key).and_then(|value| value.as_array_mut()) else {
        return;
    };
    array.retain(|item| {
        item.as_str()
            .map(|name| !names.iter().any(|blocked| blocked == name))
            .unwrap_or(true)
    });
}

fn entry_to_settings_value(entry: &McpServerConfigEntry) -> serde_json::Value {
    let cfg = McpServerConfig {
        name: entry.name.clone(),
        transport: entry.transport.clone(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        url: entry.url.clone(),
        headers: entry.headers.clone(),
        oauth: entry.oauth.clone(),
        env: entry.env.clone(),
        browser_mcp: entry.browser_mcp,
        disabled: entry.disabled,
    };
    let mut v = serde_json::to_value(&cfg).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.remove("name");
    }
    v
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

    fn test_ctx(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct RuntimeMcpGuard;

    impl RuntimeMcpGuard {
        fn install(
            manager: std::sync::Arc<tokio::sync::Mutex<crate::mcp::manager::McpManager>>,
        ) -> Self {
            crate::mcp::runtime::clear_for_tests();
            crate::mcp::runtime::install_manager(manager);
            Self
        }
    }

    impl Drop for RuntimeMcpGuard {
        fn drop(&mut self) {
            crate::mcp::runtime::clear_for_tests();
        }
    }

    #[tokio::test]
    async fn mcp_no_args_shows_help() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/test/project"));
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("MCP"));
                assert!(text.contains("/mcp add"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_list_no_servers_suggests_add() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("No MCP servers discovered") || text.contains("Discovered"),
                    "unexpected output: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_add_persists_user_scope() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        let res = handler
            .execute(
                "add ctx7 --command=npx --arg=-y --arg=ctx7 --env=FOO=bar",
                &mut ctx,
            )
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("Upserted MCP server `ctx7`") && text.contains("scope `user`"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }

        let settings = home.path().join("settings.json");
        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(disk["mcpServers"]["ctx7"]["command"], "npx");
        assert_eq!(disk["mcpServers"]["ctx7"]["args"][1], "ctx7");
        assert_eq!(disk["mcpServers"]["ctx7"]["env"]["FOO"], "bar");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_add_persists_oauth_metadata_without_tokens() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        handler
            .execute(
                "add remote --transport=sse --url=https://mcp.example.com/sse \
                 --oauth-auth-server-metadata-url=https://auth.example.com/.well-known/oauth-authorization-server \
                 --oauth-client-id=cc-rust-test --oauth-callback-port=18888 --oauth-scope=tools.read",
                &mut ctx,
            )
            .await
            .unwrap();

        let settings = home.path().join("settings.json");
        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        let oauth = &disk["mcpServers"]["remote"]["oauth"];
        assert_eq!(oauth["clientId"], "cc-rust-test");
        assert_eq!(oauth["callbackPort"], 18888);
        assert_eq!(oauth["scopes"][0], "tools.read");
        assert!(oauth.get("accessToken").is_none());
        assert!(oauth.get("refreshToken").is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_edit_updates_command() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        handler
            .execute("add mysrv --command=./old.sh", &mut ctx)
            .await
            .unwrap();
        let res = handler
            .execute("edit mysrv --command=./new.sh --arg=foo", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("Upserted MCP server `mysrv`"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }

        let settings = home.path().join("settings.json");
        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(disk["mcpServers"]["mysrv"]["command"], "./new.sh");
        assert_eq!(disk["mcpServers"]["mysrv"]["args"][0], "foo");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_remove_deletes_from_user_scope() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        handler
            .execute("add goner --command=x", &mut ctx)
            .await
            .unwrap();
        let res = handler.execute("remove goner", &mut ctx).await.unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("Removed MCP server `goner`"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }

        let settings = home.path().join("settings.json");
        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        let servers = disk
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers");
        assert!(!servers.contains_key("goner"), "goner should be removed");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_remove_ambiguous_requires_scope() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        // Create both user and project rows with the same name.
        handler
            .execute("add dupe --command=u --scope=user", &mut ctx)
            .await
            .unwrap();
        handler
            .execute("add dupe --command=p --scope=project", &mut ctx)
            .await
            .unwrap();

        let res = handler.execute("remove dupe", &mut ctx).await.unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("exists in multiple scopes") && text.contains("--scope"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_approve_project_mcp_json_servers_records_enabled() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let project_settings = cwd.path().join(".cc-rust").join("settings.json");
        std::fs::create_dir_all(project_settings.parent().unwrap()).unwrap();
        std::fs::write(
            &project_settings,
            serde_json::to_string_pretty(&serde_json::json!({
                "enabledMcpjsonServers": ["existing"],
                "disabledMcpjsonServers": ["github", "old"],
                "enableAllProjectMcpServers": false
            }))
            .unwrap(),
        )
        .unwrap();

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        let res = handler
            .execute("approve github playwright --all-project", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("Approved .mcp.json server(s)")
                    && text.contains("Future project MCP servers"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }

        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&project_settings).unwrap()).unwrap();
        assert_eq!(disk["enabledMcpjsonServers"][0], "existing");
        assert_eq!(disk["enabledMcpjsonServers"][1], "github");
        assert_eq!(disk["enabledMcpjsonServers"][2], "playwright");
        assert_eq!(disk["disabledMcpjsonServers"][0], "old");
        assert_eq!(disk["enableAllProjectMcpServers"], true);
        assert!(
            !home.path().join("settings.json").exists(),
            "approval must stay in project .cc-rust settings"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_reject_project_mcp_json_servers_records_disabled() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let project_settings = cwd.path().join(".cc-rust").join("settings.json");
        std::fs::create_dir_all(project_settings.parent().unwrap()).unwrap();
        std::fs::write(
            &project_settings,
            serde_json::to_string_pretty(&serde_json::json!({
                "enabledMcpjsonServers": ["github", "keep"],
                "disabledMcpjsonServers": ["old"]
            }))
            .unwrap(),
        )
        .unwrap();

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        let res = handler
            .execute("reject github sentry", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(
                text.contains("Rejected .mcp.json server(s)"),
                "unexpected: {}",
                text
            ),
            _ => panic!("expected Output"),
        }

        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&project_settings).unwrap()).unwrap();
        assert_eq!(disk["enabledMcpjsonServers"][0], "keep");
        assert_eq!(disk["disabledMcpjsonServers"][0], "old");
        assert_eq!(disk["disabledMcpjsonServers"][1], "github");
        assert_eq!(disk["disabledMcpjsonServers"][2], "sentry");
        assert!(
            !home.path().join("settings.json").exists(),
            "rejection must stay in project .cc-rust settings"
        );
    }

    #[tokio::test]
    async fn mcp_add_stdio_requires_command() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler.execute("add nocmd", &mut ctx).await.unwrap();
        match res {
            CommandResult::Output(text) => {
                assert!(text.contains("requires --command"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn mcp_sse_requires_url() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler
            .execute("add sse-only --transport=sse", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(text.contains("requires --url")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn mcp_streamable_http_requires_url() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler
            .execute("add http-only --transport=streamable-http", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => assert!(text.contains("requires --url")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn mcp_connect_requires_name() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler.execute("connect", &mut ctx).await.unwrap();
        match res {
            CommandResult::Output(text) => assert!(text.contains("Usage: /mcp connect")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_reconnect_uses_runtime_manager() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let manager = std::sync::Arc::new(tokio::sync::Mutex::new(
            crate::mcp::manager::McpManager::new(),
        ));
        let _runtime = RuntimeMcpGuard::install(manager.clone());
        std::fs::write(
            home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "rec-srv": {
                        "type": "stdio",
                        "command": "unused",
                        "disabled": true
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let handler = McpHandler;
        let mut ctx = test_ctx(cwd.path().to_path_buf());
        let res = handler
            .execute("reconnect rec-srv", &mut ctx)
            .await
            .unwrap();
        match res {
            CommandResult::Output(text) => {
                assert!(text.contains("rec-srv"));
                assert!(text.contains("disabled"));
            }
            _ => panic!("expected Output"),
        }
        assert!(
            manager.lock().await.clients.is_empty(),
            "disabled reconnect must not keep a live client"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn mcp_status_reports_server_list() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler.execute("status", &mut ctx).await.unwrap();
        // Should return either "No MCP servers discovered." or a formatted list.
        match res {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("MCP server status") || text.contains("No MCP servers"),
                    "unexpected output: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn mcp_unknown_subcommand_shows_help() {
        let handler = McpHandler;
        let mut ctx = test_ctx(PathBuf::from("/tmp"));
        let res = handler.execute("foobar", &mut ctx).await.unwrap();
        match res {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown mcp subcommand"));
                assert!(text.contains("/mcp add"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn parse_flags_env_splits_on_first_equal() {
        let flags = parse_flags(&["--env=A=B=C"]);
        assert!(flags.error.is_none());
        assert_eq!(flags.env.get("A").map(String::as_str), Some("B=C"));
    }

    #[test]
    fn parse_flags_invalid_scope_emits_error() {
        let flags = parse_flags(&["--scope=bogus"]);
        assert!(flags.error.as_ref().unwrap().contains("invalid --scope"));
    }
}
