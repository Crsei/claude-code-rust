//! cc-plugins — plugin loader (Phase 7 scaffold).
//!
//! Issue #76 (`[workspace-split] Phase 7`): target destination for
//! `crates/claude-code-rs/src/plugins/`. This crate depends on cc-tools for
//! the Tool trait (via cc-types once the trait lands there) and on
//! cc-permissions for decision plumbing.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod manifest;

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
