//! Subsystem command handlers and status snapshot builders.
//!
//! **Command handlers** respond to `FrontendMessage` commands for each subsystem
//! (LSP, MCP, Plugin, Skill).  For lifecycle operations that require deeper
//! integration (MCP connect/disconnect, plugin enable/disable), the handler
//! replies with a `SystemInfo` message advising the user to use the
//! corresponding slash command.
//!
//! **Status snapshot builders** assemble point-in-time status objects from
//! each subsystem's in-memory state.  These are used by `QueryStatus` commands
//! and the `SystemStatus` tool.

#![allow(dead_code)] // Handlers/builders are pre-defined for upcoming IPC wiring tasks

use super::protocol::{send_to_frontend, BackendMessage};
use super::subsystem_events::{LspEvent, McpEvent, PluginEvent, SkillEvent};
use super::subsystem_types::*;

// ===========================================================================
// Command handlers
// ===========================================================================

/// Handle an LSP subsystem command from the frontend.
///
/// Lifecycle operations (start/stop/restart) are deferred to the `/lsp` slash
/// command because `LSP_CLIENTS` is a private static.  `QueryStatus` builds a
/// server list from the default configurations and sends it back.
pub async fn handle_lsp_command(cmd: super::subsystem_events::LspCommand) {
    use super::subsystem_events::LspCommand;

    match cmd {
        LspCommand::StartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP start requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To start the {} server, run: /lsp start {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            });
        }
        LspCommand::StopServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP stop requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To stop the {} server, run: /lsp stop {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            });
        }
        LspCommand::RestartServer { language_id } => {
            tracing::info!(language_id = %language_id, "LSP restart requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /lsp to manage LSP servers. To restart the {} server, run: /lsp restart {}",
                    language_id, language_id
                ),
                level: "info".to_string(),
            });
        }
        LspCommand::QueryStatus => {
            let servers = build_lsp_server_info_list();
            let _ = send_to_frontend(&BackendMessage::LspEvent {
                event: LspEvent::ServerList { servers },
            });
        }
    }
}

/// Handle an MCP subsystem command from the frontend.
///
/// Lifecycle operations are deferred to the `/mcp` slash command.
/// `QueryStatus` builds a server list from discovered configurations.
pub async fn handle_mcp_command(cmd: super::subsystem_events::McpCommand) {
    use super::subsystem_events::McpCommand;

    match cmd {
        McpCommand::ConnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP connect requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To connect {}, run: /mcp connect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            });
        }
        McpCommand::DisconnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP disconnect requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To disconnect {}, run: /mcp disconnect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            });
        }
        McpCommand::ReconnectServer { server_name } => {
            tracing::info!(server_name = %server_name, "MCP reconnect requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /mcp to manage MCP servers. To reconnect {}, run: /mcp reconnect {}",
                    server_name, server_name
                ),
                level: "info".to_string(),
            });
        }
        McpCommand::QueryStatus => {
            let servers = build_mcp_server_info_list();
            let _ = send_to_frontend(&BackendMessage::McpEvent {
                event: McpEvent::ServerList { servers },
            });
        }
    }
}

/// Handle a plugin subsystem command from the frontend.
///
/// Enable/disable are deferred to the `/plugin` slash command.
/// `QueryStatus` returns the full plugin list.
pub async fn handle_plugin_command(cmd: super::subsystem_events::PluginCommand) {
    use super::subsystem_events::PluginCommand;

    match cmd {
        PluginCommand::Enable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin enable requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To enable {}, run: /plugin enable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            });
        }
        PluginCommand::Disable { plugin_id } => {
            tracing::info!(plugin_id = %plugin_id, "Plugin disable requested via IPC");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Use /plugin to manage plugins. To disable {}, run: /plugin disable {}",
                    plugin_id, plugin_id
                ),
                level: "info".to_string(),
            });
        }
        PluginCommand::QueryStatus => {
            let plugins = build_plugin_info_list();
            let _ = send_to_frontend(&BackendMessage::PluginEvent {
                event: PluginEvent::PluginList { plugins },
            });
        }
    }
}

/// Handle a skill subsystem command from the frontend.
///
/// `Reload` clears and re-initialises the skill registry.
/// `QueryStatus` returns the full skill list.
pub async fn handle_skill_command(cmd: super::subsystem_events::SkillCommand) {
    use super::subsystem_events::SkillCommand;

    match cmd {
        SkillCommand::Reload => {
            let cwd = std::env::current_dir().ok();
            crate::skills::clear_skills();
            crate::skills::init_skills(cwd.as_deref());
            let count = crate::skills::get_all_skills().len();
            tracing::info!(count, "Skills reloaded via IPC");
            let _ = send_to_frontend(&BackendMessage::SkillEvent {
                event: SkillEvent::SkillsLoaded { count },
            });
        }
        SkillCommand::QueryStatus => {
            let skills = build_skill_info_list();
            let _ = send_to_frontend(&BackendMessage::SkillEvent {
                event: SkillEvent::SkillList { skills },
            });
        }
    }
}

// ===========================================================================
// Status snapshot builders
// ===========================================================================

/// Build a list of LSP server info from the default server configurations.
///
/// Because `LSP_CLIENTS` uses a tokio `Mutex` and this function is
/// synchronous, all servers are reported as `"not_started"`.  Live state
/// is delivered through `LspEvent::ServerStateChanged` events.
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
///
/// Servers are discovered from settings files.  Without an active connection
/// we report all as `"pending"`.
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

/// Build a complete subsystem status snapshot combining all subsystems.
pub fn build_subsystem_status_snapshot() -> SubsystemStatusSnapshot {
    SubsystemStatusSnapshot {
        lsp: build_lsp_server_info_list(),
        mcp: build_mcp_server_info_list(),
        plugins: build_plugin_info_list(),
        skills: build_skill_info_list(),
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
        assert!(infos.len() >= 6); // rust, typescript, python, go, c, java
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
        // With no settings files present, this should return an empty list or
        // all-pending entries depending on the environment.  Either way, every
        // entry must be in "pending" state.
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
}
