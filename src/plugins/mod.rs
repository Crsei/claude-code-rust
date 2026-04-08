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

use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

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
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cc-rust")
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

/// Register a plugin in the in-memory registry.
pub fn register_plugin(plugin: PluginEntry) {
    REGISTRY.lock().insert(plugin.id.clone(), plugin);
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

/// Remove a plugin from the registry.
pub fn unregister_plugin(id: &str) -> Option<PluginEntry> {
    REGISTRY.lock().remove(id)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
}
