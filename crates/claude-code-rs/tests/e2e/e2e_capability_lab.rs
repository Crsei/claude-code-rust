//! Combined MCP + Skill + Plugin capability lab smoke tests.

mod capability_lab_support;

use capability_lab_support::{filesystem_server_config, sequential_server_config, CapabilityLab};
use cc_mcp::discovery::discover_mcp_servers;
use cc_plugins::installation::{install_plugin, InstallScope};
use serial_test::serial;

#[test]
#[serial]
fn capability_lab_fixture_combines_project_mcp_skill_and_plugin_configuration() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    lab.write_project_mcp_settings(serde_json::json!({
        "filesystem": {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", capability_lab_support::FS_SERVER_PACKAGE, lab.project_dir],
            "env": { "npm_config_cache": lab.npm_cache_dir() }
        },
        "sequential-thinking": {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", capability_lab_support::SEQUENTIAL_SERVER_PACKAGE],
            "env": { "npm_config_cache": lab.npm_cache_dir() }
        }
    }));

    let mcp = discover_mcp_servers(&lab.project_dir).expect("discover project MCP");
    assert_eq!(mcp.len(), 2);

    cc_skills::clear_skills();
    cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        Vec::new(),
        cc_skills::SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );
    assert!(cc_skills::get_all_skills()
        .iter()
        .any(|skill| skill.name == "product-brief-writer"));
    cc_skills::clear_skills();

    let plugin_dir = lab.write_plugin_fixture();
    let manifest = cc_plugins::manifest::load_manifest(&plugin_dir).expect("load plugin manifest");
    assert_eq!(manifest.skills[0].name, "plugin-review");
    assert_eq!(manifest.mcp_servers[0].name, "plugin-sequential");
    lab.assert_path_isolated();
}

#[tokio::test]
#[ignore = "connects to pinned community MCP packages and may download npx dependencies"]
#[serial]
async fn capability_lab_connects_filesystem_and_sequential_mcp_together() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = cc_mcp::manager::McpManager::new();
    manager
        .connect_all(vec![
            filesystem_server_config(&lab.project_dir, &lab.npm_cache_dir()),
            sequential_server_config(&lab.npm_cache_dir()),
        ])
        .await
        .expect("connect MCP servers");

    assert!(manager.clients.contains_key("filesystem"));
    assert!(manager.clients.contains_key("sequential-thinking"));
}

#[tokio::test]
#[ignore = "installs local plugin and connects its pinned community MCP contribution"]
#[serial]
async fn capability_lab_plugin_contributed_mcp_can_connect() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_plugins::clear_plugins();
    let plugin_dir = lab.write_plugin_fixture();
    install_plugin(
        plugin_dir.to_str().unwrap(),
        Some(InstallScope::Project),
        Some(env!("CARGO_PKG_VERSION")),
        None,
        &Default::default(),
        &Default::default(),
    )
    .await
    .expect("install plugin");
    cc_plugins::init_plugins();

    let configs = cc_plugins::discover_plugin_mcp_servers()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(configs.len(), 1);
    let mut manager = cc_mcp::manager::McpManager::new();
    manager
        .connect_all(configs)
        .await
        .expect("connect plugin MCP");
    assert!(manager.clients.contains_key("plugin-sequential"));
}
