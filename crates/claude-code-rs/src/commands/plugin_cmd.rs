//! /plugin command — layered plugin-state management (issue #47).
//!
//! The `/plugin` UI exposes three distinct layers:
//!
//! - **Install** — present in `installed_plugins.json` on disk.
//! - **Enablement** — `PluginStatus::Installed` (enabled) vs `Disabled`.
//! - **Active** — loaded into the current session's in-memory registry.
//!
//! A plugin can be installed-but-disabled (listed on disk, Enable=false), or
//! installed-and-enabled but not active (persisted changes not yet reloaded
//! into the running session).
//!
//! Subcommands:
//! - `/plugin` or `/plugin list`          — all plugins with all three columns
//! - `/plugin installed`                  — only enabled/installed plugins
//! - `/plugin disabled`                   — only disabled plugins
//! - `/plugin errors`                     — only plugins with an error status
//! - `/plugin status`                     — summary + drift diagnostics
//! - `/plugin enable <plugin-id>`         — flip status to Installed
//! - `/plugin disable <plugin-id>`        — flip status to Disabled
//! - `/plugin uninstall <plugin-id>`      — drop from installed_plugins.json
//! - `/plugin uninstall <id> --purge`     — also delete the cache dir

use anyhow::{bail, Result};
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::plugins::{self, PluginEntry, PluginStatus};

/// Handler for `/plugin`.
pub struct PluginHandler;

#[async_trait]
impl CommandHandler for PluginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            None | Some("list") | Some("ls") => Ok(handle_list(Filter::All)),
            Some("installed") => Ok(handle_list(Filter::Installed)),
            Some("disabled") => Ok(handle_list(Filter::Disabled)),
            Some("errors") | Some("error") => Ok(handle_list(Filter::Errored)),
            Some("status") => Ok(handle_status()),
            Some("enable") => {
                let id = parts.get(1).copied().unwrap_or("");
                handle_set_enabled(id, true)
            }
            Some("disable") => {
                let id = parts.get(1).copied().unwrap_or("");
                handle_set_enabled(id, false)
            }
            Some("uninstall") | Some("remove") | Some("rm") => {
                let id = parts.get(1).copied().unwrap_or("");
                // Check for --purge anywhere in the remaining tokens.
                let purge = parts.iter().skip(2).any(|p| *p == "--purge");
                handle_uninstall(id, purge)
            }
            Some("help") => Ok(handle_help()),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown plugin subcommand: '{}'\n{}",
                sub,
                usage_block()
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Help / usage
// ---------------------------------------------------------------------------

fn usage_block() -> &'static str {
    "Usage:\n  \
       /plugin                          -- list all plugins (layered view)\n  \
       /plugin installed                -- only installed & enabled\n  \
       /plugin disabled                 -- only disabled\n  \
       /plugin errors                   -- only plugins with an error status\n  \
       /plugin status                   -- summary + drift diagnostics\n  \
       /plugin enable <plugin-id>       -- enable plugin\n  \
       /plugin disable <plugin-id>      -- disable plugin\n  \
       /plugin uninstall <plugin-id>    -- remove from installed_plugins.json\n  \
       /plugin uninstall <id> --purge   -- also delete the cache directory"
}

fn handle_help() -> CommandResult {
    CommandResult::Output(format!(
        "Plugin management.\n\n\
         {}\n\n\
         Plugin metadata is persisted at ~/.cc-rust/plugins/installed_plugins.json.\n\
         Cache directories live at   ~/.cc-rust/plugins/cache/{{marketplace}}/{{id}}/.",
        usage_block()
    ))
}

// ---------------------------------------------------------------------------
// List (layered view)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum Filter {
    All,
    Installed,
    Disabled,
    Errored,
}

impl Filter {
    fn label(&self) -> &'static str {
        match self {
            Filter::All => "Plugins",
            Filter::Installed => "Installed plugins",
            Filter::Disabled => "Disabled plugins",
            Filter::Errored => "Plugins with errors",
        }
    }

    fn matches(&self, entry: &PluginEntry) -> bool {
        match self {
            Filter::All => true,
            Filter::Installed => matches!(entry.status, PluginStatus::Installed),
            Filter::Disabled => matches!(entry.status, PluginStatus::Disabled),
            Filter::Errored => matches!(entry.status, PluginStatus::Error(_)),
        }
    }
}

/// Row of the layered-state table for a single plugin.
struct Row {
    id: String,
    version: String,
    install_col: &'static str, // "yes" / "no"
    enabled_col: &'static str, // "yes" / "no" / "error"
    active_col: &'static str,  // "yes" / "no"
    error_detail: Option<String>,
    skills: Vec<String>,
    tools: Vec<String>,
    mcp: Vec<String>,
}

fn handle_list(filter: Filter) -> CommandResult {
    let rows = build_rows(filter);

    if rows.is_empty() {
        let empty_msg = match filter {
            Filter::All => "No plugins registered.".to_string(),
            other => format!(
                "No plugins match filter '{}'.",
                other.label().to_lowercase()
            ),
        };
        return CommandResult::Output(empty_msg);
    }

    let mut lines = Vec::new();
    lines.push(format!("{} ({}):", filter.label(), rows.len()));
    lines.push(String::new());
    lines.push(format!(
        "  {:<36} {:<10} {:<9} {:<8} {:<7}",
        "plugin", "version", "installed", "enabled", "active"
    ));
    lines.push(format!(
        "  {:-<36} {:-<10} {:-<9} {:-<8} {:-<7}",
        "", "", "", "", ""
    ));

    for row in &rows {
        lines.push(format!(
            "  {:<36} {:<10} {:<9} {:<8} {:<7}",
            truncate(&row.id, 36),
            truncate(&row.version, 10),
            row.install_col,
            row.enabled_col,
            row.active_col
        ));
        if let Some(ref err) = row.error_detail {
            lines.push(format!("      error: {}", err));
        }
        if !row.skills.is_empty() {
            lines.push(format!("      skills: {}", row.skills.join(", ")));
        }
        if !row.tools.is_empty() {
            lines.push(format!("      tools:  {}", row.tools.join(", ")));
        }
        if !row.mcp.is_empty() {
            lines.push(format!("      mcp:    {}", row.mcp.join(", ")));
        }
    }

    if let Some(reason) = plugins::needs_refresh() {
        lines.push(String::new());
        lines.push(format!(
            "Session drift: {} — run /reload-plugins to sync this session.",
            reason
        ));
    }

    CommandResult::Output(lines.join("\n"))
}

/// Build the layered rows for the view. Combines:
///   * `installed_plugins.json` on disk  — "installed" column
///   * Status within disk entries        — "enabled" column
///   * In-memory registry presence       — "active" column
fn build_rows(filter: Filter) -> Vec<Row> {
    let disk_plugins = plugins::loader::load_installed_plugins();
    let in_memory = plugins::get_all_plugins();

    use std::collections::HashMap;
    let mut by_id: HashMap<String, (Option<PluginEntry>, Option<PluginEntry>)> = HashMap::new();
    for p in &disk_plugins {
        by_id.entry(p.id.clone()).or_insert((None, None)).0 = Some(p.clone());
    }
    for p in &in_memory {
        by_id.entry(p.id.clone()).or_insert((None, None)).1 = Some(p.clone());
    }

    let mut rows: Vec<Row> = by_id
        .into_iter()
        .filter_map(|(id, (disk, mem))| {
            // Prefer the richest source for display metadata.
            let display_entry = disk.as_ref().or(mem.as_ref())?;
            if !filter.matches(display_entry) {
                return None;
            }

            let install_col = if disk.is_some() { "yes" } else { "no" };
            let enabled_col = match &display_entry.status {
                PluginStatus::Installed => "yes",
                PluginStatus::Disabled => "no",
                PluginStatus::Error(_) => "error",
                PluginStatus::NotInstalled => "no",
            };
            let active_col = if mem.is_some() { "yes" } else { "no" };

            let error_detail = if let PluginStatus::Error(e) = &display_entry.status {
                Some(e.clone())
            } else {
                None
            };

            Some(Row {
                id,
                version: display_entry.version.clone(),
                install_col,
                enabled_col,
                active_col,
                error_detail,
                skills: display_entry.skills.clone(),
                tools: display_entry.tools.clone(),
                mcp: display_entry.mcp_servers.clone(),
            })
        })
        .collect();

    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

// ---------------------------------------------------------------------------
// Status summary
// ---------------------------------------------------------------------------

fn handle_status() -> CommandResult {
    let disk = plugins::loader::load_installed_plugins();
    let memory = plugins::get_all_plugins();

    if disk.is_empty() && memory.is_empty() {
        return CommandResult::Output("No plugins registered.".to_string());
    }

    let mut installed = 0usize;
    let mut disabled = 0usize;
    let mut errored = 0usize;
    let mut not_installed = 0usize;

    for p in &disk {
        match p.status {
            PluginStatus::Installed => installed += 1,
            PluginStatus::Disabled => disabled += 1,
            PluginStatus::Error(_) => errored += 1,
            PluginStatus::NotInstalled => not_installed += 1,
        }
    }

    let mut lines = Vec::new();
    lines.push("Plugin status summary:".to_string());
    lines.push(format!("  - total on disk: {}", disk.len()));
    lines.push(format!("  - active in session: {}", memory.len()));
    lines.push(format!("  - installed (enabled): {}", installed));
    lines.push(format!("  - disabled: {}", disabled));
    lines.push(format!("  - error: {}", errored));
    if not_installed > 0 {
        lines.push(format!("  - not_installed: {}", not_installed));
    }

    if let Some(reason) = plugins::needs_refresh() {
        lines.push(String::new());
        lines.push(format!(
            "Session drift: {} — run /reload-plugins to bring this session back in sync.",
            reason
        ));
    } else {
        lines.push(String::new());
        lines.push("Session is in sync with disk.".to_string());
    }

    CommandResult::Output(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Enable / Disable (with drift-aware reload hint)
// ---------------------------------------------------------------------------

fn handle_set_enabled(plugin_id: &str, enable: bool) -> Result<CommandResult> {
    if plugin_id.trim().is_empty() {
        let action = if enable { "enable" } else { "disable" };
        bail!("Usage: /plugin {} <plugin-id>", action);
    }

    let mut installed_plugins = plugins::loader::load_installed_plugins();
    let Some(persisted) = installed_plugins.iter_mut().find(|p| p.id == plugin_id) else {
        return Ok(CommandResult::Output(format!(
            "Plugin '{}' not found in installed plugins.",
            plugin_id
        )));
    };

    let new_status = if enable {
        PluginStatus::Installed
    } else {
        PluginStatus::Disabled
    };

    persisted.status = new_status.clone();
    plugins::loader::save_installed_plugins(&installed_plugins)?;

    // Keep in-memory state in sync for current session when possible.
    let before = plugins::find_plugin(plugin_id);
    if plugins::set_plugin_status(plugin_id, new_status.clone()).is_none() {
        // If not present in memory yet, try an in-place refresh of just this id
        // without nuking the whole registry.
        if let Some(disk_entry) = installed_plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .cloned()
        {
            plugins::register_plugin(disk_entry);
        }
    }

    let action_done = if enable { "enabled" } else { "disabled" };
    let mut msg = format!("Plugin '{}' {}.", plugin_id, action_done);

    // After the change, check whether the session still matches disk. If the
    // active plugin list now differs (e.g. enable flipped an entry that isn't
    // yet reflected in discovered tools/skills), emit a RefreshNeeded event.
    if let Some(reason) = plugins::needs_refresh() {
        msg.push_str(&format!(
            "\nSession drift: {} — run /reload-plugins to apply.",
            reason
        ));
        emit_refresh_needed(reason);
    } else if before.is_none() && plugins::find_plugin(plugin_id).is_some() {
        // Newly-registered plugin: active tool/skill/mcp sets won't reflect
        // contributions until the session reloads. Signal softly.
        let reason = format!("'{}' added to session", plugin_id);
        msg.push_str("\nNote: run /reload-plugins to refresh contributed tools/skills/mcp.");
        emit_refresh_needed(reason);
    }

    Ok(CommandResult::Output(msg))
}

fn emit_refresh_needed(reason: String) {
    let event = crate::ipc::subsystem_events::SubsystemEvent::Plugin(
        crate::ipc::subsystem_events::PluginEvent::RefreshNeeded { reason },
    );
    // Plugins module owns the event sender static; route through a helper.
    plugins::emit_event_external(event);
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

fn handle_uninstall(plugin_id: &str, purge: bool) -> Result<CommandResult> {
    if plugin_id.trim().is_empty() {
        bail!("Usage: /plugin uninstall <plugin-id> [--purge]");
    }

    let removed = plugins::uninstall_plugin(plugin_id, purge)?;

    match removed {
        Some(entry) => {
            let mut msg = format!("Plugin '{}' uninstalled.", entry.id);
            if purge {
                msg.push_str(" Cache directory purged.");
            } else {
                msg.push_str(" (Cache directory kept; re-run with --purge to delete it.)");
            }
            Ok(CommandResult::Output(msg))
        }
        None => Ok(CommandResult::Output(format!(
            "Plugin '{}' is not installed.",
            plugin_id
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::plugins::{PluginEntry, PluginSource, PluginStatus};
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    fn make_plugin(id: &str, status: PluginStatus) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: PluginSource::Local {
                path: "/tmp".to_string(),
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

    /// Isolate CC_RUST_HOME + clear registry around a closure. Tests that touch
    /// installed_plugins.json must run serially.
    fn with_clean_state<T>(f: impl FnOnce() -> T) -> T {
        let tmp = tempfile::tempdir().expect("tempdir");
        let old = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", tmp.path());
        plugins::clear_plugins();
        let result = f();
        plugins::clear_plugins();
        match old {
            Some(v) => std::env::set_var("CC_RUST_HOME", v),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
        result
    }

    #[tokio::test]
    async fn plugin_help_works() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("help", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Plugin management")),
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn plugin_unknown_subcommand() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("wat", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown plugin subcommand")),
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn plugin_enable_missing_id_errors() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("enable", &mut ctx).await;
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Usage: /plugin enable"));
    }

    #[tokio::test]
    async fn plugin_uninstall_missing_id_errors() {
        let handler = PluginHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("uninstall", &mut ctx).await;
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Usage: /plugin uninstall"));
    }

    /// Synchronously run a handler's async execute — used inside a blocking
    /// test so we can mix async dispatch with env-var setup.
    fn run(handler: &PluginHandler, args: &str) -> CommandResult {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let mut ctx = test_ctx();
        rt.block_on(handler.execute(args, &mut ctx)).unwrap()
    }

    #[test]
    #[serial_test::serial]
    fn plugin_list_default_shows_all_layers() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[
                make_plugin("alpha", PluginStatus::Installed),
                make_plugin("beta", PluginStatus::Disabled),
            ])
            .unwrap();
            plugins::register_plugin(make_plugin("alpha", PluginStatus::Installed));
            // `beta` disabled on disk but NOT active (realistic scenario after disable).
            run(&handler, "list")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(text.contains("alpha"), "missing alpha row: {}", text);
                assert!(text.contains("beta"), "missing beta row: {}", text);
                // Columns present.
                assert!(text.contains("installed"));
                assert!(text.contains("enabled"));
                assert!(text.contains("active"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_installed_filter_excludes_disabled() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[
                make_plugin("alpha", PluginStatus::Installed),
                make_plugin("beta", PluginStatus::Disabled),
            ])
            .unwrap();
            plugins::register_plugin(make_plugin("alpha", PluginStatus::Installed));
            plugins::register_plugin(make_plugin("beta", PluginStatus::Disabled));
            run(&handler, "installed")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(text.contains("alpha"), "text: {}", text);
                assert!(
                    !text.contains("beta"),
                    "beta should not be listed: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_disabled_filter_shows_only_disabled() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[
                make_plugin("alpha", PluginStatus::Installed),
                make_plugin("beta", PluginStatus::Disabled),
            ])
            .unwrap();
            plugins::register_plugin(make_plugin("alpha", PluginStatus::Installed));
            plugins::register_plugin(make_plugin("beta", PluginStatus::Disabled));
            run(&handler, "disabled")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(text.contains("beta"), "text: {}", text);
                assert!(!text.contains("alpha"), "alpha should not be listed");
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_errors_filter_shows_error_only() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[
                make_plugin("working", PluginStatus::Installed),
                make_plugin("broken", PluginStatus::Error("boom".to_string())),
            ])
            .unwrap();
            plugins::register_plugin(make_plugin("working", PluginStatus::Installed));
            plugins::register_plugin(make_plugin(
                "broken",
                PluginStatus::Error("boom".to_string()),
            ));
            run(&handler, "errors")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(text.contains("broken"), "text: {}", text);
                assert!(text.contains("boom"), "error detail missing: {}", text);
                assert!(
                    !text.contains("working"),
                    "healthy plugin should be filtered: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_status_includes_drift_when_diverged() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            // Disk has one plugin, memory has none -> drift.
            plugins::loader::save_installed_plugins(&[make_plugin(
                "disk-only",
                PluginStatus::Installed,
            )])
            .unwrap();
            run(&handler, "status")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("Session drift"),
                    "drift line missing: {}",
                    text
                );
                assert!(
                    text.contains("disk-only"),
                    "drift plugin not named: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_status_reports_in_sync() {
        let handler = PluginHandler;
        let output = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[make_plugin("p", PluginStatus::Installed)])
                .unwrap();
            plugins::register_plugin(make_plugin("p", PluginStatus::Installed));
            run(&handler, "status")
        });
        match output {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("in sync"),
                    "expected in-sync line, got: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_uninstall_removes_entry() {
        let handler = PluginHandler;
        let (output, still_on_disk) = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[make_plugin(
                "doomed",
                PluginStatus::Installed,
            )])
            .unwrap();
            plugins::register_plugin(make_plugin("doomed", PluginStatus::Installed));
            let out = run(&handler, "uninstall doomed");
            let remaining = plugins::loader::load_installed_plugins();
            (out, remaining.iter().any(|p| p.id == "doomed"))
        });
        assert!(!still_on_disk, "doomed should have been removed");
        match output {
            CommandResult::Output(text) => {
                assert!(text.contains("doomed"), "got: {}", text);
                assert!(text.contains("uninstalled"), "got: {}", text);
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_uninstall_absent_reports_not_installed() {
        let handler = PluginHandler;
        let output = with_clean_state(|| run(&handler, "uninstall ghost"));
        match output {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("not installed") || text.contains("is not installed"),
                    "got: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn plugin_disable_flips_disk_status() {
        let handler = PluginHandler;
        let persisted = with_clean_state(|| {
            plugins::loader::save_installed_plugins(&[make_plugin(
                "togglable",
                PluginStatus::Installed,
            )])
            .unwrap();
            plugins::register_plugin(make_plugin("togglable", PluginStatus::Installed));
            run(&handler, "disable togglable");
            let disk = plugins::loader::load_installed_plugins();
            disk.iter().find(|p| p.id == "togglable").cloned()
        });
        let p = persisted.expect("togglable should still be on disk");
        assert_eq!(p.status, PluginStatus::Disabled);
    }
}
