//! /config command -- show and modify configuration settings.
//!
//! Subcommands:
//! - `/config show`                       -- show effective settings + sources
//! - `/config show --raw`                 -- show every loaded layer separately
//! - `/config sources`                    -- print per-key origin map
//! - `/config schema`                     -- print JSON Schema for settings
//! - `/config set <key> <value> [--scope] -- set a value (default scope: user)
//! - `/config reset [--scope]`            -- reset specific layer or all to defaults
//!
//! Scope flags: `--user` (default), `--project`, `--local`.

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings::{self, RawSettings, SettingsSource};

/// Handler for the `/config` slash command.
pub struct ConfigHandler;

#[async_trait]
impl CommandHandler for ConfigHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("show") | None => handle_show(&parts[parts.first().map_or(0, |_| 1)..], ctx),
            Some("set") => handle_set(&parts[1..], ctx),
            Some("reset") => handle_reset(&parts[1..], ctx),
            Some("sources") => handle_sources(ctx),
            Some("schema") => handle_schema(),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown config subcommand: '{}'\n{}",
                sub,
                usage_text()
            ))),
        }
    }
}

fn usage_text() -> &'static str {
    "Usage:\n  \
       /config show [--raw]                  -- show effective config + sources\n  \
       /config sources                       -- list which layer set each key\n  \
       /config schema                        -- print JSON Schema for settings\n  \
       /config set <key> <value> [scope]     -- set a value (scope: --user|--project|--local)\n  \
       /config reset [scope]                 -- reset to defaults"
}

// ---------------------------------------------------------------------------
// /config show
// ---------------------------------------------------------------------------

fn handle_show(parts: &[&str], ctx: &CommandContext) -> Result<CommandResult> {
    let raw_mode = parts.iter().any(|p| *p == "--raw");
    if raw_mode {
        return handle_show_raw(ctx);
    }

    let state = &ctx.app_state;
    let mut lines = Vec::new();
    lines.push("Effective configuration:".into());
    lines.push(String::new());

    let src = |key: &str| -> String {
        state
            .settings
            .sources
            .get(key)
            .map(|s| format!("[{}]", s.as_str()))
            .unwrap_or_else(|| "[default]".into())
    };

    let row = |k: &str, v: String, src_key: &str, lines: &mut Vec<String>| {
        lines.push(format!("  {:<32} {:<10} {}", k, src(src_key), v));
    };

    row("model", state.main_loop_model.clone(), "model", &mut lines);
    row(
        "backend",
        state.main_loop_backend.clone(),
        "backend",
        &mut lines,
    );
    row(
        "theme",
        state
            .settings
            .theme
            .clone()
            .unwrap_or_else(|| "(default)".into()),
        "theme",
        &mut lines,
    );
    row("verbose", state.verbose.to_string(), "verbose", &mut lines);
    row(
        "permissionMode",
        state
            .settings
            .permission_mode
            .clone()
            .unwrap_or_else(|| format!("{:?}", state.tool_permission_context.mode).to_lowercase()),
        "permissionMode",
        &mut lines,
    );
    row(
        "permissions.allow",
        join_or_dash(&state.settings.permissions.allow),
        "permissions",
        &mut lines,
    );
    row(
        "permissions.deny",
        join_or_dash(&state.settings.permissions.deny),
        "permissions",
        &mut lines,
    );
    row(
        "permissions.ask",
        join_or_dash(&state.settings.permissions.ask),
        "permissions",
        &mut lines,
    );
    row(
        "sandbox.enabled",
        opt_str(state.settings.sandbox.enabled.map(|b| b.to_string())),
        "sandbox",
        &mut lines,
    );
    row(
        "statusLine.type",
        opt_str(state.settings.status_line.r#type.clone()),
        "statusLine",
        &mut lines,
    );
    row(
        "spinnerTips.enabled",
        opt_str(state.settings.spinner_tips.enabled.map(|b| b.to_string())),
        "spinnerTips",
        &mut lines,
    );
    row(
        "spinnerTips.intervalMs",
        opt_str(
            state
                .settings
                .spinner_tips
                .interval_ms
                .map(|n| n.to_string()),
        ),
        "spinnerTips",
        &mut lines,
    );
    row(
        "spinnerTips.customTips",
        join_or_dash(&state.settings.spinner_tips.custom_tips),
        "spinnerTips",
        &mut lines,
    );
    row(
        "outputStyle",
        opt_str(state.settings.output_style.clone()),
        "outputStyle",
        &mut lines,
    );
    row(
        "language",
        opt_str(state.settings.language.clone()),
        "language",
        &mut lines,
    );
    row(
        "voiceEnabled",
        opt_str(state.settings.voice_enabled.map(|b| b.to_string())),
        "voiceEnabled",
        &mut lines,
    );
    row(
        "editorMode",
        opt_str(state.settings.editor_mode.clone()),
        "editorMode",
        &mut lines,
    );
    row(
        "viewMode",
        opt_str(state.settings.view_mode.clone()),
        "viewMode",
        &mut lines,
    );
    row(
        "terminalProgressBarEnabled",
        opt_str(
            state
                .settings
                .terminal_progress_bar_enabled
                .map(|b| b.to_string()),
        ),
        "terminalProgressBarEnabled",
        &mut lines,
    );
    row(
        "availableModels",
        join_or_dash(&state.settings.available_models),
        "availableModels",
        &mut lines,
    );
    row(
        "effortLevel",
        opt_str(state.settings.effort_level.clone()),
        "effortLevel",
        &mut lines,
    );
    row(
        "fastMode",
        opt_str(state.settings.fast_mode.map(|b| b.to_string())),
        "fastMode",
        &mut lines,
    );
    row(
        "fastModePerSessionOptIn",
        opt_str(
            state
                .settings
                .fast_mode_per_session_opt_in
                .map(|b| b.to_string()),
        ),
        "fastModePerSessionOptIn",
        &mut lines,
    );
    row(
        "teammateMode",
        opt_str(state.settings.teammate_mode.map(|b| b.to_string())),
        "teammateMode",
        &mut lines,
    );
    row(
        "claudeInChromeDefaultEnabled",
        opt_str(
            state
                .settings
                .claude_in_chrome_default_enabled
                .map(|b| b.to_string()),
        ),
        "claudeInChromeDefaultEnabled",
        &mut lines,
    );

    lines.push(String::new());
    lines.push("File locations:".into());
    lines.push(format!(
        "  user:    {}",
        settings::user_settings_path().display()
    ));
    lines.push(format!(
        "  project: {}",
        settings::project_settings_path(&ctx.cwd).display()
    ));
    lines.push(format!(
        "  local:   {}",
        settings::local_settings_path(&ctx.cwd).display()
    ));
    lines.push(format!(
        "  managed: {}",
        settings::managed_settings_path().display()
    ));

    Ok(CommandResult::Output(lines.join("\n")))
}

fn opt_str(v: Option<String>) -> String {
    v.unwrap_or_else(|| "(unset)".into())
}

fn join_or_dash(v: &[String]) -> String {
    if v.is_empty() {
        "-".to_string()
    } else {
        v.join(", ")
    }
}

fn handle_show_raw(ctx: &CommandContext) -> Result<CommandResult> {
    let loaded = settings::load_effective(&ctx.cwd)?;
    let mut lines = Vec::new();
    lines.push("Raw settings layers (lowest -> highest priority):".into());
    lines.push(String::new());

    let render = |label: &str, raw: &Option<RawSettings>, lines: &mut Vec<String>| {
        lines.push(format!("== {} ==", label));
        match raw {
            None => lines.push("  (none)".into()),
            Some(r) => match serde_json::to_string_pretty(r) {
                Ok(json) => {
                    for l in json.lines() {
                        lines.push(format!("  {}", l));
                    }
                }
                Err(e) => lines.push(format!("  (failed to serialize: {})", e)),
            },
        }
        lines.push(String::new());
    };

    render("managed", &loaded.managed, &mut lines);
    render("user", &loaded.user, &mut lines);
    render("project", &loaded.project, &mut lines);
    render("local", &loaded.local, &mut lines);

    Ok(CommandResult::Output(lines.join("\n")))
}

// ---------------------------------------------------------------------------
// /config sources
// ---------------------------------------------------------------------------

fn handle_sources(ctx: &CommandContext) -> Result<CommandResult> {
    if ctx.app_state.settings.sources.is_empty() {
        return Ok(CommandResult::Output(
            "(no settings overrides recorded — every key is default)".into(),
        ));
    }
    let mut lines = vec!["Per-key sources:".into(), String::new()];
    for (k, v) in &ctx.app_state.settings.sources {
        lines.push(format!("  {:<36} {}", k, v.as_str()));
    }
    Ok(CommandResult::Output(lines.join("\n")))
}

// ---------------------------------------------------------------------------
// /config schema
// ---------------------------------------------------------------------------

fn handle_schema() -> Result<CommandResult> {
    let schema = settings::settings_schema();
    let pretty =
        serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "(failed to render)".into());
    Ok(CommandResult::Output(pretty))
}

// ---------------------------------------------------------------------------
// /config set
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum WriteScope {
    User,
    Project,
    Local,
}

fn parse_scope(parts: &mut Vec<&str>) -> WriteScope {
    let mut scope = WriteScope::User;
    parts.retain(|p| match *p {
        "--user" => {
            scope = WriteScope::User;
            false
        }
        "--project" => {
            scope = WriteScope::Project;
            false
        }
        "--local" => {
            scope = WriteScope::Local;
            false
        }
        _ => true,
    });
    scope
}

fn handle_set(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let mut parts: Vec<&str> = parts.to_vec();
    let scope = parse_scope(&mut parts);

    if parts.len() < 2 {
        return Ok(CommandResult::Output(format!(
            "Usage: /config set <key> <value> [--user|--project|--local]\n\n\
             Available keys: model, backend, theme, verbose, permissionMode,\n  \
               outputStyle, language, voiceEnabled, editorMode, viewMode,\n  \
               terminalProgressBarEnabled, effortLevel, fastMode,\n  \
               fastModePerSessionOptIn, teammateMode, claudeInChromeDefaultEnabled\n\n{}",
            usage_text()
        )));
    }

    let key = parts[0];
    let value = parts[1..].join(" ");

    // 1. Mutate the in-memory AppState so subsequent reads in this session
    //    see the new value.
    let in_memory_msg = apply_set_in_memory(key, &value, ctx)?;

    // 2. Persist to the chosen scope's file (with backup).
    let file_msg = persist_set(scope, key, &value, &ctx.cwd)?;

    // 3. Update the source map so /config sources reflects the change.
    let src = match scope {
        WriteScope::User => SettingsSource::User,
        WriteScope::Project => SettingsSource::Project,
        WriteScope::Local => SettingsSource::Local,
    };
    ctx.app_state.settings.sources.insert(key.to_string(), src);

    Ok(CommandResult::Output(format!(
        "{}\n{}",
        in_memory_msg, file_msg
    )))
}

/// Apply a key=value to the live AppState. Returns a user-facing message.
fn apply_set_in_memory(key: &str, value: &str, ctx: &mut CommandContext) -> Result<String> {
    let s = &mut ctx.app_state.settings;
    let parsed_bool = || value == "true" || value == "1";
    let parsed_bool_opt = || Some(parsed_bool());

    match key {
        "model" => {
            ctx.app_state.main_loop_model = value.to_string();
            s.model = Some(value.to_string());
            Ok(format!("Model set to: {}", value))
        }
        "backend" => {
            let normalized = crate::engine::codex_exec::normalize_backend(Some(value));
            ctx.app_state.main_loop_backend = normalized.clone();
            s.backend = Some(normalized.clone());
            Ok(format!("Backend set to: {}", normalized))
        }
        "theme" => {
            s.theme = Some(value.to_string());
            Ok(format!("Theme set to: {}", value))
        }
        "verbose" => {
            let v = parsed_bool();
            ctx.app_state.verbose = v;
            s.verbose = Some(v);
            Ok(format!("Verbose set to: {}", v))
        }
        "permissionMode" | "permission_mode" => {
            s.permission_mode = Some(value.to_string());
            s.permissions.default_mode = Some(value.to_string());
            ctx.app_state.tool_permission_context.mode =
                crate::types::tool::PermissionMode::parse(value);
            Ok(format!("Permission mode set to: {}", value))
        }
        "outputStyle" | "output_style" => {
            s.output_style = Some(value.to_string());
            Ok(format!("Output style set to: {}", value))
        }
        "language" => {
            s.language = Some(value.to_string());
            Ok(format!("Language set to: {}", value))
        }
        "voiceEnabled" | "voice_enabled" => {
            s.voice_enabled = parsed_bool_opt();
            Ok(format!("Voice enabled: {}", parsed_bool()))
        }
        "editorMode" | "editor_mode" => {
            s.editor_mode = Some(value.to_string());
            Ok(format!("Editor mode set to: {}", value))
        }
        "viewMode" | "view_mode" => {
            s.view_mode = Some(value.to_string());
            Ok(format!("View mode set to: {}", value))
        }
        "terminalProgressBarEnabled" | "terminal_progress_bar_enabled" => {
            s.terminal_progress_bar_enabled = parsed_bool_opt();
            Ok(format!("Terminal progress bar: {}", parsed_bool()))
        }
        "effortLevel" | "effort_level" => {
            s.effort_level = Some(value.to_string());
            ctx.app_state.effort_value = Some(value.to_string());
            Ok(format!("Effort level set to: {}", value))
        }
        "fastMode" | "fast_mode" => {
            let v = parsed_bool();
            s.fast_mode = Some(v);
            ctx.app_state.fast_mode = v;
            Ok(format!("Fast mode set to: {}", v))
        }
        "fastModePerSessionOptIn" | "fast_mode_per_session_opt_in" => {
            s.fast_mode_per_session_opt_in = parsed_bool_opt();
            Ok(format!("Fast mode per-session opt-in: {}", parsed_bool()))
        }
        "teammateMode" | "teammate_mode" => {
            s.teammate_mode = parsed_bool_opt();
            Ok(format!("Teammate mode: {}", parsed_bool()))
        }
        "claudeInChromeDefaultEnabled" | "claude_in_chrome_default_enabled" => {
            s.claude_in_chrome_default_enabled = parsed_bool_opt();
            Ok(format!("Claude-in-Chrome default: {}", parsed_bool()))
        }
        _ => anyhow::bail!(
            "Unknown config key: '{}'. Run `/config show` to see available keys.",
            key
        ),
    }
}

/// Persist a key=value into the given scope's settings.json (with backup).
fn persist_set(scope: WriteScope, key: &str, value: &str, cwd: &Path) -> Result<String> {
    // 1. Read current file content (or empty default).
    let path = scope_path(scope, cwd);
    let mut raw = if path.exists() {
        let txt = std::fs::read_to_string(&path)?;
        serde_json::from_str::<RawSettings>(&txt)?
    } else {
        RawSettings::default()
    };

    // 2. Patch the key in-place.
    apply_set_to_raw(&mut raw, key, value);

    // 3. Write back via the scope-specific helper (handles backups + dir
    //    creation + atomic rename).
    let written = match scope {
        WriteScope::User => settings::write_user_settings(&raw)?,
        WriteScope::Project => settings::write_project_settings(cwd, &raw)?,
        WriteScope::Local => settings::write_local_settings(cwd, &raw)?,
    };
    Ok(format!(
        "→ persisted to {} (backups kept)",
        written.display()
    ))
}

/// Compute the on-disk path for a scope without touching the filesystem.
fn scope_path(scope: WriteScope, cwd: &Path) -> std::path::PathBuf {
    match scope {
        WriteScope::User => settings::user_settings_path(),
        WriteScope::Project => settings::project_settings_path(cwd),
        WriteScope::Local => settings::local_settings_path(cwd),
    }
}

fn apply_set_to_raw(raw: &mut RawSettings, key: &str, value: &str) {
    let bool_val = || value == "true" || value == "1";
    let bool_opt = || Some(bool_val());

    match key {
        "model" => raw.model = Some(value.into()),
        "backend" => {
            raw.backend =
                Some(crate::engine::codex_exec::normalize_backend(Some(value)).to_string());
        }
        "theme" => raw.theme = Some(value.into()),
        "verbose" => raw.verbose = bool_opt(),
        "permissionMode" | "permission_mode" => {
            raw.permission_mode = Some(value.into());
            let mut perms = raw.permissions.take().unwrap_or_default();
            perms.default_mode = Some(value.into());
            raw.permissions = Some(perms);
        }
        "outputStyle" | "output_style" => raw.output_style = Some(value.into()),
        "language" => raw.language = Some(value.into()),
        "voiceEnabled" | "voice_enabled" => raw.voice_enabled = bool_opt(),
        "editorMode" | "editor_mode" => raw.editor_mode = Some(value.into()),
        "viewMode" | "view_mode" => raw.view_mode = Some(value.into()),
        "terminalProgressBarEnabled" | "terminal_progress_bar_enabled" => {
            raw.terminal_progress_bar_enabled = bool_opt();
        }
        "effortLevel" | "effort_level" => raw.effort_level = Some(value.into()),
        "fastMode" | "fast_mode" => raw.fast_mode = bool_opt(),
        "fastModePerSessionOptIn" | "fast_mode_per_session_opt_in" => {
            raw.fast_mode_per_session_opt_in = bool_opt();
        }
        "teammateMode" | "teammate_mode" => raw.teammate_mode = bool_opt(),
        "claudeInChromeDefaultEnabled" | "claude_in_chrome_default_enabled" => {
            raw.claude_in_chrome_default_enabled = bool_opt();
        }
        // Unknown keys get stuffed in `extra` so users can experiment with
        // future fields without losing data.
        _ => {
            raw.extra
                .insert(key.to_string(), serde_json::Value::String(value.into()));
        }
    }
}

// Small helper to allow `take()` on Option<PermissionsSettings> without
// pulling in extra crates. Using `Option::take` works directly above; this
// trait is intentionally *not* defined — we only need the inherent `take`.

// ---------------------------------------------------------------------------
// /config reset
// ---------------------------------------------------------------------------

fn handle_reset(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    let mut parts: Vec<&str> = parts.to_vec();
    let scope = if parts.is_empty() {
        None
    } else {
        Some(parse_scope(&mut parts))
    };

    // Reset in-memory state (always).
    let default_state = crate::types::app_state::AppState::default();
    ctx.app_state.settings = default_state.settings;
    ctx.app_state.main_loop_model = default_state.main_loop_model;
    ctx.app_state.main_loop_backend = default_state.main_loop_backend;
    ctx.app_state.verbose = default_state.verbose;
    ctx.app_state.fast_mode = default_state.fast_mode;
    ctx.app_state.effort_value = default_state.effort_value;
    ctx.app_state.thinking_enabled = default_state.thinking_enabled;

    // Optional: also rewrite a scope's file to defaults (backed up).
    if let Some(scope) = scope {
        let path = match scope {
            WriteScope::User => settings::user_settings_path(),
            WriteScope::Project => settings::project_settings_path(&ctx.cwd),
            WriteScope::Local => settings::local_settings_path(&ctx.cwd),
        };
        settings::write_settings_file(&path, &RawSettings::default())?;
        return Ok(CommandResult::Output(format!(
            "In-memory configuration reset; {} rewritten to defaults (backup kept).",
            path.display()
        )));
    }

    Ok(CommandResult::Output(
        "In-memory configuration reset to defaults. Files on disk untouched — \
         pass --user / --project / --local to also rewrite a file."
            .into(),
    ))
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

    fn test_ctx() -> CommandContext {
        test_ctx_with_cwd(PathBuf::from("/test/project"))
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
    async fn test_config_show() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("model"));
                assert!(text.contains("backend"));
                assert!(text.contains("permissionMode"));
                assert!(text.contains("File locations"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_config_set_model_in_memory() {
        // Use a tempdir as CC_RUST_HOME so we don't clobber the real user file.
        let dir = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", dir.path().to_str().unwrap());
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("set model claude-opus", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("claude-opus"));
                assert!(text.contains("persisted"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.main_loop_model, "claude-opus");
        assert!(dir.path().join("settings.json").exists());
    }

    #[tokio::test]
    async fn test_config_set_permission_mode_updates_live_context() {
        let dir = tempfile::tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", dir.path().to_str().unwrap());
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("set permissionMode dontAsk", &mut ctx)
            .await
            .unwrap();
        let CommandResult::Output(text) = result else {
            panic!("expected output")
        };
        assert!(text.contains("Permission mode set to: dontAsk"));
        assert_eq!(
            ctx.app_state.tool_permission_context.mode,
            crate::types::tool::PermissionMode::DontAsk
        );
    }

    #[tokio::test]
    async fn test_config_set_project_from_subdir_preserves_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().join("workspace");
        let nested = project_root.join("src").join("nested");
        let project_dir = project_root.join(".cc-rust");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        let existing = serde_json::json!({
            "model": "claude-opus",
            "verbose": true,
            "theme": "dark"
        });
        std::fs::write(
            project_dir.join("settings.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let handler = ConfigHandler;
        let mut ctx = test_ctx_with_cwd(nested.clone());
        handler
            .execute("set theme light --project", &mut ctx)
            .await
            .unwrap();

        let written: RawSettings = serde_json::from_str(
            &std::fs::read_to_string(project_dir.join("settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(written.model.as_deref(), Some("claude-opus"));
        assert_eq!(written.verbose, Some(true));
        assert_eq!(written.theme.as_deref(), Some("light"));
        assert!(!nested.join(".cc-rust").join("settings.json").exists());
    }

    #[tokio::test]
    async fn test_config_unknown_subcommand() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("delete", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown config subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_config_schema_emits_object() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("schema", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("\"properties\""));
                assert!(text.contains("\"permissions\""));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_config_sources_empty_default() {
        let handler = ConfigHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("sources", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("no settings overrides") || text.contains("Per-key sources"));
            }
            _ => panic!("Expected Output result"),
        }
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
