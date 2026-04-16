//! Per-subsystem event and command enums, plus a broadcast-based event bus.
//!
//! **Event enums** (Backend -> Frontend): `Serialize + Debug + Clone`, tagged by `"kind"`.
//! **Command enums** (Frontend -> Backend): `Deserialize + Debug`, tagged by `"kind"`.
//!
//! These types are consumed by:
//! - `protocol.rs` — wrapped inside `BackendMessage` / `FrontendMessage` variants
//! - `headless.rs` — event dispatch loop
//! - `subsystem_handlers.rs` — command handling
//! - LSP / MCP / Plugin / Skill modules — to emit events

#![allow(dead_code)] // Types are pre-defined for upcoming IPC extension tasks

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::subsystem_types::*;

// ===========================================================================
// Event enums (Backend → Frontend)
// ===========================================================================

/// Events emitted by the LSP subsystem.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LspEvent {
    /// An LSP server changed its lifecycle state.
    ServerStateChanged {
        language_id: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Fresh diagnostics were published for a document.
    DiagnosticsPublished {
        uri: String,
        diagnostics: Vec<LspDiagnostic>,
    },
    /// Full list of LSP servers (response to `QueryStatus`).
    ServerList { servers: Vec<LspServerInfo> },
}

/// Events emitted by the MCP subsystem.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpEvent {
    /// An MCP server changed its connection state.
    ServerStateChanged {
        server_name: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Tools were (re-)discovered on an MCP server.
    ToolsDiscovered {
        server_name: String,
        tools: Vec<McpToolInfo>,
    },
    /// Resources were (re-)discovered on an MCP server.
    ResourcesDiscovered {
        server_name: String,
        resources: Vec<McpResourceInfo>,
    },
    /// A channel notification arrived from an MCP server.
    ChannelNotification {
        server_name: String,
        content: String,
        meta: serde_json::Value,
    },
    /// Full list of MCP servers (response to `QueryStatus`).
    ServerList { servers: Vec<McpServerStatusInfo> },
}

/// Events emitted by the plugin subsystem.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginEvent {
    /// A plugin's status changed.
    StatusChanged {
        plugin_id: String,
        name: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Full list of plugins (response to `QueryStatus`).
    PluginList { plugins: Vec<PluginInfo> },
}

/// Events emitted by the skill subsystem.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillEvent {
    /// Skills were loaded / reloaded.
    SkillsLoaded { count: usize },
    /// Full list of skills (response to `QueryStatus`).
    SkillList { skills: Vec<SkillInfo> },
}

// ===========================================================================
// Command enums (Frontend → Backend)
// ===========================================================================

/// Commands the frontend can send to the LSP subsystem.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LspCommand {
    StartServer { language_id: String },
    StopServer { language_id: String },
    RestartServer { language_id: String },
    QueryStatus,
}

/// Commands the frontend can send to the MCP subsystem.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpCommand {
    ConnectServer { server_name: String },
    DisconnectServer { server_name: String },
    ReconnectServer { server_name: String },
    QueryStatus,
}

/// Commands the frontend can send to the plugin subsystem.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginCommand {
    Enable { plugin_id: String },
    Disable { plugin_id: String },
    QueryStatus,
}

/// Commands the frontend can send to the skill subsystem.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillCommand {
    Reload,
    QueryStatus,
}

// ===========================================================================
// Unified event wrapper
// ===========================================================================

/// Wrapper that tags every subsystem event with its origin subsystem.
#[derive(Debug, Clone)]
pub enum SubsystemEvent {
    Lsp(LspEvent),
    Mcp(McpEvent),
    Plugin(PluginEvent),
    Skill(SkillEvent),
}

// ===========================================================================
// Event bus
// ===========================================================================

/// Broadcast-based event bus for subsystem events.
///
/// Any number of producers can call `sender().send(event)` and any number of
/// consumers can `subscribe()` to receive a copy of every event.
pub struct SubsystemEventBus {
    tx: broadcast::Sender<SubsystemEvent>,
}

impl SubsystemEventBus {
    /// Create a new event bus with a default channel capacity of 256.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { tx }
    }

    /// Clone the sender half for use in subsystem modules.
    pub fn sender(&self) -> broadcast::Sender<SubsystemEvent> {
        self.tx.clone()
    }

    /// Subscribe to the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<SubsystemEvent> {
        self.tx.subscribe()
    }
}

impl Default for SubsystemEventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Event serialization
    // -----------------------------------------------------------------------

    #[test]
    fn lsp_event_server_state_changed_serializes() {
        let event = LspEvent::ServerStateChanged {
            language_id: "rust".to_string(),
            state: "running".to_string(),
            error: None,
        };
        let value = serde_json::to_value(&event).expect("serialize LspEvent");
        assert_eq!(value["kind"], "server_state_changed");
        assert_eq!(value["language_id"], "rust");
        assert_eq!(value["state"], "running");
        assert!(value.get("error").is_none(), "None error should be omitted");
    }

    #[test]
    fn lsp_event_diagnostics_published_serializes() {
        let event = LspEvent::DiagnosticsPublished {
            uri: "file:///src/main.rs".to_string(),
            diagnostics: vec![LspDiagnostic {
                range: DiagnosticRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 1,
                    end_character: 10,
                },
                severity: "error".to_string(),
                message: "syntax error".to_string(),
                source: Some("rustc".to_string()),
                code: None,
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "diagnostics_published");
        assert_eq!(value["diagnostics"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn lsp_event_server_list_serializes() {
        let event = LspEvent::ServerList {
            servers: vec![LspServerInfo {
                language_id: "typescript".to_string(),
                state: "running".to_string(),
                extensions: vec![".ts".to_string()],
                open_files_count: 2,
                error: None,
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "server_list");
        assert_eq!(value["servers"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn mcp_event_server_state_changed_serializes() {
        let event = McpEvent::ServerStateChanged {
            server_name: "context7".to_string(),
            state: "connected".to_string(),
            error: None,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "server_state_changed");
        assert_eq!(value["server_name"], "context7");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn mcp_event_tools_discovered_serializes() {
        let event = McpEvent::ToolsDiscovered {
            server_name: "test-server".to_string(),
            tools: vec![McpToolInfo {
                name: "search".to_string(),
                description: Some("Search docs".to_string()),
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "tools_discovered");
        assert_eq!(value["tools"][0]["name"], "search");
    }

    #[test]
    fn mcp_event_resources_discovered_serializes() {
        let event = McpEvent::ResourcesDiscovered {
            server_name: "res-server".to_string(),
            resources: vec![McpResourceInfo {
                uri: "file:///data.json".to_string(),
                name: Some("data".to_string()),
                mime_type: Some("application/json".to_string()),
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "resources_discovered");
        assert_eq!(value["resources"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn mcp_event_channel_notification_serializes() {
        let event = McpEvent::ChannelNotification {
            server_name: "chan-server".to_string(),
            content: "hello".to_string(),
            meta: serde_json::json!({"priority": "high"}),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "channel_notification");
        assert_eq!(value["content"], "hello");
        assert_eq!(value["meta"]["priority"], "high");
    }

    #[test]
    fn mcp_event_server_list_serializes() {
        let event = McpEvent::ServerList {
            servers: vec![McpServerStatusInfo {
                name: "s1".to_string(),
                state: "connected".to_string(),
                transport: "stdio".to_string(),
                tools_count: 3,
                resources_count: 1,
                server_info: None,
                instructions: None,
                error: None,
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "server_list");
    }

    #[test]
    fn plugin_event_status_changed_serializes() {
        let event = PluginEvent::StatusChanged {
            plugin_id: "com.example.foo".to_string(),
            name: "Foo".to_string(),
            status: "active".to_string(),
            error: None,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "status_changed");
        assert_eq!(value["plugin_id"], "com.example.foo");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn plugin_event_status_changed_with_error_serializes() {
        let event = PluginEvent::StatusChanged {
            plugin_id: "com.example.broken".to_string(),
            name: "Broken".to_string(),
            status: "error".to_string(),
            error: Some("init failed".to_string()),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["error"], "init failed");
    }

    #[test]
    fn plugin_event_plugin_list_serializes() {
        let event = PluginEvent::PluginList {
            plugins: vec![PluginInfo {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                status: "active".to_string(),
                contributed_tools: vec![],
                contributed_skills: vec![],
                contributed_mcp_servers: vec![],
                error: None,
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "plugin_list");
    }

    #[test]
    fn skill_event_skills_loaded_serializes() {
        let event = SkillEvent::SkillsLoaded { count: 5 };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "skills_loaded");
        assert_eq!(value["count"], 5);
    }

    #[test]
    fn skill_event_skill_list_serializes() {
        let event = SkillEvent::SkillList {
            skills: vec![SkillInfo {
                name: "simplify".to_string(),
                source: "builtin".to_string(),
                description: "Review code".to_string(),
                user_invocable: true,
                model_invocable: false,
            }],
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "skill_list");
        assert_eq!(value["skills"][0]["name"], "simplify");
    }

    // -----------------------------------------------------------------------
    // Command deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn lsp_command_start_server_deserializes() {
        let json = r#"{"kind":"start_server","language_id":"rust"}"#;
        let cmd: LspCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            LspCommand::StartServer { language_id } => assert_eq!(language_id, "rust"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn lsp_command_stop_server_deserializes() {
        let json = r#"{"kind":"stop_server","language_id":"typescript"}"#;
        let cmd: LspCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            LspCommand::StopServer { language_id } => assert_eq!(language_id, "typescript"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn lsp_command_restart_server_deserializes() {
        let json = r#"{"kind":"restart_server","language_id":"python"}"#;
        let cmd: LspCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            LspCommand::RestartServer { language_id } => assert_eq!(language_id, "python"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn lsp_command_query_status_deserializes() {
        let json = r#"{"kind":"query_status"}"#;
        let cmd: LspCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, LspCommand::QueryStatus));
    }

    #[test]
    fn mcp_command_connect_server_deserializes() {
        let json = r#"{"kind":"connect_server","server_name":"ctx7"}"#;
        let cmd: McpCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            McpCommand::ConnectServer { server_name } => assert_eq!(server_name, "ctx7"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn mcp_command_disconnect_server_deserializes() {
        let json = r#"{"kind":"disconnect_server","server_name":"broken"}"#;
        let cmd: McpCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, McpCommand::DisconnectServer { .. }));
    }

    #[test]
    fn mcp_command_reconnect_server_deserializes() {
        let json = r#"{"kind":"reconnect_server","server_name":"s1"}"#;
        let cmd: McpCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, McpCommand::ReconnectServer { .. }));
    }

    #[test]
    fn mcp_command_query_status_deserializes() {
        let json = r#"{"kind":"query_status"}"#;
        let cmd: McpCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, McpCommand::QueryStatus));
    }

    #[test]
    fn plugin_command_enable_deserializes() {
        let json = r#"{"kind":"enable","plugin_id":"com.example.foo"}"#;
        let cmd: PluginCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            PluginCommand::Enable { plugin_id } => assert_eq!(plugin_id, "com.example.foo"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn plugin_command_disable_deserializes() {
        let json = r#"{"kind":"disable","plugin_id":"com.example.bar"}"#;
        let cmd: PluginCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, PluginCommand::Disable { .. }));
    }

    #[test]
    fn plugin_command_query_status_deserializes() {
        let json = r#"{"kind":"query_status"}"#;
        let cmd: PluginCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, PluginCommand::QueryStatus));
    }

    #[test]
    fn skill_command_reload_deserializes() {
        let json = r#"{"kind":"reload"}"#;
        let cmd: SkillCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, SkillCommand::Reload));
    }

    #[test]
    fn skill_command_query_status_deserializes() {
        let json = r#"{"kind":"query_status"}"#;
        let cmd: SkillCommand = serde_json::from_str(json).expect("deserialize");
        assert!(matches!(cmd, SkillCommand::QueryStatus));
    }

    // -----------------------------------------------------------------------
    // Event bus
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn event_bus_send_receive() {
        let bus = SubsystemEventBus::new();
        let mut rx = bus.subscribe();
        let tx = bus.sender();

        let event = SubsystemEvent::Skill(SkillEvent::SkillsLoaded { count: 3 });
        tx.send(event).expect("send should succeed");

        let received = rx.recv().await.expect("recv should succeed");
        match received {
            SubsystemEvent::Skill(SkillEvent::SkillsLoaded { count }) => {
                assert_eq!(count, 3);
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn event_bus_multiple_subscribers() {
        let bus = SubsystemEventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let tx = bus.sender();

        let event = SubsystemEvent::Lsp(LspEvent::ServerStateChanged {
            language_id: "rust".to_string(),
            state: "running".to_string(),
            error: None,
        });
        tx.send(event).expect("send");

        let e1 = rx1.recv().await.expect("rx1 recv");
        let e2 = rx2.recv().await.expect("rx2 recv");

        assert!(matches!(
            e1,
            SubsystemEvent::Lsp(LspEvent::ServerStateChanged { .. })
        ));
        assert!(matches!(
            e2,
            SubsystemEvent::Lsp(LspEvent::ServerStateChanged { .. })
        ));
    }

    #[test]
    fn event_bus_no_subscriber_no_panic() {
        let bus = SubsystemEventBus::new();
        let tx = bus.sender();

        // Sending with no subscribers should return an error but not panic.
        let result = tx.send(SubsystemEvent::Skill(SkillEvent::SkillsLoaded { count: 0 }));
        assert!(
            result.is_err(),
            "send with no subscribers should return Err"
        );
    }

    #[test]
    fn subsystem_event_wraps_all_variants() {
        // Ensure we can construct every SubsystemEvent variant (compile-time check).
        let _lsp = SubsystemEvent::Lsp(LspEvent::ServerList { servers: vec![] });
        let _mcp = SubsystemEvent::Mcp(McpEvent::ServerList { servers: vec![] });
        let _plugin = SubsystemEvent::Plugin(PluginEvent::PluginList { plugins: vec![] });
        let _skill = SubsystemEvent::Skill(SkillEvent::SkillList { skills: vec![] });
    }

    #[test]
    fn event_bus_default_trait() {
        // SubsystemEventBus implements Default.
        let bus = SubsystemEventBus::default();
        let _rx = bus.subscribe();
    }
}
