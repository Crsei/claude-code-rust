//! MCP server discovery — finds configured servers from settings
use super::McpServerConfig;
use anyhow::Result;
use std::path::Path;

/// Discover MCP server configurations from settings files
pub fn discover_mcp_servers(cwd: &Path) -> Result<Vec<McpServerConfig>> {
    let mut servers = Vec::new();

    // Check global config: ~/.cc-rust/settings.json
    if let Some(home) = dirs::home_dir() {
        let global_settings = home.join(".cc-rust").join("settings.json");
        if let Ok(configs) = load_mcp_from_settings(&global_settings) {
            servers.extend(configs);
        }
    }

    // Check project config: .cc-rust/settings.json
    let project_settings = cwd.join(".cc-rust").join("settings.json");
    if let Ok(configs) = load_mcp_from_settings(&project_settings) {
        servers.extend(configs);
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
