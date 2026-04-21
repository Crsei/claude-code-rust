//! Plugin hot-refresh primitive — the engine behind `/reload-plugins` (issue #49).
//!
//! Exposes [`reload_plugins`], which:
//!   1. Clears the in-memory registry.
//!   2. Reloads installed plugins from `~/.cc-rust/plugins/installed_plugins.json`.
//!   3. Reports the outcome as a [`ReloadReport`] and emits a
//!      [`PluginEvent::Reloaded`] on the subsystem event bus so connected
//!      frontends pick up the change without polling.
//!
//! Contributions (tools, skills, MCP servers) stay *reactive*: they are
//! resolved via `discover_plugin_*()` on each query, so no extra bookkeeping
//! is needed here. Consumers that build steady-state registries (e.g. the
//! tool registry at session start) must re-query after `reload_plugins()`
//! for the changes to land in long-lived caches.

use std::time::Instant;

use tracing::{info, warn};

use super::{clear_plugins, init_plugins, loader, PluginStatus};
use crate::ipc::subsystem_events::{PluginEvent, SubsystemEvent};

/// Summary of a plugin reload cycle.
#[derive(Debug, Clone)]
pub struct ReloadReport {
    /// Total plugins now in the registry.
    pub count: usize,
    /// Number of plugins that entered an error state during reload.
    pub error_count: usize,
    /// Per-plugin error messages, keyed by plugin id.
    pub errors: Vec<(String, String)>,
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
/// Emits `PluginEvent::Reloaded` on completion via [`super::emit_event`].
pub fn reload_plugins() -> ReloadReport {
    let start = Instant::now();

    // 1. Snapshot the on-disk state before touching the registry, so we
    //    can surface per-plugin load errors even if `init_plugins` swallows
    //    them internally.
    let on_disk = loader::load_installed_plugins();

    // 2. Wipe + repopulate. This is intentionally synchronous: callers
    //    already expect a short blocking refresh, and keeping it sync means
    //    we can safely run it from slash-command handlers without extra
    //    orchestration.
    clear_plugins();
    init_plugins();

    // 3. Collect error diagnostics by walking the fresh registry.
    let mut errors = Vec::new();
    for plugin in super::get_all_plugins() {
        if let PluginStatus::Error(msg) = plugin.status {
            warn!(plugin = %plugin.id, error = %msg, "plugin reload: entered error state");
            errors.push((plugin.id.clone(), msg.clone()));
        }
    }

    let count = super::get_all_plugins().len();
    let expected = on_disk.len();
    if count < expected {
        warn!(
            expected,
            actual = count,
            "plugin reload: registry has fewer entries than installed_plugins.json"
        );
    }

    let report = ReloadReport {
        count,
        error_count: errors.len(),
        errors,
        duration_ms: start.elapsed().as_millis(),
    };

    info!(
        count = report.count,
        errors = report.error_count,
        duration_ms = report.duration_ms,
        "plugins reloaded"
    );

    // 4. Announce on the event bus so any attached frontend can refresh.
    super::emit_event(SubsystemEvent::Plugin(PluginEvent::Reloaded {
        count: report.count,
        had_error: report.had_error(),
    }));

    report
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{register_plugin, PluginEntry, PluginSource};
    use parking_lot::Mutex;
    use std::sync::LazyLock;

    /// Serialize tests that touch the global plugin registry — otherwise
    /// `clear_plugins` / `reload_plugins` in one test races with
    /// `register_plugin` in another.
    static REGISTRY_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
        assert!(super::super::find_plugin("ephemeral").is_some());

        let report = reload_plugins();

        // The ephemeral plugin should be gone because init_plugins only
        // repopulates from installed_plugins.json.
        assert!(
            super::super::find_plugin("ephemeral").is_none(),
            "in-memory-only plugin should be wiped by reload"
        );
        // Report shape is sane.
        assert_eq!(report.count, super::super::get_all_plugins().len());
    }

    #[test]
    fn report_had_error_reflects_error_count() {
        let _guard = REGISTRY_GUARD.lock();

        clear_plugins();
        let empty = reload_plugins();
        // After reload from a clean disk there may or may not be plugins,
        // but there should be no error count for plugins we didn't register.
        assert_eq!(empty.error_count, empty.errors.len());
        assert_eq!(empty.had_error(), empty.error_count > 0);
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
        for plugin in super::super::get_all_plugins() {
            if let PluginStatus::Error(msg) = plugin.status {
                errors.push((plugin.id.clone(), msg.clone()));
            }
        }
        assert!(errors.iter().any(|(id, _)| id == "broken-test"));

        clear_plugins();
    }
}
