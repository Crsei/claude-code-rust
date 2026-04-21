//! MCP server discovery - finds configured servers from settings and plugins.

use super::McpServerConfig;
use anyhow::Result;
use std::path::Path;

/// Discover MCP server configurations from all supported sources.
///
/// Precedence for same server name (higher overrides lower):
/// 1. Plugin-contributed defaults
/// 2. Global config (`{data_root}/settings.json`)
/// 3. Project config (`.cc-rust/settings.json`)
pub fn discover_mcp_servers(cwd: &Path) -> Result<Vec<McpServerConfig>> {
    let mut servers = Vec::new();

    // Lowest precedence: plugin-contributed servers from installed plugins.
    merge_server_configs(&mut servers, crate::plugins::discover_plugin_mcp_servers());

    // Global config: {data_root}/settings.json
    let global_settings = crate::config::paths::data_root().join("settings.json");
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
    use crate::plugins::{self, PluginEntry, PluginSource, PluginStatus};
    use serial_test::serial;
    use std::path::PathBuf;
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
    fn discover_mcp_servers_includes_plugin_contributions() {
        let tmp = std::env::temp_dir().join(format!(
            "cc_rust_plugin_mcp_discovery_{}",
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::create_dir_all(&tmp);

        let plugin_cache = tmp.join("plugin-cache");
        let _ = std::fs::create_dir_all(&plugin_cache);
        let manifest = serde_json::json!({
            "name": "demo-plugin",
            "version": "1.0.0",
            "description": "demo",
            "mcp_servers": [
                {
                    "name": "plugin-mcp",
                    "command": "demo-mcp",
                    "args": ["--stdio"],
                    "env": {"DEMO": "1"}
                }
            ]
        });
        std::fs::write(
            plugin_cache.join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        plugins::clear_plugins();
        plugins::register_plugin(PluginEntry {
            id: "demo-plugin@local".to_string(),
            name: "Demo Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "demo".to_string(),
            source: PluginSource::Local {
                path: plugin_cache.to_string_lossy().to_string(),
            },
            status: PluginStatus::Installed,
            marketplace: Some("local".to_string()),
            cache_path: Some(PathBuf::from(&plugin_cache)),
            tools: vec![],
            skills: vec![],
            mcp_servers: vec!["plugin-mcp".to_string()],
            installed_at: None,
            updated_at: None,
        });

        let servers = discover_mcp_servers(&tmp).unwrap();
        let server = servers.iter().find(|s| s.name == "plugin-mcp");

        assert!(server.is_some());
        assert_eq!(server.unwrap().command.as_deref(), Some("demo-mcp"));

        plugins::clear_plugins();
        let _ = std::fs::remove_dir_all(&tmp);
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
