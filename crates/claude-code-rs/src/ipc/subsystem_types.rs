//! Shared data types for IPC subsystem status extensions.
//!
//! These types are used by:
//! - Subsystem event enums (IPC events)
//! - `BackendMessage` / `FrontendMessage` protocol extensions
//! - The `SystemStatus` tool
//! - Status snapshot builders
//!
//! All types are `Serialize + Deserialize + Debug + Clone` so they can flow
//! freely across the JSONL/SSE boundary between the Rust backend and any
//! frontend process.

#![allow(dead_code)] // Types are pre-defined for upcoming IPC extension tasks

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// LSP types
// ---------------------------------------------------------------------------

/// A single diagnostic (error/warning) reported by an LSP server.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LspDiagnostic {
    /// Source range of the diagnostic.
    pub range: DiagnosticRange,
    /// Severity label, e.g. "error", "warning", "information", "hint".
    pub severity: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// The source that generated this diagnostic (e.g. "rustc", "eslint").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Optional diagnostic code (e.g. "E0308", "no-unused-vars").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Character range within a document, expressed as 1-based line/character offsets.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiagnosticRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Status information for a running (or failed) LSP server instance.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LspServerInfo {
    /// Language identifier this server handles (e.g. "rust", "typescript").
    pub language_id: String,
    /// Current lifecycle state, e.g. "running", "starting", "stopped", "error".
    pub state: String,
    /// File extensions served by this server (e.g. `[".rs", ".toml"]`).
    pub extensions: Vec<String>,
    /// Number of files currently open in this server.
    pub open_files_count: usize,
    /// Error message if the server is in an error state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// MCP types
// ---------------------------------------------------------------------------

/// Brief description of a tool exposed by an MCP server.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpToolInfo {
    /// Tool name as registered with the MCP server.
    pub name: String,
    /// Optional human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Brief description of a resource exposed by an MCP server.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpResourceInfo {
    /// Resource URI.
    pub uri: String,
    /// Optional display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional MIME type (e.g. "application/json").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Abbreviated server identity returned by the MCP `initialize` handshake.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpServerInfoBrief {
    /// Server name as declared by the MCP server.
    pub name: String,
    /// Server version string.
    pub version: String,
}

/// Where a config entry lives in the settings layering.
///
/// The first two variants (`user`, `project`) are editable; the remaining
/// variants are *read-only* sources that cannot be mutated directly — they
/// must be managed through the plugin or IDE subsystem respectively.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigScope {
    /// Global user scope (`~/.cc-rust/settings.json`).
    User,
    /// Current project scope (`.cc-rust/settings.json`).
    Project,
    /// Contributed by a plugin (read-only; edit via `/plugin`).
    Plugin {
        /// Plugin id that contributes the entry.
        id: String,
    },
    /// Dynamically injected by an IDE integration (read-only; edit via `/ide`).
    Ide {
        /// IDE identifier that contributes the entry.
        id: String,
    },
}

impl ConfigScope {
    /// True when this scope can be edited directly via settings files.
    pub fn is_editable(&self) -> bool {
        matches!(self, ConfigScope::User | ConfigScope::Project)
    }
}

/// Editable MCP server entry — full config payload plus its scope.
///
/// Distinct from [`McpServerStatusInfo`], which describes live connection
/// state. This type is used by the `/mcp` editable management UX to
/// round-trip a server definition between frontend and backend without
/// losing fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpServerConfigEntry {
    /// Logical name / unique key in the settings map.
    pub name: String,
    /// Transport family: `"stdio"` | `"sse"` | `"streamable-http"`.
    pub transport: String,
    /// Command for `stdio` transport.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Command arguments for `stdio` transport.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// URL for `sse` / `streamable-http` transports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// HTTP headers for `sse` / `streamable-http` transports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Environment variables for `stdio`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Explicit browser-MCP marker (affects tooling heuristics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_mcp: Option<bool>,
    /// Where this entry lives. Non-editable scopes (plugin/ide) are
    /// returned for display but reject `UpsertConfig` / `RemoveConfig`.
    pub scope: ConfigScope,
}

/// Aggregate status of a single MCP server connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpServerStatusInfo {
    /// Logical name used to identify this server in configuration.
    pub name: String,
    /// Current connection state, e.g. "connected", "connecting", "disconnected", "error".
    pub state: String,
    /// Transport type, e.g. "stdio", "sse", "streamable-http".
    pub transport: String,
    /// Number of tools the server has registered.
    pub tools_count: usize,
    /// Number of resources the server exposes.
    pub resources_count: usize,
    /// Server identity from the `initialize` handshake, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<McpServerInfoBrief>,
    /// Server-provided instructions / system prompt fragment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Error message if the server is in an error state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Plugin types
// ---------------------------------------------------------------------------

/// Status and contribution summary for a loaded plugin.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginInfo {
    /// Unique plugin identifier (e.g. "com.example.my-plugin").
    pub id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Current status: "not_installed"|"installed"|"disabled"|"error".
    pub status: String,
    /// Names of tools contributed by this plugin.
    pub contributed_tools: Vec<String>,
    /// Names of skills contributed by this plugin.
    pub contributed_skills: Vec<String>,
    /// Names of MCP servers contributed by this plugin.
    pub contributed_mcp_servers: Vec<String>,
    /// Error message if the plugin failed to load or is in an error state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// IDE-integration types
// ---------------------------------------------------------------------------

/// Detected IDE and its integration state.
///
/// An IDE is either (a) merely present on the host, (b) running and
/// available to bind to, or (c) the currently selected default whose
/// MCP bridge is being actively managed.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IdeInfo {
    /// Stable identifier such as `"vscode"`, `"cursor"`, `"jetbrains"`.
    pub id: String,
    /// Human-readable name for display in lists.
    pub name: String,
    /// Whether this IDE is installed on the host machine.
    pub installed: bool,
    /// Whether this IDE currently has a running instance we can bind to.
    pub running: bool,
    /// Whether this IDE is the selected default integration.
    pub selected: bool,
    /// Current connection state when bound: `"disconnected"` |
    /// `"connecting"` | `"connected"` | `"error"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_state: Option<String>,
    /// Error message when the IDE MCP bridge is in an error state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Skill types
// ---------------------------------------------------------------------------

/// Metadata for a registered skill (built-in or user-defined).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SkillInfo {
    /// Skill name (used in `/skill` invocations).
    pub name: String,
    /// Where the skill was loaded from: "bundled"|"user"|"project"|"plugin"|"mcp".
    pub source: String,
    /// Human-readable description of what the skill does.
    pub description: String,
    /// Whether the user can invoke this skill via a slash command.
    pub user_invocable: bool,
    /// Whether the model can invoke this skill autonomously.
    pub model_invocable: bool,
}

// ---------------------------------------------------------------------------
// Aggregated snapshot
// ---------------------------------------------------------------------------

/// Point-in-time snapshot of every tracked subsystem's status.
///
/// Built by the status snapshot builder and returned by the `SystemStatus` tool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubsystemStatusSnapshot {
    /// Status of all active LSP servers.
    pub lsp: Vec<LspServerInfo>,
    /// Status of all configured MCP servers.
    pub mcp: Vec<McpServerStatusInfo>,
    /// Status of all loaded plugins.
    pub plugins: Vec<PluginInfo>,
    /// All registered skills.
    pub skills: Vec<SkillInfo>,
    /// Detected IDE integrations and the currently selected default.
    ///
    /// Empty when IDE detection has not run yet. The field is marked
    /// `#[serde(default)]` so older snapshots without this key deserialize
    /// cleanly.
    #[serde(default)]
    pub ides: Vec<IdeInfo>,
    /// Unix timestamp (seconds since epoch) when this snapshot was captured.
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_diagnostic_roundtrip() {
        let diag = LspDiagnostic {
            range: DiagnosticRange {
                start_line: 10,
                start_character: 4,
                end_line: 10,
                end_character: 20,
            },
            severity: "error".to_string(),
            message: "expected `;`".to_string(),
            source: Some("rustc".to_string()),
            code: Some("E0308".to_string()),
        };

        let json = serde_json::to_string(&diag).expect("serialize LspDiagnostic");
        let parsed: LspDiagnostic = serde_json::from_str(&json).expect("deserialize LspDiagnostic");

        assert_eq!(parsed.severity, "error");
        assert_eq!(parsed.range.start_line, 10);
        assert_eq!(parsed.source.as_deref(), Some("rustc"));
        assert_eq!(parsed.code.as_deref(), Some("E0308"));
    }

    #[test]
    fn lsp_server_info_omits_none_error() {
        let info = LspServerInfo {
            language_id: "rust".to_string(),
            state: "running".to_string(),
            extensions: vec![".rs".to_string(), ".toml".to_string()],
            open_files_count: 3,
            error: None,
        };

        let value = serde_json::to_value(&info).expect("serialize LspServerInfo");
        assert!(
            value.get("error").is_none(),
            "None error field should be omitted"
        );
        assert_eq!(value["language_id"], "rust");
        assert_eq!(value["open_files_count"], 3);
    }

    #[test]
    fn lsp_server_info_includes_error_when_present() {
        let info = LspServerInfo {
            language_id: "typescript".to_string(),
            state: "error".to_string(),
            extensions: vec![".ts".to_string(), ".tsx".to_string()],
            open_files_count: 0,
            error: Some("server crashed".to_string()),
        };

        let value = serde_json::to_value(&info).expect("serialize LspServerInfo");
        assert_eq!(value["error"], "server crashed");
    }

    #[test]
    fn config_scope_editable_flag() {
        assert!(ConfigScope::User.is_editable());
        assert!(ConfigScope::Project.is_editable());
        assert!(!ConfigScope::Plugin {
            id: "p".into()
        }
        .is_editable());
        assert!(!ConfigScope::Ide { id: "vscode".into() }.is_editable());
    }

    #[test]
    fn config_scope_roundtrip() {
        let cases = vec![
            ConfigScope::User,
            ConfigScope::Project,
            ConfigScope::Plugin { id: "com.x".into() },
            ConfigScope::Ide { id: "vscode".into() },
        ];
        for scope in cases {
            let json = serde_json::to_string(&scope).expect("serialize scope");
            let back: ConfigScope = serde_json::from_str(&json).expect("deserialize scope");
            assert_eq!(back, scope);
        }
    }

    #[test]
    fn config_scope_serializes_kind_tag() {
        let value = serde_json::to_value(&ConfigScope::User).unwrap();
        assert_eq!(value["kind"], "user");
        let value = serde_json::to_value(&ConfigScope::Plugin { id: "p".into() }).unwrap();
        assert_eq!(value["kind"], "plugin");
        assert_eq!(value["id"], "p");
    }

    #[test]
    fn mcp_server_config_entry_roundtrip() {
        let entry = McpServerConfigEntry {
            name: "context7".into(),
            transport: "stdio".into(),
            command: Some("npx".into()),
            args: Some(vec!["-y".into(), "context7".into()]),
            url: None,
            headers: None,
            env: Some(HashMap::from([("NODE_ENV".into(), "production".into())])),
            browser_mcp: None,
            scope: ConfigScope::User,
        };

        let json = serde_json::to_string(&entry).expect("serialize entry");
        let back: McpServerConfigEntry = serde_json::from_str(&json).expect("deserialize entry");

        assert_eq!(back.name, "context7");
        assert_eq!(back.transport, "stdio");
        assert_eq!(back.command.as_deref(), Some("npx"));
        assert_eq!(back.env.unwrap().get("NODE_ENV").cloned(), Some("production".into()));

        let value = serde_json::to_value(&entry).unwrap();
        assert!(value.get("url").is_none(), "None url should be omitted");
        assert!(value.get("browser_mcp").is_none());
    }

    #[test]
    fn mcp_server_config_entry_http_transport() {
        let entry = McpServerConfigEntry {
            name: "remote".into(),
            transport: "streamable-http".into(),
            command: None,
            args: None,
            url: Some("https://example.com/mcp".into()),
            headers: Some(HashMap::from([("Authorization".into(), "Bearer x".into())])),
            env: None,
            browser_mcp: Some(true),
            scope: ConfigScope::Project,
        };
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["url"], "https://example.com/mcp");
        assert_eq!(value["browser_mcp"], true);
    }

    #[test]
    fn ide_info_roundtrip() {
        let ide = IdeInfo {
            id: "vscode".into(),
            name: "Visual Studio Code".into(),
            installed: true,
            running: true,
            selected: false,
            connection_state: None,
            error: None,
        };
        let json = serde_json::to_string(&ide).unwrap();
        let back: IdeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "vscode");
        assert!(back.installed);

        let value = serde_json::to_value(&ide).unwrap();
        assert!(value.get("connection_state").is_none());
        assert!(value.get("error").is_none());
    }

    #[test]
    fn ide_info_with_connection_state() {
        let ide = IdeInfo {
            id: "cursor".into(),
            name: "Cursor".into(),
            installed: true,
            running: true,
            selected: true,
            connection_state: Some("connected".into()),
            error: None,
        };
        let value = serde_json::to_value(&ide).unwrap();
        assert_eq!(value["connection_state"], "connected");
        assert_eq!(value["selected"], true);
    }

    #[test]
    fn subsystem_snapshot_backward_compatible_missing_ides() {
        // Older payloads without `ides` field must still deserialize.
        let json = r#"{
            "lsp": [],
            "mcp": [],
            "plugins": [],
            "skills": [],
            "timestamp": 123
        }"#;
        let parsed: SubsystemStatusSnapshot = serde_json::from_str(json).expect("deserialize");
        assert!(parsed.ides.is_empty());
        assert_eq!(parsed.timestamp, 123);
    }

    #[test]
    fn mcp_server_status_info_roundtrip() {
        let status = McpServerStatusInfo {
            name: "context7".to_string(),
            state: "connected".to_string(),
            transport: "stdio".to_string(),
            tools_count: 2,
            resources_count: 0,
            server_info: Some(McpServerInfoBrief {
                name: "context7".to_string(),
                version: "1.0.0".to_string(),
            }),
            instructions: Some("Use for docs".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&status).expect("serialize McpServerStatusInfo");
        let parsed: McpServerStatusInfo =
            serde_json::from_str(&json).expect("deserialize McpServerStatusInfo");

        assert_eq!(parsed.name, "context7");
        assert_eq!(parsed.tools_count, 2);
        assert!(parsed.server_info.is_some());
        assert_eq!(parsed.server_info.as_ref().unwrap().version, "1.0.0");

        // None fields should be omitted from JSON
        let value = serde_json::to_value(&status).expect("to_value");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn mcp_server_status_info_minimal() {
        let status = McpServerStatusInfo {
            name: "broken-server".to_string(),
            state: "error".to_string(),
            transport: "sse".to_string(),
            tools_count: 0,
            resources_count: 0,
            server_info: None,
            instructions: None,
            error: Some("connection refused".to_string()),
        };

        let value = serde_json::to_value(&status).expect("serialize");
        assert!(value.get("server_info").is_none());
        assert!(value.get("instructions").is_none());
        assert_eq!(value["error"], "connection refused");
    }

    #[test]
    fn plugin_info_roundtrip() {
        let plugin = PluginInfo {
            id: "com.example.git-helper".to_string(),
            name: "Git Helper".to_string(),
            version: "0.2.1".to_string(),
            status: "installed".to_string(),
            contributed_tools: vec!["git_status".to_string(), "git_diff".to_string()],
            contributed_skills: vec!["commit".to_string()],
            contributed_mcp_servers: vec![],
            error: None,
        };

        let json = serde_json::to_string(&plugin).expect("serialize PluginInfo");
        let parsed: PluginInfo = serde_json::from_str(&json).expect("deserialize PluginInfo");

        assert_eq!(parsed.id, "com.example.git-helper");
        assert_eq!(parsed.contributed_tools.len(), 2);
        assert_eq!(parsed.contributed_skills, vec!["commit"]);
        assert!(parsed.contributed_mcp_servers.is_empty());

        let value = serde_json::to_value(&plugin).expect("to_value");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn skill_info_roundtrip() {
        let skill = SkillInfo {
            name: "simplify".to_string(),
            source: "bundled".to_string(),
            description: "Review changed code for quality".to_string(),
            user_invocable: true,
            model_invocable: false,
        };

        let json = serde_json::to_string(&skill).expect("serialize SkillInfo");
        let parsed: SkillInfo = serde_json::from_str(&json).expect("deserialize SkillInfo");

        assert_eq!(parsed.name, "simplify");
        assert_eq!(parsed.source, "bundled");
        assert!(parsed.user_invocable);
        assert!(!parsed.model_invocable);
    }

    #[test]
    fn subsystem_status_snapshot_roundtrip() {
        let snapshot = SubsystemStatusSnapshot {
            lsp: vec![LspServerInfo {
                language_id: "rust".to_string(),
                state: "running".to_string(),
                extensions: vec![".rs".to_string()],
                open_files_count: 5,
                error: None,
            }],
            mcp: vec![McpServerStatusInfo {
                name: "context7".to_string(),
                state: "connected".to_string(),
                transport: "stdio".to_string(),
                tools_count: 2,
                resources_count: 1,
                server_info: Some(McpServerInfoBrief {
                    name: "context7".to_string(),
                    version: "1.0.0".to_string(),
                }),
                instructions: None,
                error: None,
            }],
            plugins: vec![PluginInfo {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "0.1.0".to_string(),
                status: "installed".to_string(),
                contributed_tools: vec!["tool_a".to_string()],
                contributed_skills: vec![],
                contributed_mcp_servers: vec![],
                error: None,
            }],
            skills: vec![SkillInfo {
                name: "loop".to_string(),
                source: "bundled".to_string(),
                description: "Run on interval".to_string(),
                user_invocable: true,
                model_invocable: true,
            }],
            ides: vec![IdeInfo {
                id: "vscode".to_string(),
                name: "VS Code".to_string(),
                installed: true,
                running: true,
                selected: true,
                connection_state: Some("connected".to_string()),
                error: None,
            }],
            timestamp: 1713168000,
        };

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let parsed: SubsystemStatusSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(parsed.lsp.len(), 1);
        assert_eq!(parsed.mcp.len(), 1);
        assert_eq!(parsed.plugins.len(), 1);
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.ides.len(), 1);
        assert_eq!(parsed.ides[0].id, "vscode");
        assert_eq!(parsed.timestamp, 1713168000);

        // Verify nested None fields are omitted
        let value = serde_json::to_value(&snapshot).expect("to_value");
        let lsp0 = &value["lsp"][0];
        assert!(lsp0.get("error").is_none());
        let mcp0 = &value["mcp"][0];
        assert!(mcp0.get("instructions").is_none());
        assert!(mcp0.get("error").is_none());
        let plugin0 = &value["plugins"][0];
        assert!(plugin0.get("error").is_none());
    }

    #[test]
    fn empty_snapshot_roundtrip() {
        let snapshot = SubsystemStatusSnapshot {
            lsp: vec![],
            mcp: vec![],
            plugins: vec![],
            skills: vec![],
            ides: vec![],
            timestamp: 0,
        };

        let json = serde_json::to_string(&snapshot).expect("serialize empty snapshot");
        let parsed: SubsystemStatusSnapshot =
            serde_json::from_str(&json).expect("deserialize empty snapshot");

        assert!(parsed.lsp.is_empty());
        assert!(parsed.mcp.is_empty());
        assert!(parsed.plugins.is_empty());
        assert!(parsed.skills.is_empty());
        assert!(parsed.ides.is_empty());
        assert_eq!(parsed.timestamp, 0);
    }
}
