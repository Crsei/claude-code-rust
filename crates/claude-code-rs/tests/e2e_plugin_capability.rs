//! Plugin lifecycle capability tests using a local real plugin fixture.

mod capability_lab_support;

use capability_lab_support::CapabilityLab;
use cc_plugins::installation::{install_plugin, InstallScope};
use cc_plugins::marketplace::{
    MarketplacePluginEntry, MarketplaceSource, GLOBAL_MARKETPLACE_INDEX,
};
use cc_plugins::{PluginSource, PluginStatus};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn local_marketplace_plugin_installs_contributes_and_uninstalls_under_cc_rust_home() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_plugins::clear_plugins();
    GLOBAL_MARKETPLACE_INDEX.clear_cache();

    let plugin_dir = lab.write_plugin_fixture();
    GLOBAL_MARKETPLACE_INDEX.register_source(MarketplaceSource {
        name: "capability-local".to_string(),
        source: PluginSource::Local {
            path: plugin_dir.to_string_lossy().to_string(),
        },
        description: "Local capability fixture marketplace".to_string(),
        auto_update: false,
        priority: 0,
    });
    GLOBAL_MARKETPLACE_INDEX.set_marketplace_entries(
        "capability-local",
        vec![MarketplacePluginEntry {
            id: "capability-plugin".to_string(),
            name: "Capability Plugin".to_string(),
            description: "Local plugin fixture".to_string(),
            version: "1.0.0".to_string(),
            author: Some("cc-rust".to_string()),
            source_name: "capability-local".to_string(),
            download_url: None,
            checksum: None,
            tags: vec!["test".to_string()],
            homepage: None,
            license: None,
        }],
    );

    let result = install_plugin(
        plugin_dir.to_str().expect("plugin path utf-8"),
        Some(InstallScope::Project),
        Some(env!("CARGO_PKG_VERSION")),
        None,
        &Default::default(),
        &Default::default(),
    )
    .await
    .expect("install local plugin fixture");

    assert_eq!(result.plugin.id, "capability-plugin@local");
    assert!(cc_plugins::installed_plugins_path().starts_with(&lab.cc_rust_home));
    assert!(cc_plugins::installed_plugins_path().is_file());

    cc_plugins::init_plugins();
    let enabled = cc_plugins::get_enabled_plugins();
    assert!(enabled
        .iter()
        .any(|plugin| plugin.id == "capability-plugin@local"));

    let mcp = cc_plugins::discover_plugin_mcp_servers_scoped();
    assert!(mcp.iter().any(|(plugin_id, config)| {
        plugin_id == "capability-plugin@local" && config.name == "plugin-sequential"
    }));

    let skills = cc_plugins::discover_plugin_skill_definitions();
    assert!(skills.iter().any(|skill| {
        skill.plugin_id == "capability-plugin@local" && skill.name == "plugin-review"
    }));

    cc_plugins::set_plugin_status("capability-plugin@local", PluginStatus::Disabled)
        .expect("disable plugin");
    assert!(cc_plugins::discover_plugin_mcp_servers_scoped().is_empty());
    assert!(cc_plugins::discover_plugin_skill_definitions().is_empty());

    cc_plugins::set_plugin_status("capability-plugin@local", PluginStatus::Installed)
        .expect("enable plugin");
    assert!(!cc_plugins::discover_plugin_mcp_servers_scoped().is_empty());

    let removed = cc_plugins::uninstall_plugin("capability-plugin@local", false)
        .expect("uninstall plugin")
        .expect("plugin removed");
    assert_eq!(removed.id, "capability-plugin@local");
    assert!(cc_plugins::find_plugin("capability-plugin@local").is_none());
    lab.assert_path_isolated();
}

#[tokio::test]
#[serial]
async fn plugin_install_errors_are_actionable_for_missing_source_and_bad_manifest() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_plugins::clear_plugins();

    let missing = install_plugin(
        lab.project_dir.join("missing-plugin").to_str().unwrap(),
        Some(InstallScope::Project),
        Some(env!("CARGO_PKG_VERSION")),
        None,
        &Default::default(),
        &Default::default(),
    )
    .await
    .expect_err("missing local source should fail");
    assert!(missing
        .to_string()
        .contains("Could not resolve plugin source"));

    let bad_plugin = lab
        .project_dir
        .join("fixtures")
        .join("plugins")
        .join("bad-plugin");
    std::fs::create_dir_all(&bad_plugin).expect("create bad plugin dir");
    capability_lab_support::write_file(
        &bad_plugin.join("plugin.json"),
        r#"{"name":"","version":"","description":"bad"}"#,
    );
    let bad = install_plugin(
        bad_plugin.to_str().unwrap(),
        Some(InstallScope::Project),
        Some(env!("CARGO_PKG_VERSION")),
        None,
        &Default::default(),
        &Default::default(),
    )
    .await
    .expect_err("bad manifest should fail");
    assert!(
        bad.to_string().contains("Plugin name is required")
            || bad.to_string().contains("Invalid manifest"),
        "unexpected bad manifest error: {bad}"
    );
}
