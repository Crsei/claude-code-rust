//! `/doctor` slash command — aggregated diagnostics.
//!
//! Rust issue #39. Walks the configuration, auth, MCP, keybindings, and
//! terminal surfaces to produce a unified health report without a model
//! round-trip. Every row carries a severity badge so the user can scan for
//! red flags before reaching for `/terminal-setup`, `/config show`,
//! `/keybindings`, or `/mcp` individually.
//!
//! The aggregator is pure data extraction — it does not mutate anything.
//! Rendering lives in `render_report` and uses the shared tree-view helper
//! so the layout stays consistent with `/hooks`, `/agents`, and `/tasks`.

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth::{resolve_auth, AuthMethod};
use crate::config::paths;
use crate::config::settings::{load_effective, SettingsSource};
use crate::config::validation::{validate_settings, WarningSeverity};
use crate::ui::browser::{render_with_footer, shorten_path, TreeNode};

use super::terminal_setup::TerminalLabel;

pub struct DoctorHandler;

#[async_trait]
impl CommandHandler for DoctorHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim().to_ascii_lowercase();
        let report = DoctorReport::build(ctx);
        let body = match args.as_str() {
            "" | "all" | "full" => report.render_full(),
            "summary" => report.render_summary(),
            "raw" | "json" => serde_json::to_string_pretty(&report.to_json())
                .unwrap_or_else(|e| format!("(serialisation error: {})", e)),
            other => format!(
                "Unknown /doctor subcommand '{}'.\n\n\
                 Usage:\n  \
                 /doctor           — full aggregated diagnostics\n  \
                 /doctor summary   — just the status counts + headline issues\n  \
                 /doctor raw       — machine-readable JSON payload\n",
                other
            ),
        };
        Ok(CommandResult::Output(body))
    }
}

// ---------------------------------------------------------------------------
// Severity + Rows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
    Info,
}

impl Status {
    fn tag(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
            Status::Info => "info",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Status::Ok => 0,
            Status::Info => 1,
            Status::Warn => 2,
            Status::Fail => 3,
        }
    }
}

#[derive(Debug, Clone)]
struct Row {
    name: String,
    status: Status,
    detail: String,
}

impl Row {
    fn new(name: &str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Section {
    name: String,
    rows: Vec<Row>,
}

impl Section {
    fn worst(&self) -> Status {
        self.rows
            .iter()
            .map(|r| r.status)
            .max_by_key(|s| s.rank())
            .unwrap_or(Status::Ok)
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

struct DoctorReport {
    sections: Vec<Section>,
}

impl DoctorReport {
    fn build(ctx: &CommandContext) -> Self {
        let mut sections = Vec::new();
        sections.push(build_install_section());
        sections.push(build_auth_section());
        sections.push(build_settings_section(&ctx.cwd, ctx));
        sections.push(build_mcp_section(&ctx.cwd));
        sections.push(build_keybindings_section(ctx));
        sections.push(build_sandbox_section(ctx));
        sections.push(build_terminal_section());
        Self { sections }
    }

    fn counts(&self) -> (usize, usize, usize, usize) {
        let (mut ok, mut warn, mut fail, mut info) = (0, 0, 0, 0);
        for s in &self.sections {
            for r in &s.rows {
                match r.status {
                    Status::Ok => ok += 1,
                    Status::Warn => warn += 1,
                    Status::Fail => fail += 1,
                    Status::Info => info += 1,
                }
            }
        }
        (ok, warn, fail, info)
    }

    fn render_summary(&self) -> String {
        let (ok, warn, fail, info) = self.counts();
        let mut out = String::new();
        out.push_str("Doctor summary\n");
        out.push_str("──────────────\n");
        out.push_str(&format!(
            "  ok: {}   warn: {}   fail: {}   info: {}\n",
            ok, warn, fail, info
        ));
        if fail == 0 && warn == 0 {
            out.push_str("  All checks passed.\n");
            return out;
        }
        out.push_str("\nHeadline issues:\n");
        for section in &self.sections {
            for row in &section.rows {
                if matches!(row.status, Status::Fail | Status::Warn) {
                    out.push_str(&format!(
                        "  [{}] {} / {}: {}\n",
                        row.status.tag(),
                        section.name,
                        row.name,
                        row.detail
                    ));
                }
            }
        }
        out
    }

    fn render_full(&self) -> String {
        let mut roots = Vec::new();
        for section in &self.sections {
            let mut section_node =
                TreeNode::leaf(section.name.clone()).with_badge(section.worst().tag().to_string());
            for row in &section.rows {
                section_node.push_child(
                    TreeNode::leaf(row.name.clone())
                        .with_badge(row.status.tag().to_string())
                        .with_detail(row.detail.clone()),
                );
            }
            roots.push(section_node);
        }

        let (ok, warn, fail, _info) = self.counts();
        let footer = format!(
            "Counts: {} ok, {} warn, {} fail.\n\
             Use `/doctor summary` for just the red flags, \
             or `/doctor raw` for JSON.",
            ok, warn, fail
        );
        render_with_footer("Doctor", &roots, &footer)
    }

    fn to_json(&self) -> serde_json::Value {
        let sections: Vec<_> = self
            .sections
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "status": s.worst().tag(),
                    "rows": s.rows.iter().map(|r| serde_json::json!({
                        "name": r.name,
                        "status": r.status.tag(),
                        "detail": r.detail,
                    })).collect::<Vec<_>>()
                })
            })
            .collect();
        let (ok, warn, fail, info) = self.counts();
        serde_json::json!({
            "counts": { "ok": ok, "warn": warn, "fail": fail, "info": info },
            "sections": sections,
        })
    }
}

// ---------------------------------------------------------------------------
// Section builders
// ---------------------------------------------------------------------------

fn build_install_section() -> Section {
    let mut rows = Vec::new();
    let root = paths::data_root();
    let exists = root.is_dir();
    rows.push(Row::new(
        "data root",
        if exists { Status::Ok } else { Status::Warn },
        if exists {
            format!("{}", shorten_path(&root))
        } else {
            format!("not created yet ({})", shorten_path(&root))
        },
    ));

    if let Ok(override_dir) = std::env::var("CC_RUST_HOME") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            rows.push(Row::new(
                "CC_RUST_HOME",
                Status::Info,
                format!("override active: {}", trimmed),
            ));
        }
    }

    rows.push(Row::new(
        "cc-rust version",
        Status::Info,
        env!("CARGO_PKG_VERSION").to_string(),
    ));

    Section {
        name: "Install".to_string(),
        rows,
    }
}

fn build_auth_section() -> Section {
    let mut rows = Vec::new();
    let method = resolve_auth();
    let (status, detail) = match &method {
        AuthMethod::ApiKey(_) => (Status::Ok, "ANTHROPIC_API_KEY / keychain".to_string()),
        AuthMethod::ExternalToken(_) => (Status::Ok, "ANTHROPIC_AUTH_TOKEN".to_string()),
        AuthMethod::OAuthToken { method, .. } => (Status::Ok, format!("OAuth ({})", method)),
        AuthMethod::None => (
            Status::Fail,
            "no credential found — run `/login` to authenticate".to_string(),
        ),
    };
    rows.push(Row::new("credential", status, detail));

    let cred_path = paths::credentials_path();
    if cred_path.exists() {
        rows.push(Row::new(
            "credentials.json",
            Status::Info,
            shorten_path(&cred_path),
        ));
    }
    Section {
        name: "Auth".to_string(),
        rows,
    }
}

fn build_settings_section(cwd: &Path, ctx: &CommandContext) -> Section {
    let mut rows = Vec::new();
    match load_effective(cwd) {
        Ok(loaded) => {
            for (source, path) in &loaded.loaded_paths {
                let label = match source {
                    SettingsSource::Managed => "managed",
                    SettingsSource::User => "user",
                    SettingsSource::Project => "project",
                    SettingsSource::Local => "local",
                    other => other.as_str(),
                };
                rows.push(Row::new(label, Status::Ok, shorten_path(path)));
            }
            if loaded.loaded_paths.is_empty() {
                rows.push(Row::new(
                    "layers loaded",
                    Status::Info,
                    "none found — running on defaults".to_string(),
                ));
            }
        }
        Err(e) => {
            rows.push(Row::new("load", Status::Fail, e.to_string()));
        }
    }

    for w in validate_settings(&ctx.app_state.settings) {
        let status = match w.severity {
            WarningSeverity::Info => Status::Info,
            WarningSeverity::Warning => Status::Warn,
            WarningSeverity::Error => Status::Fail,
        };
        rows.push(Row::new(&w.field, status, w.message));
    }

    if rows
        .iter()
        .all(|r| r.status == Status::Ok || r.status == Status::Info)
    {
        rows.push(Row::new(
            "validation",
            Status::Ok,
            "no validation warnings".to_string(),
        ));
    }

    Section {
        name: "Settings".to_string(),
        rows,
    }
}

fn build_mcp_section(cwd: &Path) -> Section {
    let mut rows = Vec::new();
    match crate::mcp::discovery::discover_mcp_servers(cwd) {
        Ok(servers) => {
            if servers.is_empty() {
                rows.push(Row::new(
                    "discovered",
                    Status::Info,
                    "no MCP servers configured".to_string(),
                ));
            } else {
                for server in &servers {
                    let cmd = server
                        .command
                        .as_deref()
                        .unwrap_or("(no command — URL-based transport)");
                    rows.push(Row::new(
                        &server.name,
                        Status::Ok,
                        format!("command: {}", cmd),
                    ));
                }
            }
        }
        Err(e) => {
            rows.push(Row::new(
                "discovery",
                Status::Warn,
                format!("could not enumerate MCP servers: {}", e),
            ));
        }
    }
    Section {
        name: "MCP".to_string(),
        rows,
    }
}

fn build_keybindings_section(ctx: &CommandContext) -> Section {
    let mut rows = Vec::new();
    let reg = ctx.app_state.keybindings.clone();
    reg.refresh_if_changed();
    let path = reg.user_path().unwrap_or_else(paths::keybindings_path);
    let exists = path.exists();
    rows.push(Row::new(
        "config",
        if exists { Status::Ok } else { Status::Info },
        if exists {
            shorten_path(&path)
        } else {
            format!("not created ({})", shorten_path(&path))
        },
    ));
    rows.push(Row::new(
        "effective bindings",
        Status::Ok,
        reg.all_bindings().len().to_string(),
    ));
    for issue in reg.last_issues() {
        rows.push(Row::new("issue", Status::Warn, issue));
    }
    Section {
        name: "Keybindings".to_string(),
        rows,
    }
}

fn build_sandbox_section(ctx: &CommandContext) -> Section {
    let mut rows = Vec::new();
    let sandbox = &ctx.app_state.settings.sandbox;
    let enabled = sandbox.enabled.unwrap_or(false);
    rows.push(Row::new(
        "enabled",
        if enabled { Status::Ok } else { Status::Info },
        enabled.to_string(),
    ));
    if let Some(mode) = sandbox.mode.as_ref() {
        rows.push(Row::new("mode", Status::Info, mode.clone()));
    }
    if let Some(fail) = sandbox.fail_if_unavailable {
        rows.push(Row::new(
            "failIfUnavailable",
            Status::Info,
            fail.to_string(),
        ));
    }
    if !sandbox.network.allowed_domains.is_empty() {
        rows.push(Row::new(
            "allowedDomains",
            Status::Info,
            format!("{} entries", sandbox.network.allowed_domains.len()),
        ));
    }
    Section {
        name: "Sandbox".to_string(),
        rows,
    }
}

fn build_terminal_section() -> Section {
    let mut rows = Vec::new();
    let probe = super::terminal_setup::EnvProbe::from_env();
    let label = probe.terminal_label();
    rows.push(Row::new(
        "terminal",
        if matches!(label, TerminalLabel::Unknown) {
            Status::Info
        } else {
            Status::Ok
        },
        label.as_str().to_string(),
    ));
    if let Some(shell) = probe.shell.as_deref() {
        rows.push(Row::new("shell", Status::Info, shell.to_string()));
    }
    if probe.tmux.is_some() {
        rows.push(Row::new("tmux", Status::Info, "active".to_string()));
    }
    if let (None, None) = (&probe.visual, &probe.editor) {
        rows.push(Row::new(
            "editor",
            Status::Warn,
            "$VISUAL / $EDITOR unset — commands that shell out to an editor will fall back to printing the path".to_string(),
        ));
    } else {
        rows.push(Row::new(
            "editor",
            Status::Ok,
            probe
                .visual
                .as_deref()
                .or(probe.editor.as_deref())
                .unwrap_or("")
                .to_string(),
        ));
    }
    Section {
        name: "Terminal".to_string(),
        rows,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn full_report_contains_every_section() {
        let handler = DoctorHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                for section in [
                    "Install",
                    "Auth",
                    "Settings",
                    "MCP",
                    "Keybindings",
                    "Sandbox",
                    "Terminal",
                ] {
                    assert!(s.contains(section), "missing section `{}`: {}", section, s);
                }
                assert!(s.contains("Counts:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn summary_includes_counts() {
        let handler = DoctorHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("summary", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Doctor summary"));
                assert!(s.contains("ok:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn raw_emits_valid_json() {
        let handler = DoctorHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("raw", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&s).expect("raw should be valid JSON");
                assert!(parsed.get("sections").is_some());
                assert!(parsed.get("counts").is_some());
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_returns_usage() {
        let handler = DoctorHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("zzz", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /doctor"));
                assert!(s.contains("Usage"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn status_rank_orders_correctly() {
        assert!(Status::Ok.rank() < Status::Info.rank());
        assert!(Status::Info.rank() < Status::Warn.rank());
        assert!(Status::Warn.rank() < Status::Fail.rank());
    }

    #[test]
    fn section_worst_picks_highest_rank() {
        let section = Section {
            name: "x".into(),
            rows: vec![
                Row::new("a", Status::Ok, ""),
                Row::new("b", Status::Warn, ""),
                Row::new("c", Status::Info, ""),
            ],
        };
        assert_eq!(section.worst(), Status::Warn);
    }
}
