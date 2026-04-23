//! `/reload-plugins` command -- hot-refresh the plugin registry (issue #49).
//!
//! Wraps the `plugins::reload_plugins()` primitive provided by the foundation
//! commit. The primitive clears the in-memory registry and repopulates from
//! `~/.cc-rust/plugins/installed_plugins.json`, then emits
//! `PluginEvent::Reloaded` on the subsystem event bus.
//!
//! # Session re-wiring
//!
//! Plugin contributions (tools, skills, MCP servers) are discovered via
//! `discover_plugin_tools()`, `discover_plugin_skills()`, and
//! `discover_plugin_mcp_servers()`. Each walks the registry on every call, so
//! they pick up changes to the registry automatically -- no cache to
//! invalidate at the discovery layer.
//!
//! The long-lived session tool list (`QueryEngineState::tools`, seeded at
//! startup in `main.rs` via `registry::get_all_tools()`) is a separate
//! snapshot held by `QueryEngine`. Command handlers do not have direct
//! access to the engine (see `CommandContext` in `commands::mod`), so this
//! command cannot refresh that snapshot inline. For the current REPL this
//! matches the behaviour of `/plugin enable|disable`, which also leaves the
//! engine tool list untouched; the registry changes become visible to
//! sub-agents spawned after the reload (they call `get_all_tools()` fresh)
//! and to MCP/skill discovery, which the engine re-queries each turn.
//! Re-seeding the engine tool list on reload is tracked as a follow-up and
//! intentionally out of scope for this command.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::plugins;

/// Handler for `/reload-plugins`.
pub struct ReloadPluginsHandler;

#[async_trait]
impl CommandHandler for ReloadPluginsHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let report = plugins::reload_plugins();
        Ok(CommandResult::Output(format_report(&report)))
    }
}

/// Format a [`plugins::ReloadReport`] as the user-facing output block.
///
/// Shape:
///
/// ```text
/// Reloaded {count} plugin(s) in {duration_ms}ms.
/// ```
///
/// When `report.errors` is non-empty, each `(id, error)` pair is appended on
/// its own line prefixed with `"  - "`, followed by a trailing summary line
/// of the form `"1 plugin(s) failed to load."` so the error count is obvious
/// even if the per-plugin list is long.
fn format_report(report: &plugins::ReloadReport) -> String {
    let mut out = format!(
        "Reloaded {} plugin(s) in {}ms.",
        report.count, report.duration_ms
    );

    if !report.errors.is_empty() {
        for (id, err) in &report.errors {
            out.push('\n');
            out.push_str(&format!("  - {}: {}", id, err));
        }
        out.push('\n');
        out.push_str(&format!("{} plugin(s) failed to load.", report.error_count));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::plugins::{
        clear_plugins, register_plugin, PluginEntry, PluginSource, PluginStatus, ReloadReport,
    };
    use crate::types::app_state::AppState;
    use parking_lot::Mutex;
    use std::path::PathBuf;
    use std::sync::LazyLock;

    /// Serialize tests that touch the global plugin registry -- otherwise
    /// `clear_plugins` / `register_plugin` / `reload_plugins` in one test
    /// races with parallel tests elsewhere in the crate.
    static REGISTRY_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
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

    // -----------------------------------------------------------------------
    // format_report: pure formatting -- no global state required.
    // -----------------------------------------------------------------------

    #[test]
    fn format_report_success_no_errors() {
        let report = ReloadReport {
            count: 3,
            error_count: 0,
            errors: vec![],
            duration_ms: 42,
        };
        let out = format_report(&report);
        assert_eq!(out, "Reloaded 3 plugin(s) in 42ms.");
    }

    #[test]
    fn format_report_zero_plugins_is_not_an_error() {
        let report = ReloadReport {
            count: 0,
            error_count: 0,
            errors: vec![],
            duration_ms: 7,
        };
        let out = format_report(&report);
        assert_eq!(out, "Reloaded 0 plugin(s) in 7ms.");
        assert!(!out.contains("failed"));
    }

    #[test]
    fn format_report_includes_error_lines() {
        let report = ReloadReport {
            count: 2,
            error_count: 1,
            errors: vec![("broken@local".into(), "manifest parse failed".into())],
            duration_ms: 11,
        };
        let out = format_report(&report);

        assert!(out.starts_with("Reloaded 2 plugin(s) in 11ms."));
        assert!(out.contains("  - broken@local: manifest parse failed"));
        assert!(out.contains("1 plugin(s) failed to load."));
    }

    #[test]
    fn format_report_includes_all_errors() {
        let report = ReloadReport {
            count: 5,
            error_count: 2,
            errors: vec![
                ("a@local".into(), "err a".into()),
                ("b@local".into(), "err b".into()),
            ],
            duration_ms: 3,
        };
        let out = format_report(&report);
        assert!(out.contains("  - a@local: err a"));
        assert!(out.contains("  - b@local: err b"));
        assert!(out.contains("2 plugin(s) failed to load."));
    }

    // -----------------------------------------------------------------------
    // Handler execute: smoke test against the live registry.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn handler_smoke_test_clean_registry() {
        let _guard = REGISTRY_GUARD.lock();

        clear_plugins();
        let handler = ReloadPluginsHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.starts_with("Reloaded "), "got: {}", text);
                assert!(text.contains("plugin(s) in "), "got: {}", text);
                assert!(text.ends_with("ms."), "got: {}", text);
            }
            _ => panic!("Expected CommandResult::Output"),
        }
    }

    #[tokio::test]
    async fn handler_surfaces_plugin_errors() {
        let _guard = REGISTRY_GUARD.lock();

        // Seed an Error-status plugin so reload_plugins() captures it in
        // errors. reload_plugins() first clears the registry then calls
        // init_plugins() which reads from disk; to surface the error
        // deterministically we register the plugin *after* reload_plugins()
        // would have run, by directly exercising format_report on a
        // hand-built report here. The e2e path that catches error-state
        // plugins is covered by refresh::tests.
        clear_plugins();
        register_plugin(make_plugin(
            "broken-test",
            PluginStatus::Error("boom".into()),
        ));

        // Exercise format_report as the handler would: gather errors and
        // assemble the report shape we expect reload_plugins() to produce.
        let mut errors = Vec::new();
        for plugin in plugins::get_all_plugins() {
            if let PluginStatus::Error(msg) = plugin.status {
                errors.push((plugin.id.clone(), msg));
            }
        }
        let simulated = ReloadReport {
            count: plugins::get_all_plugins().len(),
            error_count: errors.len(),
            errors,
            duration_ms: 0,
        };
        let out = format_report(&simulated);

        assert!(out.contains("  - broken-test: boom"));
        assert!(out.contains("1 plugin(s) failed to load."));

        clear_plugins();
    }

    #[tokio::test]
    async fn handler_single_plugin_smoke() {
        let _guard = REGISTRY_GUARD.lock();

        // Seed one installed plugin, then run the handler against the live
        // primitive. reload_plugins() wipes + repopulates from disk, so the
        // in-memory plugin vanishes unless it is also persisted. The success
        // case we assert is the output format, not the plugin count.
        clear_plugins();
        register_plugin(make_plugin("solo-test", PluginStatus::Installed));

        let handler = ReloadPluginsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();

        match result {
            CommandResult::Output(text) => {
                // The report line must be present regardless of whether the
                // seeded plugin survived the reload cycle.
                assert!(text.starts_with("Reloaded "), "got: {}", text);
                assert!(text.contains("plugin(s) in "), "got: {}", text);
            }
            _ => panic!("Expected CommandResult::Output"),
        }

        clear_plugins();
    }
}
