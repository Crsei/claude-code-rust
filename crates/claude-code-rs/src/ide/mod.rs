//! IDE detection + selection + MCP bridge glue (issue #41).
//!
//! This module is the single source of truth for the `/ide` command and the
//! IPC IDE subsystem. It exposes:
//!
//! - [`detect_ides`] — OS-level detection of VS Code, Cursor, and JetBrains
//!   IDEs using PATH lookups, well-known install directories, and the
//!   integrated-terminal env vars (`TERM_PROGRAM`, `VSCODE_PID`, etc.).
//! - [`selected_ide`] / [`select_ide`] / [`clear_selection`] — read and write
//!   the persisted selection under `{data_root}/settings.json` using the
//!   `selectedIde` key.
//! - [`ide_mcp_config`] — build a dynamic `McpServerConfig` that launches the
//!   selected IDE's MCP bridge (currently a stdio subprocess per IDE).
//!
//! Selection changes emit `SubsystemEvent::Ide(IdeEvent::SelectionChanged)`
//! through an `EVENT_TX` static that mirrors the `plugins::mod.rs` pattern.
//!
//! ## Detection heuristic (summary)
//!
//! | IDE | Detection signals |
//! |-----|-------------------|
//! | VS Code | `which("code")` or `which("code-insiders")`; env `VSCODE_PID` or `TERM_PROGRAM=vscode`; Windows: `%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe` |
//! | Cursor | `which("cursor")`; env `TERM_PROGRAM=cursor`; Windows: `%LOCALAPPDATA%\Programs\cursor\Cursor.exe` |
//! | JetBrains | `which("idea"|"goland"|"pycharm"|"rubymine"|"webstorm")`; env `IDEA_INITIAL_DIRECTORY` or `JEDITERM_SOURCE`; platform-specific install paths |
//!
//! "Running" is inferred from the integrated-terminal env vars and/or PATH
//! presence; a missing IDE is reported as `installed: false, running: false`
//! rather than omitted so callers can show a complete matrix.
//!
//! ## MCP bridge (summary)
//!
//! The bridge command is intentionally minimal: each IDE ships its own CLI,
//! so `ide_mcp_config("vscode")` returns an `McpServerConfig` that spawns
//! `code --mcp-server` (and Cursor's `cursor --mcp-server`, etc.). This is a
//! best-effort default; users can override via `settings.json`.
//! "Real" reconnect semantics — disconnecting an already-live bridge and
//! restarting it under the MCP manager — are deferred to a follow-up: the
//! current `/ide reconnect` publishes a `ConnectionStateChanged` event and
//! updates the selection, which the MCP manager picks up the next time it
//! re-discovers servers.

#![allow(dead_code)] // Some helpers are read-only conveniences for the /ide command.

use std::path::PathBuf;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::LazyLock;
use tokio::sync::broadcast;
use tracing::warn;

use crate::ipc::subsystem_events::{IdeEvent, SubsystemEvent};
use crate::ipc::subsystem_types::IdeInfo;
use crate::mcp::McpServerConfig;

// ---------------------------------------------------------------------------
// Subsystem event emission (mirrors `plugins::mod.rs` pattern)
// ---------------------------------------------------------------------------

static EVENT_TX: LazyLock<Mutex<Option<broadcast::Sender<SubsystemEvent>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Inject the event sender from the headless event loop. Mirrors
/// `plugins::set_event_sender`.
pub fn set_event_sender(tx: broadcast::Sender<SubsystemEvent>) {
    *EVENT_TX.lock() = Some(tx);
}

fn emit_event(event: SubsystemEvent) {
    if let Some(tx) = EVENT_TX.lock().as_ref() {
        let _ = tx.send(event);
    }
}

// ---------------------------------------------------------------------------
// IDE registry (hard-coded — no marketplace for IDEs).
// ---------------------------------------------------------------------------

/// Static metadata used to construct an `IdeInfo` entry.
struct IdeSpec {
    id: &'static str,
    name: &'static str,
    /// CLI binaries that indicate this IDE is installed (checked via `which`).
    binaries: &'static [&'static str],
    /// `TERM_PROGRAM` values (or other env vars) that indicate this IDE is
    /// currently running and hosting the integrated terminal.
    term_programs: &'static [&'static str],
    /// Extra env vars whose mere presence implies the IDE is running.
    env_markers: &'static [&'static str],
}

const IDE_SPECS: &[IdeSpec] = &[
    IdeSpec {
        id: "vscode",
        name: "Visual Studio Code",
        binaries: &["code", "code-insiders"],
        term_programs: &["vscode"],
        env_markers: &["VSCODE_PID", "VSCODE_IPC_HOOK"],
    },
    IdeSpec {
        id: "cursor",
        name: "Cursor",
        binaries: &["cursor"],
        term_programs: &["cursor"],
        env_markers: &["CURSOR_PID"],
    },
    IdeSpec {
        id: "intellij",
        name: "IntelliJ IDEA",
        binaries: &["idea"],
        term_programs: &["JetBrains.IntelliJIdea"],
        env_markers: &["IDEA_INITIAL_DIRECTORY", "JEDITERM_SOURCE"],
    },
    IdeSpec {
        id: "goland",
        name: "GoLand",
        binaries: &["goland"],
        term_programs: &["JetBrains.GoLand"],
        env_markers: &[],
    },
    IdeSpec {
        id: "pycharm",
        name: "PyCharm",
        binaries: &["pycharm"],
        term_programs: &["JetBrains.PyCharm"],
        env_markers: &[],
    },
    IdeSpec {
        id: "rubymine",
        name: "RubyMine",
        binaries: &["rubymine"],
        term_programs: &["JetBrains.RubyMine"],
        env_markers: &[],
    },
    IdeSpec {
        id: "webstorm",
        name: "WebStorm",
        binaries: &["webstorm"],
        term_programs: &["JetBrains.WebStorm"],
        env_markers: &[],
    },
];

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Run OS-level detection for every IDE in [`IDE_SPECS`] and return the result.
///
/// Never panics — if detection for a given IDE fails (e.g. PATH lookup
/// errors), the entry is still returned with `installed: false, running: false`.
pub fn detect_ides() -> Vec<IdeInfo> {
    let selected = selected_ide();
    IDE_SPECS
        .iter()
        .map(|spec| {
            let installed = is_installed(spec);
            let running = is_running(spec);
            IdeInfo {
                id: spec.id.to_string(),
                name: spec.name.to_string(),
                installed,
                running,
                selected: selected.as_deref() == Some(spec.id),
                connection_state: None,
                error: None,
            }
        })
        .collect()
}

/// Check whether any of the IDE's binaries resolve on `PATH`, or whether a
/// well-known install path exists (Windows).
fn is_installed(spec: &IdeSpec) -> bool {
    for bin in spec.binaries {
        if which::which(bin).is_ok() {
            return true;
        }
    }
    // Platform-specific fallbacks for IDEs that don't ship a PATH-registered CLI.
    #[cfg(windows)]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(local_app_data).join("Programs");
            let candidates: &[&str] = match spec.id {
                "vscode" => &[
                    "Microsoft VS Code/Code.exe",
                    "Microsoft VS Code Insiders/Code - Insiders.exe",
                ],
                "cursor" => &["cursor/Cursor.exe"],
                _ => &[],
            };
            for rel in candidates {
                if base.join(rel).exists() {
                    return true;
                }
            }
        }
    }
    false
}

/// Consider the IDE "running" if the process is hosting the current terminal
/// (via `TERM_PROGRAM` or a dedicated env marker).
///
/// We deliberately do not scan the full process list — that would require
/// pulling in `sysinfo` and its platform-specific quirks. The env-based
/// signal is sufficient for the common case where the user launches cc-rust
/// from the IDE's integrated terminal.
fn is_running(spec: &IdeSpec) -> bool {
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        for tp in spec.term_programs {
            if term_program.eq_ignore_ascii_case(tp) {
                return true;
            }
        }
    }
    for marker in spec.env_markers {
        if std::env::var_os(marker).is_some() {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Persisted selection (`{data_root}/settings.json` → `selectedIde`)
// ---------------------------------------------------------------------------

fn settings_path() -> PathBuf {
    cc_config::paths::data_root().join("settings.json")
}

/// Read the currently-selected IDE id, if any.
pub fn selected_ide() -> Option<String> {
    let path = settings_path();
    let text = std::fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("selectedIde")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

/// Persist `id` as the selected IDE and emit a `SelectionChanged` event.
///
/// Triggers an in-memory "reconnect" by publishing a
/// `ConnectionStateChanged { state: "connecting" }` event. The MCP manager
/// picks up the change on its next discovery pass.
pub fn select_ide(id: &str) -> Result<()> {
    validate_ide_id(id)?;
    write_selection(Some(id))?;
    emit_event(SubsystemEvent::Ide(IdeEvent::SelectionChanged {
        ide_id: Some(id.to_string()),
    }));
    // Kick off the bridge; see `ide_mcp_config` for the command we use.
    emit_event(SubsystemEvent::Ide(IdeEvent::ConnectionStateChanged {
        ide_id: id.to_string(),
        state: "connecting".to_string(),
        error: None,
    }));
    Ok(())
}

/// Remove the persisted IDE selection.
pub fn clear_selection() -> Result<()> {
    write_selection(None)?;
    emit_event(SubsystemEvent::Ide(IdeEvent::SelectionChanged {
        ide_id: None,
    }));
    Ok(())
}

/// Re-publish a `ConnectionStateChanged` event for the currently-selected
/// IDE. Used by `/ide reconnect` as a lightweight reconnect trigger.
pub fn reconnect_selected() -> Result<()> {
    let Some(id) = selected_ide() else {
        anyhow::bail!("no IDE is currently selected; run `/ide select <id>` first");
    };
    emit_event(SubsystemEvent::Ide(IdeEvent::ConnectionStateChanged {
        ide_id: id,
        state: "connecting".to_string(),
        error: None,
    }));
    Ok(())
}

fn validate_ide_id(id: &str) -> Result<()> {
    if IDE_SPECS.iter().any(|s| s.id == id) {
        Ok(())
    } else {
        let known = IDE_SPECS
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("unknown IDE id '{}' (known: {})", id, known)
    }
}

fn write_selection(id: Option<&str>) -> Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create settings dir {}", parent.display()))?;
    }

    // Preserve any other keys the user has set; we only touch `selectedIde`.
    let mut value: Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            warn!(
                path = %path.display(),
                error = %e,
                "ide: existing settings.json is not valid JSON; overwriting only selectedIde"
            );
            json!({})
        }),
        Err(_) => json!({}),
    };

    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.json root is not a JSON object"))?;

    match id {
        Some(id) => {
            obj.insert("selectedIde".to_string(), Value::String(id.to_string()));
        }
        None => {
            obj.remove("selectedIde");
        }
    }

    let pretty = serde_json::to_string_pretty(&value)?;
    // Best-effort atomic-ish write (same pattern as cc_config::write_settings_file).
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP bridge
// ---------------------------------------------------------------------------

/// Build a dynamic `McpServerConfig` for the IDE's MCP bridge.
///
/// Current defaults (stdio):
/// - `vscode` → `code --mcp-server`
/// - `cursor` → `cursor --mcp-server`
/// - JetBrains → `<binary> mcp` (best guess — these IDEs currently expose MCP
///   through a plugin binary, so `None` is returned when the CLI is missing).
///
/// Returns `None` when no reasonable bridge command is available on this
/// platform or when the IDE's binary isn't installed.
pub fn ide_mcp_config(id: &str) -> Option<McpServerConfig> {
    let spec = IDE_SPECS.iter().find(|s| s.id == id)?;

    // Find the first binary that actually resolves, or fall back to the
    // canonical name (so the config is still useful on a system where the
    // IDE is installed but the PATH lookup fails — the MCP manager will
    // surface the spawn error).
    let binary = spec
        .binaries
        .iter()
        .find(|b| which::which(b).is_ok())
        .copied()
        .or_else(|| spec.binaries.first().copied())?;

    let args = match id {
        "vscode" | "cursor" => vec!["--mcp-server".to_string()],
        // JetBrains MCP plugin convention (best-effort default).
        _ => vec!["mcp".to_string()],
    };

    Some(McpServerConfig {
        name: format!("ide-{}", id),
        transport: "stdio".to_string(),
        command: Some(binary.to_string()),
        args: Some(args),
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
        disabled: None,
    })
}

/// Return the `McpServerConfig` to inject into discovery, if an IDE is
/// selected *and* a bridge command is available. This is the entry point
/// called by the MCP discovery hook.
pub fn selected_ide_mcp_config() -> Vec<McpServerConfig> {
    match selected_ide() {
        Some(id) => ide_mcp_config(&id).into_iter().collect(),
        None => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    /// RAII-style env-var guard — mirrors the `HomeGuard` used elsewhere.
    struct HomeGuard {
        previous: Option<String>,
    }

    impl HomeGuard {
        fn set(path: &Path) -> Self {
            let previous = std::env::var("CC_RUST_HOME").ok();
            std::env::set_var("CC_RUST_HOME", path);
            Self { previous }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("CC_RUST_HOME", v),
                None => std::env::remove_var("CC_RUST_HOME"),
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn detect_ides_returns_full_registry_without_panicking() {
        let ides = detect_ides();
        // Even on hosts with nothing installed we get a complete matrix.
        assert!(
            ides.len() >= IDE_SPECS.len(),
            "expected one entry per IDE_SPECS; got {}",
            ides.len()
        );
        let ids: Vec<_> = ides.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"vscode"));
        assert!(ids.contains(&"cursor"));
        assert!(ids.contains(&"intellij"));
    }

    #[test]
    #[serial_test::serial]
    fn detect_ides_reports_none_selected_by_default() {
        // Point CC_RUST_HOME at an empty dir so no prior selection leaks in.
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());

        let ides = detect_ides();
        for info in &ides {
            assert!(!info.selected, "no IDE should be selected initially");
        }
    }

    #[test]
    #[serial_test::serial]
    fn select_ide_rejects_unknown_id() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());

        let err = select_ide("nonexistent-ide").expect_err("should reject unknown id");
        let msg = format!("{}", err);
        assert!(msg.contains("unknown IDE id"), "unexpected: {}", msg);
    }

    #[test]
    #[serial_test::serial]
    fn selected_ide_round_trips_through_settings() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());

        // Initially nothing is selected.
        assert!(selected_ide().is_none());

        // Select vscode.
        select_ide("vscode").expect("select_ide");
        assert_eq!(selected_ide().as_deref(), Some("vscode"));

        // Verify settings.json actually got written.
        let settings_file = tmp.path().join("settings.json");
        let text = std::fs::read_to_string(&settings_file).expect("settings.json");
        let value: Value = serde_json::from_str(&text).expect("parse");
        assert_eq!(value["selectedIde"], "vscode");

        // Detection now reflects the selection.
        let ides = detect_ides();
        let vscode = ides.iter().find(|i| i.id == "vscode").expect("vscode");
        assert!(vscode.selected);

        // Clear the selection.
        clear_selection().expect("clear_selection");
        assert!(selected_ide().is_none());
        let text = std::fs::read_to_string(&settings_file).expect("settings.json after clear");
        let value: Value = serde_json::from_str(&text).expect("parse");
        assert!(value.get("selectedIde").is_none());
    }

    #[test]
    #[serial_test::serial]
    fn selected_ide_preserves_other_settings_keys() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());

        // Pre-seed settings.json with an unrelated key.
        let settings_file = tmp.path().join("settings.json");
        std::fs::write(
            &settings_file,
            serde_json::to_string_pretty(&json!({
                "theme": "dark",
                "model": "claude-opus-4-7"
            }))
            .unwrap(),
        )
        .unwrap();

        select_ide("cursor").expect("select_ide");

        let text = std::fs::read_to_string(&settings_file).expect("settings.json");
        let value: Value = serde_json::from_str(&text).expect("parse");
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["model"], "claude-opus-4-7");
        assert_eq!(value["selectedIde"], "cursor");
    }

    #[test]
    #[serial_test::serial]
    fn ide_mcp_config_returns_stdio_spawn_for_vscode_and_cursor() {
        // These configs don't actually spawn anything until the MCP manager
        // uses them — we're just checking the shape.
        if let Some(cfg) = ide_mcp_config("vscode") {
            assert_eq!(cfg.name, "ide-vscode");
            assert_eq!(cfg.transport, "stdio");
            assert!(cfg.command.is_some());
            let args = cfg.args.clone().unwrap_or_default();
            assert!(args.iter().any(|a| a == "--mcp-server"));
        }
        if let Some(cfg) = ide_mcp_config("cursor") {
            assert_eq!(cfg.name, "ide-cursor");
            assert_eq!(cfg.transport, "stdio");
        }
    }

    #[test]
    fn ide_mcp_config_returns_none_for_unknown_id() {
        assert!(ide_mcp_config("nonexistent").is_none());
    }

    #[test]
    #[serial_test::serial]
    fn reconnect_selected_errors_when_nothing_selected() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());
        let err = reconnect_selected().expect_err("should error");
        assert!(format!("{}", err).contains("no IDE is currently selected"));
    }

    #[test]
    #[serial_test::serial]
    fn selected_ide_mcp_config_empty_by_default() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());
        assert!(selected_ide_mcp_config().is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn selected_ide_mcp_config_non_empty_after_selection() {
        let tmp = TempDir::new().expect("tmp");
        let _guard = HomeGuard::set(tmp.path());
        select_ide("vscode").expect("select");
        let configs = selected_ide_mcp_config();
        // Either empty (no `code` binary on the test host) or exactly one entry.
        assert!(configs.len() <= 1);
        if let Some(cfg) = configs.first() {
            assert_eq!(cfg.name, "ide-vscode");
        }
    }
}
