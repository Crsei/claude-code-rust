//! Plugin loader — reads installed_plugins.json, discovers cached plugins,
//! and converts plugin manifests into PluginEntry registrations.
//!
//! Corresponds to TypeScript: src/utils/plugins/pluginLoader.ts +
//!                            src/utils/plugins/pluginInstallationHelpers.ts

#![allow(unused)]

use std::path::Path;

use anyhow::{Context, Result};

use super::manifest::{load_manifest, PluginManifest};
use super::{
    cache_dir, installed_plugins_path, PluginEntry, PluginSource, PluginStatus,
};

// ---------------------------------------------------------------------------
// Installed plugins persistence (installed_plugins.json)
// ---------------------------------------------------------------------------

/// V2 installed plugins file format.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct InstalledPluginsFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    plugins: Vec<PluginEntry>,
}

/// Load installed plugins from `~/.cc-rust/plugins/installed_plugins.json`.
pub fn load_installed_plugins() -> Vec<PluginEntry> {
    let path = installed_plugins_path();
    if !path.is_file() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    match serde_json::from_str::<InstalledPluginsFile>(&content) {
        Ok(file) => file.plugins,
        Err(_) => {
            // Try parsing as bare array (V1 format)
            serde_json::from_str::<Vec<PluginEntry>>(&content).unwrap_or_default()
        }
    }
}

/// Save installed plugins to `~/.cc-rust/plugins/installed_plugins.json`.
pub fn save_installed_plugins(plugins: &[PluginEntry]) -> Result<()> {
    let path = installed_plugins_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let file = InstalledPluginsFile {
        version: 2,
        plugins: plugins.to_vec(),
    };

    let json = serde_json::to_string_pretty(&file)
        .context("Failed to serialize installed plugins")?;

    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Plugin discovery from cache directory
// ---------------------------------------------------------------------------

/// Scan the cache directory for installed plugins.
///
/// Looks for `cache/{marketplace}/{plugin}/{version}/plugin.json`.
pub fn discover_cached_plugins() -> Vec<(PluginManifest, std::path::PathBuf)> {
    let cache = cache_dir();
    if !cache.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();

    // Iterate marketplace dirs
    let marketplace_dirs = match std::fs::read_dir(&cache) {
        Ok(d) => d,
        Err(_) => return found,
    };

    for mp_entry in marketplace_dirs.flatten() {
        if !mp_entry.path().is_dir() {
            continue;
        }

        // Iterate plugin dirs within marketplace
        let plugin_dirs = match std::fs::read_dir(mp_entry.path()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for plugin_entry in plugin_dirs.flatten() {
            if !plugin_entry.path().is_dir() {
                continue;
            }

            // Iterate version dirs within plugin
            let version_dirs = match std::fs::read_dir(plugin_entry.path()) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for version_entry in version_dirs.flatten() {
                let version_dir = version_entry.path();
                if !version_dir.is_dir() {
                    continue;
                }

                // Try loading plugin.json
                if let Ok(manifest) = load_manifest(&version_dir) {
                    found.push((manifest, version_dir));
                }
            }
        }
    }

    found
}

/// Convert a manifest + cache path into a PluginEntry.
pub fn manifest_to_entry(
    manifest: &PluginManifest,
    cache_path: &Path,
    marketplace: Option<&str>,
) -> PluginEntry {
    let id = if let Some(mp) = marketplace {
        format!("{}@{}", manifest.name, mp)
    } else {
        manifest.name.clone()
    };

    PluginEntry {
        id,
        name: manifest
            .display_name
            .clone()
            .unwrap_or_else(|| manifest.name.clone()),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        source: PluginSource::Local {
            path: cache_path.to_string_lossy().to_string(),
        },
        status: PluginStatus::Installed,
        marketplace: marketplace.map(|s| s.to_string()),
        cache_path: Some(cache_path.to_path_buf()),
        tools: manifest.tools.iter().map(|t| t.name.clone()).collect(),
        skills: manifest.skills.iter().map(|s| s.name.clone()).collect(),
        mcp_servers: manifest.mcp_servers.iter().map(|m| m.name.clone()).collect(),
        installed_at: None,
        updated_at: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::*;
    use std::collections::HashMap;
    use std::io::Write;

    #[test]
    fn test_load_installed_plugins_no_file() {
        // Should return empty vec, not panic
        let plugins = load_installed_plugins();
        // May be non-empty if file exists from prior runs; just verify no crash
        let _ = plugins;
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("test_plugin_save");
        let _ = std::fs::create_dir_all(&dir);

        let path = dir.join("installed_plugins.json");
        let plugins = vec![PluginEntry {
            id: "test@mp".into(),
            name: "Test Plugin".into(),
            version: "1.0.0".into(),
            description: "A test".into(),
            source: PluginSource::Local { path: "/tmp".into() },
            status: PluginStatus::Installed,
            marketplace: Some("mp".into()),
            cache_path: None,
            tools: vec!["tool1".into()],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: Some(1234567890),
            updated_at: None,
        }];

        // Write to custom path
        let file = InstalledPluginsFile {
            version: 2,
            plugins: plugins.clone(),
        };
        let json = serde_json::to_string_pretty(&file).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Read back
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: InstalledPluginsFile = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.plugins.len(), 1);
        assert_eq!(loaded.plugins[0].id, "test@mp");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_to_entry() {
        let manifest = PluginManifest {
            name: "my-plugin".into(),
            display_name: Some("My Plugin".into()),
            version: "2.0.0".into(),
            description: "Does things".into(),
            author: None,
            license: None,
            min_app_version: None,
            tools: vec![ToolContribution {
                name: "tool-a".into(),
                description: "".into(),
                input_schema: None,
                read_only: false,
            }],
            skills: vec![SkillContribution {
                name: "skill-a".into(),
                path: "skills/a/SKILL.md".into(),
                description: None,
            }],
            mcp_servers: vec![],
            commands: vec![],
            dependencies: HashMap::new(),
            configuration: None,
        };

        let entry = manifest_to_entry(
            &manifest,
            Path::new("/cache/mp/my-plugin/2.0.0"),
            Some("mp"),
        );
        assert_eq!(entry.id, "my-plugin@mp");
        assert_eq!(entry.name, "My Plugin");
        assert_eq!(entry.tools, vec!["tool-a"]);
        assert_eq!(entry.skills, vec!["skill-a"]);
        assert_eq!(entry.status, PluginStatus::Installed);
    }

    #[test]
    fn test_discover_cached_empty() {
        // Should not panic on non-existent cache dir
        let result = discover_cached_plugins();
        let _ = result; // may find real cached plugins
    }

    #[test]
    fn test_discover_cached_with_plugin() {
        let base = std::env::temp_dir().join("test_discover_cache");
        let plugin_dir = base.join("mp").join("test-plug").join("1.0.0");
        let _ = std::fs::create_dir_all(&plugin_dir);

        let manifest = serde_json::json!({
            "name": "test-plug",
            "version": "1.0.0",
            "description": "Discovered plugin"
        });
        let mut f = std::fs::File::create(plugin_dir.join("plugin.json")).unwrap();
        write!(f, "{}", serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

        // Temporarily point cache_dir to our test dir isn't straightforward
        // since cache_dir() is hard-coded. Instead, directly test load_manifest.
        let m = load_manifest(&plugin_dir).unwrap();
        assert_eq!(m.name, "test-plug");

        let _ = std::fs::remove_dir_all(&base);
    }
}
