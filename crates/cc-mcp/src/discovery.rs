//! MCP server discovery - finds configured servers from settings and plugins.

use super::McpServerConfig;
use anyhow::Result;
use parking_lot::Mutex;
use std::path::Path;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Plugin-contributed server hook
// ---------------------------------------------------------------------------
//
// `discover_mcp_servers` used to call `crate::plugins::discover_plugin_mcp_servers()`
// directly. Once cc-mcp moved into its own crate (issue #72), reaching back
// into the root crate's `plugins` module would have been a cycle. The host
// registers a callback that returns plugin-contributed server configs.

type PluginHook = Box<dyn Fn() -> Vec<McpServerConfig> + Send + Sync>;

static PLUGIN_HOOK: LazyLock<Mutex<Option<PluginHook>>> = LazyLock::new(|| Mutex::new(None));

/// Register a callback the host can use to contribute plugin-sourced MCP
/// server configs into discovery. Replaces any previous hook.
pub fn set_plugin_hook<F>(cb: F)
where
    F: Fn() -> Vec<McpServerConfig> + Send + Sync + 'static,
{
    *PLUGIN_HOOK.lock() = Some(Box::new(cb));
}

fn plugin_servers() -> Vec<McpServerConfig> {
    PLUGIN_HOOK
        .lock()
        .as_ref()
        .map(|cb| cb())
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
/// 2. IDE-contributed bridge (selected via `/ide select`, issue #41)
/// 3. Global config (`{data_root}/settings.json`)
/// 4. Project config (`.cc-rust/settings.json`)
pub fn discover_mcp_servers(cwd: &Path) -> Result<Vec<McpServerConfig>> {
    let mut servers = Vec::new();

    // Lowest precedence: plugin-contributed servers from installed plugins.
    merge_server_configs(&mut servers, plugin_servers());

    // IDE-contributed bridge (issue #41). Sits between plugins and user
    // settings so a user-authored `settings.json` entry with the same name
    // can still override it.
    merge_server_configs(&mut servers, ide_servers());

    // Global config: {data_root}/settings.json
    let global_settings = cc_config::paths::data_root().join("settings.json");
    if let Ok(configs) = load_mcp_from_settings(&global_settings) {
        merge_server_configs(&mut servers, configs);
    }

    // Highest precedence: project config .cc-rust/settings.json
    let project_settings = cwd.join(".cc-rust").join("settings.json");
    if let Ok(configs) = load_mcp_from_settings(&project_settings) {
        merge_server_configs(&mut servers, configs);
    }

    Ok(servers)
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

fn merge_server_configs(into: &mut Vec<McpServerConfig>, incoming: Vec<McpServerConfig>) {
    for config in incoming {
        if let Some(existing) = into.iter_mut().find(|s| s.name == config.name) {
            *existing = config;
        } else {
            into.push(config);
        }
    }
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
    fn merge_server_configs_overrides_by_name() {
        let mut base = vec![McpServerConfig {
            name: "same".to_string(),
            transport: "stdio".to_string(),
            command: Some("cmd-a".to_string()),
            args: Some(vec!["a".to_string()]),
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        }];
        let incoming = vec![McpServerConfig {
            name: "same".to_string(),
            transport: "stdio".to_string(),
            command: Some("cmd-b".to_string()),
            args: Some(vec!["b".to_string()]),
            url: None,
            headers: None,
            env: None,
            browser_mcp: None,
        }];

        merge_server_configs(&mut base, incoming);

        assert_eq!(base.len(), 1);
        assert_eq!(base[0].command.as_deref(), Some("cmd-b"));
        assert_eq!(base[0].args.as_ref().unwrap(), &vec!["b".to_string()]);
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
