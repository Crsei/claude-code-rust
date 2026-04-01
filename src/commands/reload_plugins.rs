//! `/reload-plugins` command -- reload plugins from disk.
//!
//! Re-scans the plugins directory, reloads installed plugins, and
//! reports the count of plugins and skills found.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct ReloadPluginsHandler;

#[async_trait]
impl CommandHandler for ReloadPluginsHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        reload()
    }
}

/// Perform the plugin reload.
fn reload() -> Result<CommandResult> {
    // Clear the in-memory registry and re-discover from disk.
    crate::plugins::clear_plugins();

    // Re-scan the cache directory for plugin manifests.
    let discovered = crate::plugins::loader::discover_cached_plugins();
    let plugin_count = discovered.len();

    // Convert discovered manifests to entries and register them.
    let mut skill_count: usize = 0;
    let mut tool_count: usize = 0;

    for (manifest, cache_path) in &discovered {
        // Derive marketplace name from cache path (parent of plugin dir).
        let marketplace = cache_path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());

        let entry =
            crate::plugins::loader::manifest_to_entry(manifest, cache_path, marketplace);

        skill_count += entry.skills.len();
        tool_count += entry.tools.len();

        crate::plugins::register_plugin(entry);
    }

    // Also reload the installed_plugins.json entries that may not be
    // in the cache (e.g., externally installed plugins).
    let installed = crate::plugins::loader::load_installed_plugins();
    let installed_count = installed.len();
    for entry in installed {
        // Only register if not already registered from cache discovery.
        if crate::plugins::find_plugin(&entry.id).is_none() {
            skill_count += entry.skills.len();
            tool_count += entry.tools.len();
            crate::plugins::register_plugin(entry);
        }
    }

    let total_plugins = crate::plugins::get_all_plugins().len();

    Ok(CommandResult::Output(format!(
        "Plugins reloaded.\n\n\
         Discovered from cache: {} plugin(s)\n\
         From installed list:   {} plugin(s)\n\
         Total registered:      {} plugin(s)\n\
         Tools contributed:     {}\n\
         Skills contributed:    {}",
        plugin_count, installed_count, total_plugins, tool_count, skill_count
    )))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_reload_plugins_output() {
        let handler = ReloadPluginsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Plugins reloaded"));
                assert!(text.contains("plugin(s)"));
                assert!(text.contains("Tools contributed"));
                assert!(text.contains("Skills contributed"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_reload_plugins_ignores_args() {
        let handler = ReloadPluginsHandler;
        let mut ctx = test_ctx();
        // Arguments are ignored — always does a full reload.
        let result = handler.execute("--force", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Plugins reloaded"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[test]
    fn test_reload_clears_registry() {
        // Register a fake plugin, then verify reload clears it.
        crate::plugins::clear_plugins();
        crate::plugins::register_plugin(crate::plugins::PluginEntry {
            id: "fake-for-reload-test".to_string(),
            name: "Fake".to_string(),
            version: "0.0.1".to_string(),
            description: "test".to_string(),
            source: crate::plugins::PluginSource::Local {
                path: "/tmp".to_string(),
            },
            status: crate::plugins::PluginStatus::Installed,
            marketplace: None,
            cache_path: None,
            tools: vec![],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        });
        assert!(crate::plugins::find_plugin("fake-for-reload-test").is_some());

        // Calling reload clears and re-discovers.
        let result = reload().unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Plugins reloaded")),
            _ => panic!("Expected Output"),
        }

        // The fake plugin should be gone (it's not on disk).
        assert!(crate::plugins::find_plugin("fake-for-reload-test").is_none());
    }
}
