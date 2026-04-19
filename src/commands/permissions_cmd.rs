//! /permissions command — source-aware view and management of the
//! permission system.
//!
//! Subcommands:
//! - `/permissions`                              show effective rules + sources
//! - `/permissions mode <m>`                     switch permission mode
//!     where `<m>` ∈ default | auto | bypass | plan | acceptEdits | dontAsk
//! - `/permissions allow <rule>  [scope]`        add an always-allow rule
//! - `/permissions deny  <rule>  [scope]`        add an always-deny rule
//! - `/permissions ask   <rule>  [scope]`        add an always-ask rule
//! - `/permissions session-grant <tool>`         add a session-only grant
//! - `/permissions clear-session-grants`         drop all session grants
//! - `/permissions reset`                        reset all in-memory rules
//!
//! `<scope>` is one of `--user|--project|--local|--session` (default: --user).
//! Persistent scopes are also written back to the corresponding settings
//! file via [`config::settings`] (with backup).

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings;
use crate::types::tool::PermissionMode;

#[derive(Debug, Clone, Copy)]
enum PersistScope {
    User,
    Project,
    Local,
    Session,
}

impl PersistScope {
    fn parse(parts: &mut Vec<&str>) -> PersistScope {
        let mut scope = PersistScope::User;
        parts.retain(|p| match *p {
            "--user" => {
                scope = PersistScope::User;
                false
            }
            "--project" => {
                scope = PersistScope::Project;
                false
            }
            "--local" => {
                scope = PersistScope::Local;
                false
            }
            "--session" => {
                scope = PersistScope::Session;
                false
            }
            _ => true,
        });
        scope
    }

    fn source_label(self) -> &'static str {
        match self {
            PersistScope::User => "user",
            PersistScope::Project => "project",
            PersistScope::Local => "local",
            PersistScope::Session => "session",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RuleKind {
    Allow,
    Ask,
    Deny,
}

impl RuleKind {
    fn label(self) -> &'static str {
        match self {
            RuleKind::Allow => "allow",
            RuleKind::Ask => "ask",
            RuleKind::Deny => "deny",
        }
    }
}

/// Handler for the `/permissions` slash command.
pub struct PermissionsHandler;

#[async_trait]
impl CommandHandler for PermissionsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("mode") => handle_mode(&parts[1..], ctx),
            Some("allow") => handle_add(RuleKind::Allow, &parts[1..], ctx),
            Some("ask") => handle_add(RuleKind::Ask, &parts[1..], ctx),
            Some("deny") => handle_add(RuleKind::Deny, &parts[1..], ctx),
            Some("session-grant") => handle_session_grant(&parts[1..], ctx),
            Some("clear-session-grants") => handle_clear_session(ctx),
            Some("reset") => handle_reset(ctx),
            None => handle_show(ctx),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown permissions subcommand: '{}'\n{}",
                sub,
                usage()
            ))),
        }
    }
}

fn usage() -> &'static str {
    "Usage:\n  \
       /permissions                              -- show effective settings + sources\n  \
       /permissions mode <m>                     -- m: default|auto|bypass|plan|acceptEdits|dontAsk\n  \
       /permissions allow <rule> [scope]         -- always-allow rule\n  \
       /permissions ask   <rule> [scope]         -- always-ask rule\n  \
       /permissions deny  <rule> [scope]         -- always-deny rule\n  \
       /permissions session-grant <tool>         -- session-only allow\n  \
       /permissions clear-session-grants         -- drop all session grants\n  \
       /permissions reset                        -- reset in-memory rules\n\n  \
       scope: --user (default) | --project | --local | --session"
}

// ---------------------------------------------------------------------------
// Show
// ---------------------------------------------------------------------------

fn handle_show(ctx: &CommandContext) -> Result<CommandResult> {
    let perm = &ctx.app_state.tool_permission_context;

    let mut lines = Vec::new();
    lines.push("Permission settings:".into());
    lines.push(String::new());
    lines.push(format!(
        "  Mode:                {} (bypass available={}, auto available={:?})",
        perm.mode.as_str(),
        perm.is_bypass_permissions_mode_available,
        perm.is_auto_mode_available,
    ));

    // Effective rules grouped by kind, with source.
    render_rules(
        "Always deny  (deny > ask > allow)",
        &perm.always_deny_rules,
        &mut lines,
    );
    render_rules("Always ask", &perm.always_ask_rules, &mut lines);
    render_rules("Always allow", &perm.always_allow_rules, &mut lines);

    // Session grants — separate section so users can audit transient state.
    lines.push(String::new());
    lines.push("  Session grants:".into());
    if perm.session_allow_rules.is_empty() {
        lines.push("    (none — cleared on session end)".into());
    } else {
        for (source, rules) in &perm.session_allow_rules {
            for rule in rules {
                lines.push(format!("    {:<40} (from {}) [transient]", rule, source));
            }
        }
    }

    // Additional working directories.
    lines.push(String::new());
    lines.push("  Additional working directories:".into());
    if perm.additional_working_directories.is_empty() {
        lines.push("    (none)".into());
    } else {
        for (path, dir) in &perm.additional_working_directories {
            let ro = if dir.read_only { " [read-only]" } else { "" };
            lines.push(format!("    {}{}", path, ro));
        }
    }

    if perm.always_allow_rules.is_empty()
        && perm.always_deny_rules.is_empty()
        && perm.always_ask_rules.is_empty()
        && perm.session_allow_rules.is_empty()
    {
        lines.push(String::new());
        lines.push("  No custom permission rules configured.".into());
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

fn render_rules(
    title: &str,
    rules_by_source: &std::collections::HashMap<String, Vec<String>>,
    lines: &mut Vec<String>,
) {
    lines.push(String::new());
    lines.push(format!("  {}:", title));
    if rules_by_source.is_empty() {
        lines.push("    (none)".into());
        return;
    }
    // Sort sources for stable output.
    let mut sorted: Vec<(&String, &Vec<String>)> = rules_by_source.iter().collect();
    sorted.sort_by_key(|(k, _)| (*k).clone());
    for (source, rules) in sorted {
        for rule in rules {
            lines.push(format!("    {:<40} (from {})", rule, source));
        }
    }
}

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

fn handle_mode(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(format!(
            "Current mode: {}\nUsage: /permissions mode <default|auto|bypass|plan|acceptEdits|dontAsk>",
            ctx.app_state.tool_permission_context.mode.as_str(),
        )));
    }

    let requested = PermissionMode::parse(parts[0]);
    if matches!(requested, PermissionMode::Default) && !parts[0].eq_ignore_ascii_case("default") {
        return Ok(CommandResult::Output(format!(
            "Unknown permission mode: '{}'.\nAvailable: default, auto, bypass, plan, acceptEdits, dontAsk",
            parts[0]
        )));
    }

    if matches!(requested, PermissionMode::Bypass)
        && !ctx
            .app_state
            .tool_permission_context
            .is_bypass_permissions_mode_available
    {
        return Ok(CommandResult::Output(
            "Bypass mode is disabled by configuration (permissions.enableBypassMode=false).".into(),
        ));
    }

    if matches!(requested, PermissionMode::Auto)
        && ctx.app_state.tool_permission_context.is_auto_mode_available == Some(false)
    {
        return Ok(CommandResult::Output(
            "Auto mode is disabled by configuration (permissions.enableAutoMode=false).".into(),
        ));
    }

    ctx.app_state.tool_permission_context.mode = requested.clone();
    Ok(CommandResult::Output(format!(
        "Permission mode set to: {}",
        requested.as_str(),
    )))
}

// ---------------------------------------------------------------------------
// Add (allow/ask/deny) with scope persistence
// ---------------------------------------------------------------------------

fn handle_add(kind: RuleKind, parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let mut parts: Vec<&str> = parts.to_vec();
    let scope = PersistScope::parse(&mut parts);

    if parts.is_empty() {
        return Ok(CommandResult::Output(format!(
            "Usage: /permissions {} <rule> [--user|--project|--local|--session]",
            kind.label()
        )));
    }
    let rule = parts.join(" ");

    // 1. Mutate in-memory state so the rule applies immediately.
    let bucket = match (kind, scope) {
        (RuleKind::Allow, PersistScope::Session) => {
            &mut ctx.app_state.tool_permission_context.session_allow_rules
        }
        (RuleKind::Ask, PersistScope::Session) | (RuleKind::Deny, PersistScope::Session) => {
            return Ok(CommandResult::Output(
                "Session scope only supports allow grants. Use --user/--project/--local for ask/deny."
                    .into(),
            ));
        }
        (RuleKind::Allow, _) => &mut ctx.app_state.tool_permission_context.always_allow_rules,
        (RuleKind::Ask, _) => &mut ctx.app_state.tool_permission_context.always_ask_rules,
        (RuleKind::Deny, _) => &mut ctx.app_state.tool_permission_context.always_deny_rules,
    };
    bucket
        .entry(scope.source_label().to_string())
        .or_default()
        .push(rule.clone());

    // 2. Persist to disk for non-session scopes.
    let persist_msg = match scope {
        PersistScope::Session => "(session-only — not persisted)".to_string(),
        other => persist_rule(kind, other, &rule, &ctx.cwd)?,
    };

    Ok(CommandResult::Output(format!(
        "{} rule '{}' added (scope={}). {}",
        kind.label(),
        rule,
        scope.source_label(),
        persist_msg,
    )))
}

fn persist_rule(
    kind: RuleKind,
    scope: PersistScope,
    rule: &str,
    cwd: &std::path::Path,
) -> Result<String> {
    let path = match scope {
        PersistScope::User => settings::user_settings_path(),
        PersistScope::Project => settings::project_settings_path(cwd),
        PersistScope::Local => settings::local_settings_path(cwd),
        PersistScope::Session => unreachable!(),
    };
    let mut raw = if path.exists() {
        let txt = std::fs::read_to_string(&path)?;
        serde_json::from_str::<settings::RawSettings>(&txt)?
    } else {
        settings::RawSettings::default()
    };
    let mut perms = raw.permissions.clone().unwrap_or_default();
    match kind {
        RuleKind::Allow => perms.allow.push(rule.to_string()),
        RuleKind::Ask => perms.ask.push(rule.to_string()),
        RuleKind::Deny => perms.deny.push(rule.to_string()),
    }
    raw.permissions = Some(perms);

    let written = match scope {
        PersistScope::User => settings::write_user_settings(&raw)?,
        PersistScope::Project => settings::write_project_settings(cwd, &raw)?,
        PersistScope::Local => settings::write_local_settings(cwd, &raw)?,
        PersistScope::Session => unreachable!(),
    };
    Ok(format!("→ persisted to {}", written.display()))
}

// ---------------------------------------------------------------------------
// Session grants
// ---------------------------------------------------------------------------

fn handle_session_grant(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /permissions session-grant <tool_name>".into(),
        ));
    }
    let tool = parts[0];
    ctx.app_state
        .tool_permission_context
        .grant_session_allow(tool);
    Ok(CommandResult::Output(format!(
        "Session grant added for '{}' (transient — cleared on session end).",
        tool,
    )))
}

fn handle_clear_session(ctx: &mut CommandContext) -> Result<CommandResult> {
    ctx.app_state.tool_permission_context.clear_session_grants();
    Ok(CommandResult::Output("Session grants cleared.".into()))
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

fn handle_reset(ctx: &mut CommandContext) -> Result<CommandResult> {
    let default = crate::types::app_state::AppState::default();
    ctx.app_state.tool_permission_context = default.tool_permission_context;
    Ok(CommandResult::Output(
        "Permission rules reset to defaults (in-memory only — files on disk untouched).".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        test_ctx_with_cwd(PathBuf::from("."))
    }

    fn test_ctx_with_cwd(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_permissions_show_includes_sections() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("Permission settings"));
        assert!(text.contains("Mode:"));
        assert!(text.contains("Always deny"));
        assert!(text.contains("Always ask"));
        assert!(text.contains("Always allow"));
        assert!(text.contains("Session grants"));
        assert!(text.contains("Additional working directories"));
    }

    #[tokio::test]
    async fn test_permissions_mode_accept_edits() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("mode acceptEdits", &mut ctx).await.unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("acceptEdits"));
        assert_eq!(
            ctx.app_state.tool_permission_context.mode,
            PermissionMode::AcceptEdits
        );
    }

    #[tokio::test]
    async fn test_permissions_mode_dont_ask() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("mode dontAsk", &mut ctx).await.unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("dontAsk"));
        assert_eq!(
            ctx.app_state.tool_permission_context.mode,
            PermissionMode::DontAsk
        );
    }

    #[tokio::test]
    async fn test_permissions_session_grant_and_clear() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        handler
            .execute("session-grant Bash", &mut ctx)
            .await
            .unwrap();
        assert!(ctx
            .app_state
            .tool_permission_context
            .has_session_grant("Bash"));

        handler
            .execute("clear-session-grants", &mut ctx)
            .await
            .unwrap();
        assert!(!ctx
            .app_state
            .tool_permission_context
            .has_session_grant("Bash"));
    }

    #[tokio::test]
    async fn test_permissions_ask_rule_session_rejected() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("ask Bash --session", &mut ctx)
            .await
            .unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("Session scope"));
    }

    #[tokio::test]
    async fn test_permissions_allow_user_persist() {
        // Use a tempdir as CC_RUST_HOME so /permissions allow doesn't
        // touch the developer's real settings file.
        let dir = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", dir.path().to_str().unwrap());

        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("allow Bash(prefix:git)", &mut ctx)
            .await
            .unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("allow rule"));
        assert!(text.contains("persisted"));
        assert!(dir.path().join("settings.json").exists());
    }

    #[tokio::test]
    async fn test_permissions_allow_project_from_subdir_preserves_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().join("workspace");
        let nested = project_root.join("src").join("nested");
        let project_dir = project_root.join(".cc-rust");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        let existing = serde_json::json!({
            "permissions": {
                "deny": ["Bash(rm)"],
                "allow": ["Read"]
            }
        });
        std::fs::write(
            project_dir.join("settings.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let handler = PermissionsHandler;
        let mut ctx = test_ctx_with_cwd(nested.clone());
        handler
            .execute("allow Bash(prefix:git) --project", &mut ctx)
            .await
            .unwrap();

        let written: settings::RawSettings = serde_json::from_str(
            &std::fs::read_to_string(project_dir.join("settings.json")).unwrap(),
        )
        .unwrap();
        let perms = written.permissions.expect("permissions present");
        assert!(perms.deny.iter().any(|rule| rule == "Bash(rm)"));
        assert!(perms.allow.iter().any(|rule| rule == "Read"));
        assert!(perms.allow.iter().any(|rule| rule == "Bash(prefix:git)"));
        assert!(!nested.join(".cc-rust").join("settings.json").exists());
    }

    #[tokio::test]
    async fn test_permissions_unknown_subcommand() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("Unknown permissions subcommand"));
    }

    /// Process-env guard for tests that mutate CC_RUST_HOME.
    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
