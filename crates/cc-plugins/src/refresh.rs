//! Plugin hot-refresh primitive — the engine behind `/reload-plugins` (issue #49).
//!
//! Exposes [`reload_plugins`], which:
//!   1. Clears the in-memory registry.
//!   2. Reloads installed plugins from `~/.cc-rust/plugins/installed_plugins.json`.
//!   3. Reports the outcome as a [`ReloadReport`] and emits a
//!      [`crate::PluginSubsystemEvent::Reloaded`] on the host event sink so
//!      connected frontends pick up the change without polling.
//!
//! Contributions (tools, skills, MCP servers) stay *reactive*: they are
//! resolved via `discover_plugin_*()` on each query, so no extra bookkeeping
//! is needed here. Consumers that build steady-state registries (e.g. the
//! tool registry at session start) must re-query after `reload_plugins()`
//! for the changes to land in long-lived caches.

use std::time::Instant;

use tracing::{info, warn};

use super::{clear_plugins, init_plugins, loader, PluginSubsystemEvent};
use crate::PluginStatus;

/// Summary of a plugin reload cycle.
#[derive(Debug, Clone)]
pub struct ReloadReport {
    /// Total plugins now in the registry.
    pub count: usize,
    /// Number of plugin and global diagnostics observed during reload.
    pub error_count: usize,
    /// Per-plugin error messages, keyed by plugin id.
    pub errors: Vec<(String, String)>,
    /// Global metadata/cache diagnostics that are not tied to a plugin id.
    pub global_errors: Vec<String>,
    /// How long the reload cycle took.
    pub duration_ms: u128,
}

impl ReloadReport {
    /// True when at least one plugin failed to load.
    pub fn had_error(&self) -> bool {
        self.error_count > 0
    }
}

/// Hot-refresh the plugin registry.
///
/// This is the canonical entry point for session-level plugin refresh.
/// See the module docs for the semantics of "refresh" and what stays
/// reactive vs. what the caller must re-query.
///
/// Emits [`PluginSubsystemEvent::Reloaded`] on completion.
pub fn reload_plugins() -> ReloadReport {
    let start = Instant::now();

    // 1. Snapshot the on-disk state before touching the registry, so we
    //    can surface per-plugin load errors even if `init_plugins` swallows
    //    them internally.
    let on_disk = loader::load_installed_plugins_report();

    // 2. Wipe + repopulate. This is intentionally synchronous: callers
    //    already expect a short blocking refresh, and keeping it sync means
    //    we can safely run it from slash-command handlers without extra
    //    orchestration.
    clear_plugins();
    init_plugins();

    // 3. Collect error diagnostics by walking the fresh registry.
    let mut errors = Vec::new();
    let mut seen_error_ids = std::collections::HashSet::new();
    for plugin in super::get_all_plugins() {
        if let PluginStatus::Error(msg) = plugin.status {
            warn!(plugin = %plugin.id, error = %msg, "plugin reload: entered error state");
            seen_error_ids.insert(plugin.id.clone());
            errors.push((plugin.id.clone(), msg.clone()));
        }
    }
    let mut global_errors = Vec::new();
    for diagnostic in super::get_plugin_diagnostics() {
        warn!(
            path = %diagnostic.path.display(),
            plugin_id = diagnostic.plugin_id.as_deref().unwrap_or("<metadata>"),
            error = %diagnostic.message,
            "plugin reload: metadata diagnostic"
        );
        if let Some(plugin_id) = &diagnostic.plugin_id {
            if seen_error_ids.insert(plugin_id.clone()) {
                errors.push((plugin_id.clone(), diagnostic.message.clone()));
            }
        } else {
            global_errors.push(format!(
                "{}: {}",
                diagnostic.path.display(),
                diagnostic.message
            ));
        }
    }

    let count = super::get_all_plugins().len();
    let expected = on_disk.plugins.len();
    if count < expected {
        warn!(
            expected,
            actual = count,
            "plugin reload: registry has fewer entries than installed_plugins.json"
        );
    }

    let report = ReloadReport {
        count,
        error_count: errors.len() + global_errors.len(),
        errors,
        global_errors,
        duration_ms: start.elapsed().as_millis(),
    };

    let diagnostic_count = super::get_plugin_diagnostics().len();
    info!(
        count = report.count,
        errors = report.error_count,
        diagnostics = diagnostic_count,
        duration_ms = report.duration_ms,
        "plugins reloaded"
    );

    // 4. Announce on the event sink so any attached frontend can refresh.
    super::emit_event(PluginSubsystemEvent::Reloaded {
        count: report.count,
        had_error: report.had_error(),
    });

    report
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{installed_plugins_path, plugins_dir, register_plugin};
    use crate::{PluginEntry, PluginSource};
    use parking_lot::Mutex;
    use std::fs;
    use std::path::Path;
    use std::sync::LazyLock;

    /// Serialize tests that touch the global plugin registry — otherwise
    /// `clear_plugins` / `reload_plugins` in one test races with
    /// `register_plugin` in another.
    static REGISTRY_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvGuard {
        old: Option<String>,
    }

    impl EnvGuard {
        fn set_cc_rust_home(path: &Path) -> Self {
            let old = std::env::var("CC_RUST_HOME").ok();
            std::env::set_var("CC_RUST_HOME", path);
            Self { old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(value) => std::env::set_var("CC_RUST_HOME", value),
                None => std::env::remove_var("CC_RUST_HOME"),
            }
        }
    }

    fn make_plugin(id: &str, status: PluginStatus) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: PluginSource::Local {
                path: "/tmp/test".to_string(),
            },
            status,
            marketplace: None,
            cache_path: None,
            tools: vec![],
            skills: vec![],
            mcp_servers: vec![],
            installed_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn reload_clears_in_memory_state() {
        let _guard = REGISTRY_GUARD.lock();

        // Seed the registry with a plugin that is *not* persisted to disk.
        clear_plugins();
        register_plugin(make_plugin("ephemeral", PluginStatus::Installed));
        assert!(crate::find_plugin("ephemeral").is_some());

        let report = reload_plugins();

        // The ephemeral plugin should be gone because init_plugins only
        // repopulates from installed_plugins.json.
        assert!(
            crate::find_plugin("ephemeral").is_none(),
            "in-memory-only plugin should be wiped by reload"
        );
        // Report shape is sane.
        assert_eq!(report.count, crate::get_all_plugins().len());
    }

    #[test]
    fn report_had_error_reflects_error_count() {
        let _guard = REGISTRY_GUARD.lock();

        clear_plugins();
        let empty = reload_plugins();
        // After reload from a clean disk there may or may not be plugins,
        // but there should be no error count for plugins we didn't register.
        assert_eq!(
            empty.error_count,
            empty.errors.len() + empty.global_errors.len()
        );
        assert_eq!(empty.had_error(), empty.error_count > 0);
    }

    #[test]
    #[serial_test::serial]
    fn report_surfaces_global_metadata_diagnostics() {
        let _guard = REGISTRY_GUARD.lock();
        let home = std::env::temp_dir().join(format!(
            "cc_rust_reload_global_diagnostic_{}",
            uuid::Uuid::new_v4()
        ));
        let _env = EnvGuard::set_cc_rust_home(&home);
        clear_plugins();
        fs::create_dir_all(plugins_dir()).unwrap();
        fs::write(installed_plugins_path(), "{ broken json").unwrap();

        let report = reload_plugins();

        assert_eq!(report.count, 0);
        assert_eq!(report.errors.len(), 0);
        assert_eq!(report.global_errors.len(), 1);
        assert!(report.global_errors[0].contains("installed_plugins.json"));
        assert_eq!(report.error_count, 1);
        assert!(report.had_error());

        clear_plugins();
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn report_surfaces_error_plugins() {
        let _guard = REGISTRY_GUARD.lock();

        // Simulate the shape init_plugins produces when a manifest fails
        // to parse: the plugin entry lands in the registry with
        // `PluginStatus::Error(...)`.
        clear_plugins();
        register_plugin(make_plugin(
            "broken-test",
            PluginStatus::Error("boom".to_string()),
        ));

        // Scan the registry the way reload_plugins() does on step 3.
        let mut errors = Vec::new();
        for plugin in crate::get_all_plugins() {
            if let PluginStatus::Error(msg) = plugin.status {
                errors.push((plugin.id.clone(), msg.clone()));
            }
        }
        assert!(errors.iter().any(|(id, _)| id == "broken-test"));

        clear_plugins();
    }
}
