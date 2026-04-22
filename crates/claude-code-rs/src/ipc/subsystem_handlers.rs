//! Subsystem command handlers and status snapshot builders.
//!
//! **Command handlers** respond to `FrontendMessage` commands for each subsystem
//! (LSP, MCP, Plugin, Skill).  Each handler returns a `Vec<BackendMessage>`
//! that the caller sends via the [`FrontendSink`].  Handlers never write to
//! stdout directly.
//!
//! **Status snapshot builders** assemble point-in-time status objects from
//! each subsystem's in-memory state.  These are used by `QueryStatus` commands
//! and the `SystemStatus` tool.

use std::path::PathBuf;

use super::protocol::BackendMessage;
use super::subsystem_events::{IdeEvent, LspEvent, McpEvent, PluginEvent, SkillEvent};
use super::subsystem_types::*;
use cc_mcp::discovery::DiscoveryScope;

// ===========================================================================
// Command handlers (return value pattern — no direct I/O)
// ===========================================================================

/// Handle an LSP subsystem command from the frontend.
///
/// Lifecycle operations (start/stop/restart) are deferred to the `/lsp` slash
/// command because `LSP_CLIENTS` is a private static.  `QueryStatus` builds a
/// server list from the default configurations and returns it.
pub fn handle_lsp_command(cmd: super::subsystem_events::LspCommand) -> Vec<BackendMessage> {
    use super::subsystem_events::LspCommand;

    match cmd {
        LspCommand::StartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP start requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To start the {} server, run: /lsp start {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            }]
        }
        LspCommand::StopServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP stop requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To stop the {} server, run: /lsp stop {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            }]
        }
        LspCommand::RestartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP restart requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To restart the {} server, run: /lsp restart {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            }]
        }
        LspCommand::QueryStatus => {
            let servers = build_lsp_server_info_list();
            vec![BackendMessage::LspEvent {
                event: LspEvent::ServerList { servers },
            }]
        }
        LspCommand::QuerySettings => {
            let settings = load_lsp_recommendation_settings();
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
        LspCommand::RecommendationResponse {
            request_id,
            plugin_name,
            decision,
        } => {
            tracing::info!(
                request_id = %request_id,
                plugin_name = %plugin_name,
                decision = %decision,
                "LSP recommendation response"
            );
            let (settings, info_text) = apply_recommendation_decision(&plugin_name, &decision);
            let mut msgs = Vec::with_capacity(2);
            msgs.push(BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            });
            if let Some(text) = info_text {
                msgs.push(BackendMessage::SystemInfo {
                    text,
                    level: "info".to_string(),
                });
            }
            msgs
        }
        LspCommand::UnmutePlugin { plugin_name } => {
            let settings = unmute_lsp_plugin(&plugin_name);
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
        LspCommand::SetRecommendationsDisabled { disabled } => {
            let settings = set_lsp_recommendations_disabled(disabled);
            vec![BackendMessage::LspEvent {
                event: LspEvent::SettingsSnapshot { settings },
            }]
        }
    }
}

// ---------------------------------------------------------------------------
// LSP recommendation settings persistence
// ---------------------------------------------------------------------------

/// Key inside the user-level `settings.json` object that holds the
/// `LspRecommendationSettings` payload.
const LSP_RECOMMENDATIONS_KEY: &str = "lspRecommendations";

/// Read the `lspRecommendations` block from the user settings file.
///
/// Returns a `Default` value when the file is missing, the key is
/// absent, or the stored value fails to deserialize — a corrupt entry
/// should never brick the prompt pipeline.
pub fn load_lsp_recommendation_settings() -> LspRecommendationSettings {
    let path = cc_config::settings::user_settings_path();
    let Ok(value) = read_settings_value(&path) else {
        return LspRecommendationSettings::default();
    };
    value
        .get(LSP_RECOMMENDATIONS_KEY)
        .and_then(|v| serde_json::from_value::<LspRecommendationSettings>(v.clone()).ok())
        .unwrap_or_default()
}

/// Persist `settings` under the `lspRecommendations` key, preserving every
/// other field in the user settings file.
fn save_lsp_recommendation_settings(settings: &LspRecommendationSettings) {
    let path = cc_config::settings::user_settings_path();
    let mut value = read_settings_value(&path).unwrap_or_else(|err| {
        tracing::warn!(error = %err, "LSP recommendations: read user settings failed; overwriting with fresh object");
        serde_json::Value::Object(serde_json::Map::new())
    });
    if !value.is_object() {
        value = serde_json::Value::Object(serde_json::Map::new());
    }
    let encoded = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, "LSP recommendations: serialize failed");
            return;
        }
    };
    value
        .as_object_mut()
        .expect("value is object")
        .insert(LSP_RECOMMENDATIONS_KEY.to_string(), encoded);
    if let Err(err) = write_settings_value(&path, &value) {
        tracing::warn!(error = %err, "LSP recommendations: write user settings failed");
    }
}

/// Apply a user decision from an [`LspEvent::RecommendationRequest`] prompt.
///
/// Returns the updated settings snapshot plus an optional info-level
/// message the frontend can surface in its system log. `yes` / `no` do
/// not mutate persistent state — the install itself is carried out
/// elsewhere (or postponed to the next session); only `never` and
/// `disable` are sticky.
fn apply_recommendation_decision(
    plugin_name: &str,
    decision: &str,
) -> (LspRecommendationSettings, Option<String>) {
    let mut settings = load_lsp_recommendation_settings();
    let info = match decision {
        "yes" => Some(format!(
            "Install of LSP plugin '{}' is not yet wired up — track in /lsp when the install path lands.",
            plugin_name
        )),
        "no" => None,
        "never" => {
            if !settings.muted_plugins.iter().any(|p| p == plugin_name) {
                settings.muted_plugins.push(plugin_name.to_string());
                save_lsp_recommendation_settings(&settings);
            }
            Some(format!(
                "Muted LSP recommendation for '{}'. Run /lsp to undo.",
                plugin_name
            ))
        }
        "disable" => {
            if !settings.disabled {
                settings.disabled = true;
                save_lsp_recommendation_settings(&settings);
            }
            Some("Disabled all LSP plugin recommendations. Run /lsp to re-enable.".to_string())
        }
        other => {
            tracing::warn!(decision = %other, "LSP recommendations: unknown decision value");
            None
        }
    };
    (settings, info)
}

/// Remove `plugin_name` from the muted list and persist the result.
fn unmute_lsp_plugin(plugin_name: &str) -> LspRecommendationSettings {
    let mut settings = load_lsp_recommendation_settings();
    let before = settings.muted_plugins.len();
    settings.muted_plugins.retain(|p| p != plugin_name);
    if settings.muted_plugins.len() != before {
        save_lsp_recommendation_settings(&settings);
    }
    settings
}

/// Flip the global "disable all recommendations" switch.
fn set_lsp_recommendations_disabled(disabled: bool) -> LspRecommendationSettings {
    let mut settings = load_lsp_recommendation_settings();
    if settings.disabled != disabled {
        settings.disabled = disabled;
        save_lsp_recommendation_settings(&settings);
    }
    settings
}

/// Handle an MCP subsystem command from the frontend.
///
/// Lifecycle operations (`ConnectServer` / `DisconnectServer` /
/// `ReconnectServer`) surface an info-level system message pointing at the
/// `/mcp` slash command; the actual live-state mutation lives in the
/// manager owned by the query engine.
///
/// `QueryStatus` builds a runtime-state list; `QueryConfig`/`UpsertConfig`/
/// `RemoveConfig` implement the scope-aware config editor (issue #44).
pub fn handle_mcp_command(cmd: super::subsystem_events::McpCommand) -> Vec<BackendMessage> {
    use super::subsystem_events::McpCommand;

    match cmd {
        McpCommand::ConnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP connect requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To connect {}, run: /mcp connect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            }]
        }
        McpCommand::DisconnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP disconnect requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To disconnect {}, run: /mcp disconnect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            }]
        }
        McpCommand::ReconnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP reconnect requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To reconnect {}, run: /mcp reconnect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            }]
        }
        McpCommand::QueryStatus => {
            let servers = build_mcp_server_info_list();
            vec![BackendMessage::McpEvent {
                event: McpEvent::ServerList { servers },
            }]
        }
        McpCommand::QueryConfig => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let entries = build_mcp_server_config_entries(&cwd);
            vec![BackendMessage::McpEvent {
                event: McpEvent::ConfigList { entries },
            }]
        }
        McpCommand::UpsertConfig { entry } => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match upsert_mcp_entry(&cwd, entry) {
                Ok(updated) => vec![BackendMessage::McpEvent {
                    event: McpEvent::ConfigChanged {
                        server_name: updated.name.clone(),
                        entry: Some(updated),
                    },
                }],
                Err((server_name, message)) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %message,
                        "MCP: upsert_config rejected"
                    );
                    vec![BackendMessage::McpEvent {
                        event: McpEvent::ConfigError {
                            server_name,
                            error: message,
                        },
                    }]
                }
            }
        }
        McpCommand::RemoveConfig { server_name, scope } => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match remove_mcp_entry(&cwd, &server_name, &scope) {
                Ok(()) => vec![BackendMessage::McpEvent {
                    event: McpEvent::ConfigChanged {
                        server_name,
                        entry: None,
                    },
                }],
                Err(message) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %message,
                        "MCP: remove_config rejected"
                    );
                    vec![BackendMessage::McpEvent {
                        event: McpEvent::ConfigError {
                            server_name,
                            error: message,
                        },
                    }]
                }
            }
        }
    }
}

/// Handle a plugin subsystem command from the frontend.
///
/// Enable/disable are deferred to the `/plugin` slash command.
/// `QueryStatus` returns the full plugin list.
pub fn handle_plugin_command(cmd: super::subsystem_events::PluginCommand) -> Vec<BackendMessage> {
    use super::subsystem_events::PluginCommand;

    match cmd {
        PluginCommand::Enable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin enable requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To enable {}, run: /plugin enable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            }]
        }
        PluginCommand::Disable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin disable requested via IPC");
            vec![BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To disable {}, run: /plugin disable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            }]
        }
        PluginCommand::QueryStatus => {
            let plugins = build_plugin_info_list();
            vec![BackendMessage::PluginEvent {
                event: PluginEvent::PluginList { plugins },
            }]
        }
        PluginCommand::Reload => {
            tracing::info!("Plugin reload requested via IPC");
            let report = crate::plugins::reload_plugins();
            vec![BackendMessage::PluginEvent {
                event: PluginEvent::Reloaded {
                    count: report.count,
                    had_error: report.had_error(),
                },
            }]
        }
        PluginCommand::Uninstall {
            plugin_id,
            purge_cache,
        } => {
            tracing::info!(
                plugin_id = %plugin_id,
                purge_cache,
                "Plugin uninstall requested via IPC"
            );
            match crate::plugins::uninstall_plugin(&plugin_id, purge_cache) {
                Ok(Some(entry)) => vec![BackendMessage::PluginEvent {
                    event: PluginEvent::StatusChanged {
                        plugin_id: entry.id.clone(),
                        name: entry.name.clone(),
                        status: "not_installed".to_string(),
                        error: None,
                    },
                }],
                Ok(None) => vec![BackendMessage::SystemInfo {
                    text: format!("Plugin '{}' is not installed.", plugin_id),
                    level: "warn".to_string(),
                }],
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("Failed to uninstall '{}': {}", plugin_id, e),
                    level: "error".to_string(),
                }],
            }
        }
    }
}

/// Handle an IDE subsystem command from the frontend (issue #41).
///
/// - `Detect` / `QueryStatus` re-run detection and return the current list.
/// - `Select` / `Clear` persist the user's selection through `crate::ide`.
/// - `Reconnect` re-triggers a `ConnectionStateChanged` event so the MCP
///   manager notices the selection on its next discovery pass.
pub fn handle_ide_command(cmd: super::subsystem_events::IdeCommand) -> Vec<BackendMessage> {
    use super::subsystem_events::IdeCommand;

    match cmd {
        IdeCommand::Detect | IdeCommand::QueryStatus => {
            let ides = build_ide_info_list();
            vec![BackendMessage::IdeEvent {
                event: IdeEvent::IdeList { ides },
            }]
        }
        IdeCommand::Select { ide_id } => {
            tracing::info!(ide_id = %ide_id, "IDE select requested via IPC");
            match crate::ide::select_ide(&ide_id) {
                Ok(()) => {
                    let ides = build_ide_info_list();
                    vec![BackendMessage::IdeEvent {
                        event: IdeEvent::IdeList { ides },
                    }]
                }
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE select failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
        IdeCommand::Clear => {
            tracing::info!("IDE selection clear requested via IPC");
            match crate::ide::clear_selection() {
                Ok(()) => {
                    let ides = build_ide_info_list();
                    vec![BackendMessage::IdeEvent {
                        event: IdeEvent::IdeList { ides },
                    }]
                }
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE clear failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
        IdeCommand::Reconnect => {
            tracing::info!("IDE reconnect requested via IPC");
            match crate::ide::reconnect_selected() {
                Ok(()) => vec![BackendMessage::SystemInfo {
                    text: "IDE reconnect scheduled".to_string(),
                    level: "info".to_string(),
                }],
                Err(e) => vec![BackendMessage::SystemInfo {
                    text: format!("IDE reconnect failed: {}", e),
                    level: "error".to_string(),
                }],
            }
        }
    }
}

/// Handle a skill subsystem command from the frontend.
///
/// `Reload` clears and re-initialises the skill registry.
/// `QueryStatus` returns the full skill list.
pub fn handle_skill_command(cmd: super::subsystem_events::SkillCommand) -> Vec<BackendMessage> {
    use super::subsystem_events::SkillCommand;

    match cmd {
        SkillCommand::Reload => {
            let cwd = std::env::current_dir().ok();
            crate::skills::clear_skills();
            crate::skills::init_skills(
                &crate::config::paths::skills_dir_global(),
                cwd.as_deref(),
            );
            let count = crate::skills::get_all_skills().len();
            tracing::info!(count, "Skills reloaded via IPC");
            vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillsLoaded { count },
            }]
        }
        SkillCommand::QueryStatus => {
            let skills = build_skill_info_list();
            vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillList { skills },
            }]
        }
    }
}

// ===========================================================================
// Status snapshot builders
// ===========================================================================

/// Build a list of LSP server info from the default server configurations.
pub fn build_lsp_server_info_list() -> Vec<LspServerInfo> {
    crate::lsp_service::default_server_configs()
        .into_iter()
        .map(|cfg| LspServerInfo {
            language_id: cfg.language_id,
            state: "not_started".to_string(),
            extensions: cfg.extensions.iter().map(|e| format!(".{}", e)).collect(),
            open_files_count: 0,
            error: None,
        })
        .collect()
}

/// Build a list of MCP server status info from discovered configurations.
pub fn build_mcp_server_info_list() -> Vec<McpServerStatusInfo> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let configs = crate::mcp::discovery::discover_mcp_servers(&cwd).unwrap_or_default();

    configs
        .into_iter()
        .map(|cfg| McpServerStatusInfo {
            name: cfg.name,
            state: "pending".to_string(),
            transport: cfg.transport,
            tools_count: 0,
            resources_count: 0,
            server_info: None,
            instructions: None,
            error: None,
        })
        .collect()
}

/// Build a list of editable config entries (issue #44) from scope-aware
/// discovery. Unlike [`build_mcp_server_info_list`] this preserves one row
/// per scope so the same logical server can appear in multiple scopes (e.g.
/// "same name in user + project").
pub fn build_mcp_server_config_entries(cwd: &std::path::Path) -> Vec<McpServerConfigEntry> {
    let scoped = crate::mcp::discovery::discover_mcp_servers_scoped(cwd).unwrap_or_default();
    scoped
        .into_iter()
        .map(|s| McpServerConfigEntry {
            name: s.config.name,
            scope: scope_from_discovery(&s.scope),
            transport: s.config.transport,
            command: s.config.command,
            args: s.config.args,
            url: s.config.url,
            headers: s.config.headers,
            env: s.config.env,
            browser_mcp: s.config.browser_mcp,
        })
        .collect()
}

/// Map the discovery-layer `DiscoveryScope` onto the IPC `ConfigScope`.
fn scope_from_discovery(scope: &DiscoveryScope) -> ConfigScope {
    match scope {
        DiscoveryScope::User => ConfigScope::User,
        DiscoveryScope::Project => ConfigScope::Project,
        DiscoveryScope::Plugin(id) => ConfigScope::Plugin { id: id.clone() },
        DiscoveryScope::Ide(id) => ConfigScope::Ide { id: id.clone() },
    }
}

// ---------------------------------------------------------------------------
// MCP config persistence (issue #44)
// ---------------------------------------------------------------------------

/// Resolve the `settings.json` path for an editable scope.
///
/// Returns `Err` when the scope is read-only (plugin / IDE).
///
/// We intentionally **don't** walk ancestors for `Project`: the scoped
/// discovery layer reads exactly `{cwd}/.cc-rust/settings.json`, so any
/// write must land in the same place or the round-trip breaks. Callers
/// that really want the ancestor-walking behaviour should stabilize their
/// project root before invoking this.
fn settings_path_for_scope(
    cwd: &std::path::Path,
    scope: &ConfigScope,
) -> Result<PathBuf, String> {
    match scope {
        ConfigScope::User => Ok(cc_config::settings::user_settings_path()),
        ConfigScope::Project => Ok(cwd.join(".cc-rust").join("settings.json")),
        ConfigScope::Plugin { id } => Err(format!(
            "scope `plugin:{}` is read-only — edit the plugin manifest instead",
            id
        )),
        ConfigScope::Ide { id } => Err(format!(
            "scope `ide:{}` is read-only — edit the IDE bridge config instead",
            id
        )),
    }
}

/// Read the raw settings file (returning defaults if missing).
fn read_settings_value(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(&content).map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

/// Write a raw settings value with parent-dir creation + atomic rename.
fn write_settings_value(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {}", parent.display(), e))?;
    }
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to serialize settings: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty)
        .map_err(|e| format!("failed to write {}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("failed to rename {} -> {}: {}", tmp.display(), path.display(), e))?;
    Ok(())
}

/// Upsert a server config into the settings file backing `entry.scope`.
///
/// Returns the entry that was persisted on success, or `(server_name, message)`
/// on failure (so the caller can emit `McpEvent::ConfigError`).
fn upsert_mcp_entry(
    cwd: &std::path::Path,
    entry: McpServerConfigEntry,
) -> Result<McpServerConfigEntry, (String, String)> {
    if !entry.scope.is_editable() {
        return Err((
            entry.name.clone(),
            format!(
                "scope `{}` is read-only — cannot upsert MCP server config",
                entry.scope.label()
            ),
        ));
    }

    let path = settings_path_for_scope(cwd, &entry.scope)
        .map_err(|e| (entry.name.clone(), e))?;

    let mut settings = read_settings_value(&path).map_err(|e| (entry.name.clone(), e))?;
    if !settings.is_object() {
        return Err((
            entry.name.clone(),
            format!("{} is not a JSON object", path.display()),
        ));
    }

    let obj = settings.as_object_mut().unwrap();
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !servers.is_object() {
        return Err((
            entry.name.clone(),
            format!("{} has a non-object `mcpServers` field", path.display()),
        ));
    }
    let servers_obj = servers.as_object_mut().unwrap();
    servers_obj.insert(entry.name.clone(), entry_to_settings_value(&entry));

    write_settings_value(&path, &settings).map_err(|e| (entry.name.clone(), e))?;
    Ok(entry)
}

/// Remove a server config entry from the settings file backing `scope`.
fn remove_mcp_entry(
    cwd: &std::path::Path,
    server_name: &str,
    scope: &ConfigScope,
) -> Result<(), String> {
    if !scope.is_editable() {
        return Err(format!(
            "scope `{}` is read-only — cannot remove MCP server config",
            scope.label()
        ));
    }
    let path = settings_path_for_scope(cwd, scope)?;
    if !path.exists() {
        return Err(format!(
            "no settings file at {} — nothing to remove",
            path.display()
        ));
    }
    let mut settings = read_settings_value(&path)?;
    let Some(obj) = settings.as_object_mut() else {
        return Err(format!("{} is not a JSON object", path.display()));
    };
    let Some(servers) = obj.get_mut("mcpServers") else {
        return Err(format!(
            "{} has no `mcpServers` section",
            path.display()
        ));
    };
    let Some(servers_obj) = servers.as_object_mut() else {
        return Err(format!(
            "{} has a non-object `mcpServers` field",
            path.display()
        ));
    };
    if servers_obj.remove(server_name).is_none() {
        return Err(format!(
            "{} has no MCP server named `{}`",
            path.display(),
            server_name
        ));
    }
    write_settings_value(&path, &settings)?;
    Ok(())
}

/// Serialize an entry for the on-disk `mcpServers[name]` value.
///
/// The settings file uses the legacy `McpServerConfig` shape (transport under
/// `type`, `command`/`args`/`url`/…). Consumers using different shapes can
/// still round-trip thanks to `McpServerConfig`'s permissive deserializer.
fn entry_to_settings_value(entry: &McpServerConfigEntry) -> serde_json::Value {
    let cfg = crate::mcp::McpServerConfig {
        name: entry.name.clone(),
        transport: entry.transport.clone(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        url: entry.url.clone(),
        headers: entry.headers.clone(),
        env: entry.env.clone(),
        browser_mcp: entry.browser_mcp,
    };
    // `McpServerConfig` serializes `name` as a field; the settings file uses
    // the map key for naming, so drop it from the inner object.
    let mut value = serde_json::to_value(&cfg).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        obj.remove("name");
    }
    value
}

/// Build a list of plugin info from the in-memory plugin registry.
pub fn build_plugin_info_list() -> Vec<PluginInfo> {
    use crate::plugins::PluginStatus;

    crate::plugins::get_all_plugins()
        .into_iter()
        .map(|p| {
            let (status_str, error) = match &p.status {
                PluginStatus::NotInstalled => ("not_installed".to_string(), None),
                PluginStatus::Installed => ("installed".to_string(), None),
                PluginStatus::Disabled => ("disabled".to_string(), None),
                PluginStatus::Error(e) => ("error".to_string(), Some(e.clone())),
            };
            PluginInfo {
                id: p.id,
                name: p.name,
                version: p.version,
                status: status_str,
                contributed_tools: p.tools,
                contributed_skills: p.skills,
                contributed_mcp_servers: p.mcp_servers,
                error,
            }
        })
        .collect()
}

/// Build a list of skill info from the global skill registry.
pub fn build_skill_info_list() -> Vec<SkillInfo> {
    use crate::skills::SkillSource;

    crate::skills::get_all_skills()
        .into_iter()
        .map(|s| {
            let source_str = match &s.source {
                SkillSource::Bundled => "bundled".to_string(),
                SkillSource::User => "user".to_string(),
                SkillSource::Project => "project".to_string(),
                SkillSource::Plugin(_) => "plugin".to_string(),
                SkillSource::Mcp(_) => "mcp".to_string(),
            };
            SkillInfo {
                name: s.display_name().to_string(),
                source: source_str,
                description: s.frontmatter.description.clone(),
                user_invocable: s.is_user_invocable(),
                model_invocable: s.is_model_invocable(),
            }
        })
        .collect()
}

/// Build the list of detected IDE integrations (issue #41).
///
/// Thin wrapper around [`crate::ide::detect_ides`] that exists primarily
/// so the IPC layer has a stable entry point we can hook from other
/// places (e.g. the future `/ide` TUI view) without reaching into the
/// `ide` module.
pub fn build_ide_info_list() -> Vec<IdeInfo> {
    crate::ide::detect_ides()
}

/// Build a complete subsystem status snapshot combining all subsystems.
pub fn build_subsystem_status_snapshot() -> SubsystemStatusSnapshot {
    SubsystemStatusSnapshot {
        lsp: build_lsp_server_info_list(),
        mcp: build_mcp_server_info_list(),
        plugins: build_plugin_info_list(),
        skills: build_skill_info_list(),
        ides: build_ide_info_list(),
        timestamp: chrono::Utc::now().timestamp(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_lsp_server_info_list_returns_configured_servers() {
        let infos = build_lsp_server_info_list();
        assert!(infos.len() >= 6);
        let rust = infos.iter().find(|i| i.language_id == "rust");
        assert!(rust.is_some());
        assert_eq!(rust.unwrap().state, "not_started");
    }

    #[test]
    fn build_lsp_server_info_list_has_dotted_extensions() {
        let infos = build_lsp_server_info_list();
        let rust = infos.iter().find(|i| i.language_id == "rust").unwrap();
        assert!(
            rust.extensions.contains(&".rs".to_string()),
            "extensions should be dot-prefixed"
        );
    }

    #[test]
    fn build_mcp_server_info_list_defaults_to_pending() {
        let infos = build_mcp_server_info_list();
        for info in &infos {
            assert_eq!(info.state, "pending");
        }
    }

    #[test]
    fn build_plugin_info_list_maps_status() {
        use crate::plugins;

        plugins::clear_plugins();
        plugins::register_plugin(plugins::PluginEntry {
            id: "test-plugin-handlers".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "For testing".to_string(),
            source: plugins::PluginSource::Local {
                path: "/tmp/test".to_string(),
            },
            status: plugins::PluginStatus::Installed,
            marketplace: None,
            cache_path: None,
            tools: vec!["tool_a".to_string()],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });
        plugins::register_plugin(plugins::PluginEntry {
            id: "err-plugin-handlers".to_string(),
            name: "Error Plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Broken".to_string(),
            source: plugins::PluginSource::Local {
                path: "/tmp/err".to_string(),
            },
            status: plugins::PluginStatus::Error("load failed".to_string()),
            marketplace: None,
            cache_path: None,
            tools: vec![],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });

        let infos = build_plugin_info_list();
        let test_p = infos.iter().find(|p| p.id == "test-plugin-handlers");
        assert!(test_p.is_some());
        assert_eq!(test_p.unwrap().status, "installed");
        assert!(test_p.unwrap().error.is_none());

        let err_p = infos.iter().find(|p| p.id == "err-plugin-handlers");
        assert!(err_p.is_some());
        assert_eq!(err_p.unwrap().status, "error");
        assert_eq!(err_p.unwrap().error.as_deref(), Some("load failed"));

        plugins::clear_plugins();
    }

    #[test]
    fn build_skill_info_list_returns_skills() {
        use crate::skills;

        skills::clear_skills();
        skills::register_skill(crate::skills::SkillDefinition {
            name: "test-skill".to_string(),
            source: crate::skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: crate::skills::SkillFrontmatter {
                description: "A test".to_string(),
                user_invocable: true,
                ..Default::default()
            },
            prompt_body: String::new(),
        });
        let infos = build_skill_info_list();
        let test = infos.iter().find(|s| s.name == "test-skill");
        assert!(test.is_some());
        assert_eq!(test.unwrap().source, "bundled");
        assert!(test.unwrap().user_invocable);
        skills::clear_skills();
    }

    #[test]
    fn build_skill_info_list_maps_sources() {
        use crate::skills;

        skills::clear_skills();

        let sources = vec![
            ("bundled-sk", skills::SkillSource::Bundled, "bundled"),
            ("user-sk", skills::SkillSource::User, "user"),
            ("project-sk", skills::SkillSource::Project, "project"),
            (
                "plugin-sk",
                skills::SkillSource::Plugin("p".to_string()),
                "plugin",
            ),
            ("mcp-sk", skills::SkillSource::Mcp("m".to_string()), "mcp"),
        ];

        for (name, source, _) in &sources {
            skills::register_skill(skills::SkillDefinition {
                name: name.to_string(),
                source: source.clone(),
                base_dir: None,
                frontmatter: skills::SkillFrontmatter {
                    description: "test".to_string(),
                    user_invocable: true,
                    ..Default::default()
                },
                prompt_body: String::new(),
            });
        }

        let infos = build_skill_info_list();
        for (name, _, expected_source) in &sources {
            let info = infos.iter().find(|s| s.name == *name);
            assert!(info.is_some(), "skill {} should be present", name);
            assert_eq!(info.unwrap().source, *expected_source);
        }

        skills::clear_skills();
    }

    #[test]
    fn build_subsystem_status_snapshot_has_timestamp() {
        let snapshot = build_subsystem_status_snapshot();
        assert!(snapshot.timestamp > 0, "timestamp should be positive");
        assert!(snapshot.lsp.len() >= 6);
    }

    // ── Handler return-value tests ────────────────────────────────────

    #[test]
    fn handle_lsp_query_status_returns_server_list() {
        use super::super::subsystem_events::LspCommand;
        let msgs = handle_lsp_command(LspCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::LspEvent { .. }));
    }

    #[test]
    fn handle_lsp_start_returns_info() {
        use super::super::subsystem_events::LspCommand;
        let msgs = handle_lsp_command(LspCommand::StartServer {
            language_id: "rust".into(),
        });
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SystemInfo { .. }));
    }

    #[test]
    fn apply_recommendation_decision_no_is_noop() {
        // `no` should not emit a system-info message or mutate persistence.
        let (_settings, info) = apply_recommendation_decision("foo-ls", "no");
        assert!(info.is_none(), "'no' should not produce an info message");
    }

    #[test]
    fn apply_recommendation_decision_unknown_is_warned_but_silent() {
        // Unknown decisions shouldn't blow up or leak into the UI.
        let (_settings, info) = apply_recommendation_decision("foo-ls", "banana");
        assert!(info.is_none());
    }

    #[test]
    fn apply_recommendation_decision_yes_emits_placeholder_info() {
        // Until the install path is wired up, `yes` returns the placeholder
        // info text. When the real install lands this test should be
        // replaced — the point here is that `yes` *does* surface a message
        // so the user knows something happened.
        let (_settings, info) = apply_recommendation_decision("rust-analyzer", "yes");
        assert!(info.is_some());
        let text = info.unwrap();
        assert!(text.contains("rust-analyzer"));
    }

    #[test]
    fn handle_mcp_query_status_returns_server_list() {
        use super::super::subsystem_events::McpCommand;
        let msgs = handle_mcp_command(McpCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::McpEvent { .. }));
    }

    // ── MCP config editing tests (issue #44) ─────────────────────────
    //
    // These tests drive the pure `upsert_mcp_entry` / `remove_mcp_entry`
    // helpers against a temp `CC_RUST_HOME` / cwd to avoid touching the
    // user's real settings file. They also verify `ConfigError` is emitted
    // for read-only scopes.

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

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_persists_to_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "ctx7".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "ctx7".to_string()]),
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };

        let written = upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");
        assert_eq!(written.name, "ctx7");

        let settings_path = home.path().join("settings.json");
        assert!(settings_path.exists(), "user settings.json should be created");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(on_disk["mcpServers"]["ctx7"]["command"], "npx");
        assert_eq!(on_disk["mcpServers"]["ctx7"]["args"][0], "-y");
    }

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_persists_to_project_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "proj-srv".to_string(),
            scope: ConfigScope::Project,
            transport: "stdio".to_string(),
            command: Some("./local.sh".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };

        upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");

        let path = cwd.path().join(".cc-rust").join("settings.json");
        assert!(path.exists(), "project settings.json should be created");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["mcpServers"]["proj-srv"]["command"], "./local.sh");
    }

    #[test]
    #[serial_test::serial]
    fn upsert_mcp_entry_rejects_plugin_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "plugin-srv".to_string(),
            scope: ConfigScope::Plugin {
                id: "com.example.p".to_string(),
            },
            transport: "stdio".to_string(),
            command: Some("x".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };

        let err = upsert_mcp_entry(cwd.path(), entry).expect_err("plugin scope rejected");
        assert_eq!(err.0, "plugin-srv");
        assert!(err.1.contains("read-only"));
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_round_trips_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "ctx7".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };
        upsert_mcp_entry(cwd.path(), entry).expect("upsert ok");

        remove_mcp_entry(cwd.path(), "ctx7", &ConfigScope::User).expect("remove ok");

        let settings_path = home.path().join("settings.json");
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let servers = on_disk
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers object");
        assert!(!servers.contains_key("ctx7"), "entry should be gone");
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_rejects_plugin_scope() {
        let cwd = tempfile::tempdir().expect("tempdir");
        let err = remove_mcp_entry(
            cwd.path(),
            "p",
            &ConfigScope::Plugin {
                id: "com.example.p".to_string(),
            },
        )
        .expect_err("plugin scope rejected");
        assert!(err.contains("read-only"));
    }

    #[test]
    #[serial_test::serial]
    fn remove_mcp_entry_errors_on_missing_file() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let err = remove_mcp_entry(cwd.path(), "nope", &ConfigScope::User)
            .expect_err("missing file should error");
        assert!(err.contains("nothing to remove"));
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_upsert_config_emits_config_changed() {
        use super::super::subsystem_events::McpCommand;
        let home = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        let entry = McpServerConfigEntry {
            name: "h-test".to_string(),
            scope: ConfigScope::User,
            transport: "stdio".to_string(),
            command: Some("t".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };
        let msgs = handle_mcp_command(McpCommand::UpsertConfig { entry });
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BackendMessage::McpEvent {
                event:
                    McpEvent::ConfigChanged {
                        server_name,
                        entry: Some(e),
                    },
            } => {
                assert_eq!(server_name, "h-test");
                assert_eq!(e.name, "h-test");
                assert_eq!(e.scope, ConfigScope::User);
            }
            other => panic!("unexpected response: {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_upsert_config_on_read_only_emits_config_error() {
        use super::super::subsystem_events::McpCommand;

        let entry = McpServerConfigEntry {
            name: "plugin-srv".to_string(),
            scope: ConfigScope::Plugin {
                id: "com.example".to_string(),
            },
            transport: "stdio".to_string(),
            command: Some("x".to_string()),
            args: None,
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        };
        let msgs = handle_mcp_command(McpCommand::UpsertConfig { entry });
        match &msgs[0] {
            BackendMessage::McpEvent {
                event: McpEvent::ConfigError { server_name, .. },
            } => assert_eq!(server_name, "plugin-srv"),
            other => panic!("expected ConfigError, got {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn handle_mcp_query_config_returns_config_list() {
        use super::super::subsystem_events::McpCommand;
        let msgs = handle_mcp_command(McpCommand::QueryConfig);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BackendMessage::McpEvent {
                event: McpEvent::ConfigList { .. },
            } => {}
            other => panic!("expected ConfigList, got {:?}", other),
        }
    }

    #[test]
    #[serial_test::serial]
    fn build_mcp_server_config_entries_tags_user_scope() {
        let home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        std::fs::write(
            home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "u-srv": {"transport": "stdio", "command": "u-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let entries = build_mcp_server_config_entries(cwd.path());
        let entry = entries
            .iter()
            .find(|e| e.name == "u-srv")
            .expect("user entry present");
        assert_eq!(entry.scope, ConfigScope::User);
        assert_eq!(entry.command.as_deref(), Some("u-cmd"));
    }

    #[test]
    fn handle_plugin_query_status_returns_plugin_list() {
        use super::super::subsystem_events::PluginCommand;
        let msgs = handle_plugin_command(PluginCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::PluginEvent { .. }));
    }

    #[test]
    fn handle_skill_query_status_returns_skill_list() {
        use super::super::subsystem_events::SkillCommand;
        let msgs = handle_skill_command(SkillCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SkillEvent { .. }));
    }
}
