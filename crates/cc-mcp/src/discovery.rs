//! MCP server discovery - finds configured servers from settings and plugins.

use super::McpServerConfig;
use anyhow::Result;
use parking_lot::Mutex;
use std::path::Path;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Scope tagging (issue #44)
// ---------------------------------------------------------------------------

/// Lightweight origin tag returned alongside each discovered server.
///
/// The host crate (`claude-code-rs`) maps this onto its richer
/// `ipc::subsystem_types::ConfigScope`, which can't live here because cc-mcp
/// is a leaf crate. Keep this enum tiny and string-friendly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryScope {
    /// Global user settings (`{data_root}/settings.json`).
    User,
    /// Project-scoped settings (`{cwd}/.cc-rust/settings.json`).
    Project,
    /// Contributed by a plugin (id preserved).
    Plugin(String),
    /// Contributed by an IDE bridge (id preserved).
    #[allow(dead_code)]
    Ide(String),
}

/// Single scoped discovery result — pairs a loaded config with its origin.
#[derive(Debug, Clone)]
pub struct ScopedMcpServer {
    pub scope: DiscoveryScope,
    pub config: McpServerConfig,
}

// ---------------------------------------------------------------------------
// Plugin-contributed server hook
// ---------------------------------------------------------------------------
//
// `discover_mcp_servers` used to call `crate::plugins::discover_plugin_mcp_servers()`
// directly. Once cc-mcp moved into its own crate (issue #72), reaching back
// into the root crate's `plugins` module would have been a cycle. The host
// registers a callback that returns plugin-contributed server configs.
//
// Two hook shapes are supported:
// - `set_plugin_hook` (legacy): just returns configs. Every entry is marked
//   as `DiscoveryScope::Plugin("")` in the scoped stream — the host may
//   override via `set_scoped_plugin_hook` if it wants real plugin ids.
// - `set_scoped_plugin_hook` (issue #44): returns `(plugin_id, config)`
//   pairs so the host can distinguish "which plugin contributed what".

type PluginHook = Box<dyn Fn() -> Vec<McpServerConfig> + Send + Sync>;
type ScopedPluginHook = Box<dyn Fn() -> Vec<(String, McpServerConfig)> + Send + Sync>;

static PLUGIN_HOOK: LazyLock<Mutex<Option<PluginHook>>> = LazyLock::new(|| Mutex::new(None));
static SCOPED_PLUGIN_HOOK: LazyLock<Mutex<Option<ScopedPluginHook>>> =
    LazyLock::new(|| Mutex::new(None));

/// Register a callback the host can use to contribute plugin-sourced MCP
/// server configs into discovery. Replaces any previous hook.
pub fn set_plugin_hook<F>(cb: F)
where
    F: Fn() -> Vec<McpServerConfig> + Send + Sync + 'static,
{
    *PLUGIN_HOOK.lock() = Some(Box::new(cb));
}

/// Register a scope-aware plugin hook that preserves the owning plugin id.
///
/// When both hooks are registered, the scoped hook takes precedence. The
/// plugin-contribution precedence order (plugin < user < project) is
/// preserved; the scoped hook only adds richer origin tagging.
pub fn set_scoped_plugin_hook<F>(cb: F)
where
    F: Fn() -> Vec<(String, McpServerConfig)> + Send + Sync + 'static,
{
    *SCOPED_PLUGIN_HOOK.lock() = Some(Box::new(cb));
}

/// Plugin-contributed servers paired with their owning plugin id (may be
/// empty when only the legacy non-scoped hook is registered).
fn scoped_plugin_servers() -> Vec<(String, McpServerConfig)> {
    if let Some(scoped) = SCOPED_PLUGIN_HOOK.lock().as_ref() {
        return scoped();
    }
    PLUGIN_HOOK
        .lock()
        .as_ref()
        .map(|cb| cb().into_iter().map(|cfg| (String::new(), cfg)).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// IDE-contributed server hook (issue #41)
// ---------------------------------------------------------------------------
//
// Mirrors `set_plugin_hook` for the IDE-as-MCP-source bridge. The host
// (`crate::ide`) registers a callback that, when an IDE is selected,
// returns the bridge `McpServerConfig` to merge into discovery.

type IdeHook = Box<dyn Fn() -> Vec<McpServerConfig> + Send + Sync>;

static IDE_HOOK: LazyLock<Mutex<Option<IdeHook>>> = LazyLock::new(|| Mutex::new(None));

/// Register a callback for IDE-sourced MCP server configs. Replaces any
/// previous hook.
pub fn set_ide_hook<F>(cb: F)
where
    F: Fn() -> Vec<McpServerConfig> + Send + Sync + 'static,
{
    *IDE_HOOK.lock() = Some(Box::new(cb));
}

fn ide_servers() -> Vec<McpServerConfig> {
    IDE_HOOK
        .lock()
        .as_ref()
        .map(|cb| cb())
        .unwrap_or_default()
}

/// Discover MCP server configurations from all supported sources.
///
/// Precedence for same server name (higher overrides lower):
/// 1. Plugin-contributed defaults
/// 2. Global config (`{data_root}/settings.json`)
/// 3. Project config (`.cc-rust/settings.json`)
///
/// Duplicates (same `name` from multiple sources) are merged: the
/// higher-precedence entry wins. Callers that need one row per source
/// should use [`discover_mcp_servers_scoped`] instead.
pub fn discover_mcp_servers(cwd: &Path) -> Result<Vec<McpServerConfig>> {
    let scoped = discover_mcp_servers_scoped(cwd)?;
    let mut merged: Vec<McpServerConfig> = Vec::new();
    for entry in scoped {
        if let Some(existing) = merged.iter_mut().find(|s| s.name == entry.config.name) {
            *existing = entry.config;
        } else {
            merged.push(entry.config);
        }
    }
    Ok(merged)
}

/// Scope-aware discovery (issue #44).
///
/// Returns one entry per *source*, so the same logical server name may
/// appear multiple times — once per scope that defined it. The host can
/// decide whether to merge (legacy behaviour via [`discover_mcp_servers`])
/// or present them as editable per-scope rows in the UI.
///
/// Ordering matches precedence (low → high), so callers that want the
/// "winning" entry for a given name can take the last match.
pub fn discover_mcp_servers_scoped(cwd: &Path) -> Result<Vec<ScopedMcpServer>> {
    let mut out = Vec::new();

    // Lowest precedence: plugin-contributed servers.
    for (plugin_id, config) in scoped_plugin_servers() {
        out.push(ScopedMcpServer {
            scope: DiscoveryScope::Plugin(plugin_id),
            config,
        });
    }

    // IDE-contributed bridge (issue #41). Sits between plugins and user
    // settings so a user-authored `settings.json` entry with the same name
    // can still override it. The IDE id is unknown at this layer, so the
    // host may refine it via a scoped IDE hook in future; for now we
    // produce an empty-id tag so the frontend can attribute it generically.
    for config in ide_servers() {
        out.push(ScopedMcpServer {
            scope: DiscoveryScope::Ide(String::new()),
            config,
        });
    }

    // Global config: {data_root}/settings.json
    let global_settings = cc_config::paths::data_root().join("settings.json");
    if let Ok(configs) = load_mcp_from_settings(&global_settings) {
        for config in configs {
            out.push(ScopedMcpServer {
                scope: DiscoveryScope::User,
                config,
            });
        }
    }

    // Highest precedence: project config .cc-rust/settings.json
    let project_settings = cwd.join(".cc-rust").join("settings.json");
    if let Ok(configs) = load_mcp_from_settings(&project_settings) {
        for config in configs {
            out.push(ScopedMcpServer {
                scope: DiscoveryScope::Project,
                config,
            });
        }
    }

    Ok(out)
}

fn load_mcp_from_settings(path: &Path) -> Result<Vec<McpServerConfig>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)?;
    let settings: serde_json::Value = serde_json::from_str(&content)?;

    let mut configs = Vec::new();
    if let Some(mcp_servers) = settings.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, config) in mcp_servers {
            if let Ok(mut server_config) = serde_json::from_value::<McpServerConfig>(config.clone())
            {
                server_config.name = name.clone();
                configs.push(server_config);
            }
        }
    }

    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

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
    #[serial]
    fn discover_mcp_servers_merges_project_over_user() {
        let home = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());

        std::fs::write(
            home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "same": {"transport": "stdio", "command": "user-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let p_dir = cwd.path().join(".cc-rust");
        std::fs::create_dir_all(&p_dir).unwrap();
        std::fs::write(
            p_dir.join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "same": {"transport": "stdio", "command": "project-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let merged = discover_mcp_servers(cwd.path()).unwrap();
        let same = merged.iter().find(|s| s.name == "same").unwrap();
        assert_eq!(
            same.command.as_deref(),
            Some("project-cmd"),
            "project scope must win"
        );
    }

    #[test]
    #[serial]
    fn scoped_discovery_tags_user_and_project_sources() {
        let cc_rust_home = TempDir::new().expect("cc_rust_home tempdir");
        let cwd = TempDir::new().expect("cwd tempdir");
        let _home = EnvGuard::set(
            "CC_RUST_HOME",
            cc_rust_home.path().to_str().expect("utf8 tempdir"),
        );

        std::fs::write(
            cc_rust_home.path().join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "u-server": {"transport": "stdio", "command": "u-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let project_dir = cwd.path().join(".cc-rust");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(
            project_dir.join("settings.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "p-server": {"transport": "stdio", "command": "p-cmd"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let scoped = discover_mcp_servers_scoped(cwd.path()).expect("scoped discovery");
        let u = scoped
            .iter()
            .find(|s| s.config.name == "u-server")
            .expect("user entry");
        assert_eq!(u.scope, DiscoveryScope::User);
        let p = scoped
            .iter()
            .find(|s| s.config.name == "p-server")
            .expect("project entry");
        assert_eq!(p.scope, DiscoveryScope::Project);
    }

    #[test]
    #[serial]
    fn discover_mcp_servers_reads_global_config_from_cc_rust_home() {
        let cc_rust_home = TempDir::new().expect("cc_rust_home tempdir");
        let cwd = TempDir::new().expect("cwd tempdir");
        let _home = EnvGuard::set(
            "CC_RUST_HOME",
            cc_rust_home.path().to_str().expect("utf8 tempdir"),
        );

        let settings = serde_json::json!({
            "mcpServers": {
                "override-server": {
                    "transport": "stdio",
                    "command": "from-cc-rust-home",
                    "args": ["--flag"]
                }
            }
        });
        std::fs::write(
            cc_rust_home.path().join("settings.json"),
            serde_json::to_string_pretty(&settings).expect("serialize settings"),
        )
        .expect("write settings");

        let servers = discover_mcp_servers(cwd.path()).expect("discover servers");
        let server = servers
            .iter()
            .find(|s| s.name == "override-server")
            .expect("server from CC_RUST_HOME settings");

        assert_eq!(server.command.as_deref(), Some("from-cc-rust-home"));
        assert_eq!(server.args.as_ref(), Some(&vec!["--flag".to_string()]));
    }
}
