//! Plugin system — discovery, installation, and management of plugins.
//!
//! Corresponds to TypeScript: src/utils/plugins/ + src/services/plugins/
//!
//! Plugins extend Claude Code with additional tools, skills, commands, and
//! MCP servers. They are installed from marketplaces (git repos or NPM) and
//! cached locally in `~/.cc-rust/plugins/`.
//!
//! Three-layer architecture:
//! 1. **Intent** — settings files declare desired plugins/marketplaces
//! 2. **Materialization** — `~/.cc-rust/plugins/` contains cached files
//! 3. **Active** — loaded into memory and available to the engine

#![allow(unused)]

pub mod loader;
pub mod manifest;
pub mod tools;

use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};
use tracing::warn;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Source from which a plugin can be installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source")]
pub enum PluginSource {
    /// NPM package.
    #[serde(rename = "npm")]
    Npm {
        package: String,
        version: Option<String>,
    },
    /// GitHub repository.
    #[serde(rename = "github")]
    GitHub {
        repo: String,
        ref_spec: Option<String>,
    },
    /// Generic git URL.
    #[serde(rename = "git")]
    Git {
        url: String,
        ref_spec: Option<String>,
    },
    /// Local filesystem path.
    #[serde(rename = "local")]
    Local { path: String },
}

/// Plugin installation status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginStatus {
    /// Not yet installed.
    NotInstalled,
    /// Installed and available.
    Installed,
    /// Installed but disabled by user.
    Disabled,
    /// Installation or load error.
    Error(String),
}

/// A registered plugin in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Plugin identifier (e.g. "my-plugin@official-marketplace").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Plugin description.
    pub description: String,
    /// Installation source.
    pub source: PluginSource,
    /// Current status.
    pub status: PluginStatus,
    /// Marketplace that provides this plugin (if any).
    pub marketplace: Option<String>,
    /// Local cache path where the plugin is materialized.
    pub cache_path: Option<PathBuf>,
    /// Tools contributed by this plugin.
    pub tools: Vec<String>,
    /// Skills contributed by this plugin.
    pub skills: Vec<String>,
    /// MCP servers contributed by this plugin.
    pub mcp_servers: Vec<String>,
    /// Installation timestamp (Unix seconds).
    pub installed_at: Option<i64>,
    /// Last update timestamp.
    pub updated_at: Option<i64>,
}

/// A marketplace that hosts plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    /// Marketplace name (e.g. "official-marketplace").
    pub name: String,
    /// Source for fetching the marketplace.
    pub source: PluginSource,
    /// Local path where marketplace is cached.
    pub install_location: Option<PathBuf>,
    /// Last refresh timestamp.
    pub last_updated: Option<String>,
    /// Whether to auto-update on startup.
    pub auto_update: bool,
}

// ---------------------------------------------------------------------------
// Plugin cache structure
// ---------------------------------------------------------------------------

/// Expected cache directory layout:
///
/// ```text
/// ~/.cc-rust/plugins/
/// ├── cache/
/// │   └── {marketplace}/{plugin}/{version}/
/// │       └── plugin.json          ← manifest
/// ├── marketplaces/
/// │   ├── official-marketplace/
/// │   │   └── marketplace.json     ← plugin index
/// │   └── {other}/
/// ├── known_marketplaces.json      ← marketplace registry
/// └── installed_plugins.json       ← installation metadata
/// ```
pub fn plugins_dir() -> PathBuf {
    crate::config::settings::global_claude_dir()
        .unwrap_or_else(|_| PathBuf::from(".").join(".cc-rust"))
        .join("plugins")
}

pub fn cache_dir() -> PathBuf {
    plugins_dir().join("cache")
}

pub fn marketplaces_dir() -> PathBuf {
    plugins_dir().join("marketplaces")
}

pub fn installed_plugins_path() -> PathBuf {
    plugins_dir().join("installed_plugins.json")
}

pub fn known_marketplaces_path() -> PathBuf {
    plugins_dir().join("known_marketplaces.json")
}

// ---------------------------------------------------------------------------
// Plugin registry (in-memory)
// ---------------------------------------------------------------------------

static REGISTRY: LazyLock<Mutex<HashMap<String, PluginEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Subsystem event emission
// ---------------------------------------------------------------------------

/// Event sender for subsystem events.
static EVENT_TX: LazyLock<
    Mutex<Option<tokio::sync::broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>>>,
> = LazyLock::new(|| Mutex::new(None));

/// Inject the event sender from the headless event loop.
#[allow(dead_code)] // Called by headless event loop wiring (Task 12).
pub fn set_event_sender(
    tx: tokio::sync::broadcast::Sender<crate::ipc::subsystem_events::SubsystemEvent>,
) {
    *EVENT_TX.lock() = Some(tx);
}

/// Emit a subsystem event.
fn emit_event(event: crate::ipc::subsystem_events::SubsystemEvent) {
    if let Some(tx) = EVENT_TX.lock().as_ref() {
        let _ = tx.send(event);
    }
}

/// Register a plugin in the in-memory registry.
pub fn register_plugin(plugin: PluginEntry) {
    let status_str = match &plugin.status {
        PluginStatus::NotInstalled => "not_installed",
        PluginStatus::Installed => "installed",
        PluginStatus::Disabled => "disabled",
        PluginStatus::Error(_) => "error",
    };
    let event = crate::ipc::subsystem_events::SubsystemEvent::Plugin(
        crate::ipc::subsystem_events::PluginEvent::StatusChanged {
            plugin_id: plugin.id.clone(),
            name: plugin.name.clone(),
            status: status_str.to_string(),
            error: match &plugin.status {
                PluginStatus::Error(e) => Some(e.clone()),
                _ => None,
            },
        },
    );
    REGISTRY.lock().insert(plugin.id.clone(), plugin);
    emit_event(event);
}

/// Get all registered plugins.
pub fn get_all_plugins() -> Vec<PluginEntry> {
    REGISTRY.lock().values().cloned().collect()
}

/// Find a plugin by ID.
pub fn find_plugin(id: &str) -> Option<PluginEntry> {
    REGISTRY.lock().get(id).cloned()
}

/// Get only enabled (installed, not disabled) plugins.
pub fn get_enabled_plugins() -> Vec<PluginEntry> {
    get_all_plugins()
        .into_iter()
        .filter(|p| p.status == PluginStatus::Installed)
        .collect()
}

/// Update status for a plugin in the in-memory registry.
///
/// Returns the updated plugin when found.
pub fn set_plugin_status(id: &str, status: PluginStatus) -> Option<PluginEntry> {
    let mut reg = REGISTRY.lock();
    let plugin = reg.get_mut(id)?;
    plugin.status = status.clone();

    let status_str = match status {
        PluginStatus::NotInstalled => "not_installed",
        PluginStatus::Installed => "installed",
        PluginStatus::Disabled => "disabled",
        PluginStatus::Error(_) => "error",
    };
    let error = match status {
        PluginStatus::Error(e) => Some(e),
        _ => None,
    };

    emit_event(crate::ipc::subsystem_events::SubsystemEvent::Plugin(
        crate::ipc::subsystem_events::PluginEvent::StatusChanged {
            plugin_id: plugin.id.clone(),
            name: plugin.name.clone(),
            status: status_str.to_string(),
            error,
        },
    ));

    Some(plugin.clone())
}

/// Remove a plugin from the registry.
pub fn unregister_plugin(id: &str) -> Option<PluginEntry> {
    let result = REGISTRY.lock().remove(id);
    if let Some(ref removed) = result {
        emit_event(crate::ipc::subsystem_events::SubsystemEvent::Plugin(
            crate::ipc::subsystem_events::PluginEvent::StatusChanged {
                plugin_id: removed.id.clone(),
                name: removed.name.clone(),
                status: "not_installed".to_string(),
                error: None,
            },
        ));
    }
    result
}

/// Clear all plugins (for testing or refresh).
pub fn clear_plugins() {
    REGISTRY.lock().clear();
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Initialize the plugin system — loads installed plugins from disk.
pub fn init_plugins() {
    let installed = loader::load_installed_plugins();
    for plugin in installed {
        register_plugin(plugin);
    }
}

/// Discover executable runtime tools contributed by enabled plugins.
pub fn discover_plugin_tools() -> Vec<Arc<dyn crate::types::tool::Tool>> {
    let mut out = Vec::new();

    for plugin in get_enabled_plugins() {
        let Some(cache_path) = plugin.cache_path.clone() else {
            continue;
        };

        let manifest = match manifest::load_manifest(&cache_path) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    plugin = %plugin.id,
                    path = %cache_path.display(),
                    error = %e,
                    "Plugin: failed to load manifest for tool contribution"
                );
                continue;
            }
        };

        for contributed in manifest.tools {
            if contributed.runtime.is_none() {
                continue;
            }

            out.push(Arc::new(tools::PluginToolWrapper::new(
                plugin.id.clone(),
                cache_path.clone(),
                contributed,
            )) as Arc<dyn crate::types::tool::Tool>);
        }
    }

    out
}

/// Discover MCP server configs contributed by enabled plugins.
///
/// These are loaded from each plugin's cached `plugin.json`.
pub fn discover_plugin_mcp_servers() -> Vec<crate::mcp::McpServerConfig> {
    let mut out = Vec::new();

    for plugin in get_enabled_plugins() {
        let Some(cache_path) = plugin.cache_path.clone() else {
            continue;
        };

        let manifest = match manifest::load_manifest(&cache_path) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    plugin = %plugin.id,
                    path = %cache_path.display(),
                    error = %e,
                    "Plugin: failed to load manifest for MCP contribution"
                );
                continue;
            }
        };

        for mcp in manifest.mcp_servers {
            let env = if mcp.env.is_empty() {
                None
            } else {
                Some(mcp.env.clone())
            };
            out.push(crate::mcp::McpServerConfig {
                name: mcp.name,
                transport: "stdio".to_string(),
                command: Some(mcp.command),
                args: Some(mcp.args),
                url: None,
                headers: None,
                env,
            });
        }
    }

    out
}

/// Discover plugin-provided skill definitions from enabled plugins.
pub fn discover_plugin_skills() -> Vec<crate::skills::SkillDefinition> {
    let mut out = Vec::new();

    for plugin in get_enabled_plugins() {
        let Some(cache_path) = plugin.cache_path.clone() else {
            continue;
        };

        let manifest = match manifest::load_manifest(&cache_path) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    plugin = %plugin.id,
                    path = %cache_path.display(),
                    error = %e,
                    "Plugin: failed to load manifest for skill contribution"
                );
                continue;
            }
        };

        for contributed in manifest.skills {
            let skill_path = cache_path.join(&contributed.path);
            let source = crate::skills::SkillSource::Plugin(plugin.id.clone());

            let mut skill =
                match crate::skills::loader::load_skill_from_file_path(&skill_path, source) {
                    Some(s) => s,
                    None => {
                        warn!(
                            plugin = %plugin.id,
                            path = %skill_path.display(),
                            "Plugin: failed to load contributed skill file"
                        );
                        continue;
                    }
                };

            // Manifest contribution name is treated as canonical registry name.
            skill.name = contributed.name;
            if let Some(desc) = contributed.description {
                if !desc.trim().is_empty() {
                    skill.frontmatter.description = desc;
                }
            }

            out.push(skill);
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::{StdioToolRuntime, ToolContribution, ToolRuntime};
    use std::fs;

    fn make_plugin(id: &str) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            source: PluginSource::Local {
                path: "/tmp/test".to_string(),
            },
            status: PluginStatus::Installed,
            marketplace: None,
            cache_path: None,
            tools: vec![],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: Some(1000),
            updated_at: None,
        }
    }

    #[test]
    fn test_register_and_find() {
        clear_plugins();
        let p = make_plugin("test-find");
        register_plugin(p);
        let found = find_plugin("test-find");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test-find");
    }

    #[test]
    fn test_get_enabled_plugins() {
        clear_plugins();
        let mut p1 = make_plugin("enabled-1");
        p1.status = PluginStatus::Installed;
        let mut p2 = make_plugin("disabled-1");
        p2.status = PluginStatus::Disabled;
        register_plugin(p1);
        register_plugin(p2);

        let enabled = get_enabled_plugins();
        assert!(enabled.iter().any(|p| p.id == "enabled-1"));
        assert!(!enabled.iter().any(|p| p.id == "disabled-1"));
    }

    #[test]
    fn test_unregister() {
        clear_plugins();
        register_plugin(make_plugin("to-remove"));
        assert!(find_plugin("to-remove").is_some());
        unregister_plugin("to-remove");
        assert!(find_plugin("to-remove").is_none());
    }

    #[test]
    fn test_plugin_source_variants() {
        let npm = PluginSource::Npm {
            package: "@foo/bar".into(),
            version: Some("1.0.0".into()),
        };
        let github = PluginSource::GitHub {
            repo: "owner/repo".into(),
            ref_spec: Some("main".into()),
        };
        let git = PluginSource::Git {
            url: "https://example.com/repo.git".into(),
            ref_spec: None,
        };
        let local = PluginSource::Local {
            path: "/tmp/plugin".into(),
        };

        // Verify serde roundtrip
        for src in [npm, github, git, local] {
            let json = serde_json::to_string(&src).unwrap();
            let back: PluginSource = serde_json::from_str(&json).unwrap();
            assert_eq!(back, src);
        }
    }

    #[test]
    fn test_plugin_status_serde() {
        let statuses = vec![
            PluginStatus::NotInstalled,
            PluginStatus::Installed,
            PluginStatus::Disabled,
            PluginStatus::Error("oops".into()),
        ];
        for s in statuses {
            let json = serde_json::to_string(&s).unwrap();
            let back: PluginStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn test_paths() {
        let pd = plugins_dir();
        assert!(pd.to_string_lossy().contains(".cc-rust"));
        assert!(cache_dir().to_string_lossy().contains("cache"));
        assert!(installed_plugins_path()
            .to_string_lossy()
            .contains("installed_plugins"));
    }

    #[test]
    fn test_set_plugin_status() {
        clear_plugins();
        register_plugin(make_plugin("status-target"));

        let updated = set_plugin_status("status-target", PluginStatus::Disabled);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, PluginStatus::Disabled);
    }

    #[test]
    fn test_discover_plugin_mcp_servers() {
        clear_plugins();

        let tmp = std::env::temp_dir().join(format!("cc_rust_plugin_mcp_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("plugin.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "demo-plugin",
                "version": "1.0.0",
                "description": "demo",
                "mcp_servers": [
                    {
                        "name": "demo-mcp",
                        "command": "demo-command",
                        "args": ["--stdio"],
                        "env": {"DEMO_ENV": "1"}
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        register_plugin(PluginEntry {
            id: "demo-plugin@local".to_string(),
            name: "Demo Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "demo".to_string(),
            source: PluginSource::Local {
                path: tmp.to_string_lossy().to_string(),
            },
            status: PluginStatus::Installed,
            marketplace: Some("local".to_string()),
            cache_path: Some(tmp.clone()),
            tools: vec![],
            skills: vec![],
            mcp_servers: vec!["demo-mcp".to_string()],
            installed_at: None,
            updated_at: None,
        });

        let servers = discover_plugin_mcp_servers();
        let server = servers.iter().find(|s| s.name == "demo-mcp");
        assert!(server.is_some());
        assert_eq!(server.unwrap().command.as_deref(), Some("demo-command"));

        clear_plugins();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_discover_plugin_skills() {
        clear_plugins();

        let tmp =
            std::env::temp_dir().join(format!("cc_rust_plugin_skill_{}", uuid::Uuid::new_v4()));
        let skill_dir = tmp.join("skills").join("demo");
        fs::create_dir_all(&skill_dir).unwrap();

        fs::write(
            tmp.join("plugin.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "demo-plugin",
                "version": "1.0.0",
                "description": "demo",
                "skills": [
                    {
                        "name": "demo-skill",
                        "path": "skills/demo/SKILL.md",
                        "description": "plugin skill description"
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: original\nauto: true\n---\n\nDo demo work.",
        )
        .unwrap();

        register_plugin(PluginEntry {
            id: "demo-plugin@local".to_string(),
            name: "Demo Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "demo".to_string(),
            source: PluginSource::Local {
                path: tmp.to_string_lossy().to_string(),
            },
            status: PluginStatus::Installed,
            marketplace: Some("local".to_string()),
            cache_path: Some(tmp.clone()),
            tools: vec![],
            skills: vec!["demo-skill".to_string()],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });

        let skills = discover_plugin_skills();
        let skill = skills.iter().find(|s| s.name == "demo-skill");
        assert!(skill.is_some());
        assert_eq!(
            skill.unwrap().frontmatter.description,
            "plugin skill description"
        );

        clear_plugins();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_discover_plugin_tools() {
        clear_plugins();

        let tmp =
            std::env::temp_dir().join(format!("cc_rust_plugin_tool_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("plugin.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "demo-plugin",
                "version": "1.0.0",
                "description": "demo",
                "tools": [
                    {
                        "name": "demo_tool",
                        "description": "demo runtime tool",
                        "read_only": true,
                        "runtime": {
                            "type": "stdio",
                            "command": if cfg!(windows) { "cmd" } else { "sh" },
                            "args": if cfg!(windows) {
                                serde_json::json!(["/d", "/s", "/c", "more"])
                            } else {
                                serde_json::json!(["-c", "cat"])
                            }
                        }
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        register_plugin(PluginEntry {
            id: "demo-plugin@local".to_string(),
            name: "Demo Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "demo".to_string(),
            source: PluginSource::Local {
                path: tmp.to_string_lossy().to_string(),
            },
            status: PluginStatus::Installed,
            marketplace: Some("local".to_string()),
            cache_path: Some(tmp.clone()),
            tools: vec!["demo_tool".to_string()],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });

        let tools = discover_plugin_tools();
        let tool = tools.iter().find(|tool| tool.name() == "demo_tool");
        assert!(tool.is_some(), "runtime tool should be discoverable");

        clear_plugins();
        let _ = fs::remove_dir_all(&tmp);
    }
}
