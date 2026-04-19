//! `/statusline` slash command — view, configure, test, or clear the
//! scriptable status-line command (issue #11).
//!
//! Usage:
//!
//! ```text
//!   /statusline                     show current config + last output
//!   /statusline set <cmd ...>       persist a new command (user scope)
//!   /statusline clear               clear `statusLine.command`
//!   /statusline enable | disable    toggle `statusLine.enabled`
//!   /statusline test [cmd]          run a one-shot with a synthetic payload
//!   /statusline payload             print the current payload as JSON
//!   /statusline refresh <ms>        set refreshIntervalMs
//!   /statusline timeout <ms>        set timeoutMs
//!   /statusline padding <n>         set padding (left spaces)
//! ```
//!
//! Persisted edits go to `~/.cc-rust/settings.json` (user scope). The next
//! TUI startup picks them up; within the current session the in-memory
//! `AppState.settings.status_line` snapshot is also updated so the runner
//! honours the change immediately.

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::settings::{self, RawSettings, StatusLineSettings};
use crate::ui::status_line::{
    ContextWindowStatus, CostStatus, ModelInfo, StatusLineOutput, StatusLinePayload,
    StatusLineRunner, WorkspaceStatus,
};

pub struct StatusLineHandler;

#[async_trait]
impl CommandHandler for StatusLineHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();
        let (sub, rest) = split_sub(trimmed);
        let sub = sub.to_ascii_lowercase();

        // Runner handle on AppState drives the live TUI. `/statusline
        // status` reads its counters; `test` deliberately uses a fresh
        // runner so it doesn't pollute the live one's last output.
        let runner = ctx.app_state.status_line_runner.clone();

        let result = match sub.as_str() {
            "" | "status" | "show" => render_status(&ctx.app_state.settings.status_line, &runner),
            "clear" | "remove" | "unset" => clear_command(ctx).await,
            "set" => set_command(rest, ctx).await,
            "enable" => toggle_enabled(Some(true), ctx).await,
            "disable" => toggle_enabled(Some(false), ctx).await,
            "test" => test_run(rest, ctx).await,
            "payload" => print_payload(ctx),
            "refresh" => set_u64_field("refreshIntervalMs", rest, ctx).await,
            "timeout" => set_u64_field("timeoutMs", rest, ctx).await,
            "padding" => set_padding(rest, ctx).await,
            other => Ok(usage(other)),
        };

        Ok(CommandResult::Output(match result {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        }))
    }
}

fn split_sub(args: &str) -> (&str, &str) {
    match args.find(char::is_whitespace) {
        Some(i) => (&args[..i], args[i..].trim_start()),
        None => (args, ""),
    }
}

fn usage(other: &str) -> String {
    format!(
        "Unknown /statusline subcommand '{}'.\n\nUsage:\n  \
         /statusline                    — show config + last output\n  \
         /statusline set <cmd ...>      — persist a new command\n  \
         /statusline clear              — remove the command\n  \
         /statusline enable|disable     — toggle without clearing\n  \
         /statusline test [cmd]         — one-shot with a synthetic payload\n  \
         /statusline payload            — print the current JSON payload\n  \
         /statusline refresh <ms>       — set refreshIntervalMs\n  \
         /statusline timeout <ms>       — set timeoutMs\n  \
         /statusline padding <n>        — left padding (spaces)",
        other
    )
}

fn render_status(s: &StatusLineSettings, runner: &StatusLineRunner) -> Result<String> {
    let mut out = String::new();
    out.push_str("Status line\n");
    out.push_str("───────────\n");

    let ty = s.r#type.as_deref().unwrap_or("(unset)");
    out.push_str(&format!("  type:              {}\n", ty));
    let cmd = s.runnable_command().unwrap_or("(none)");
    out.push_str(&format!("  command:           {}\n", cmd));
    out.push_str(&format!(
        "  enabled:           {}\n",
        match s.enabled {
            Some(true) => "yes",
            Some(false) => "no",
            None => "auto (enabled when command is set)",
        }
    ));
    out.push_str(&format!(
        "  padding:           {}\n",
        s.padding
            .map(|p| p.to_string())
            .unwrap_or_else(|| "0".into())
    ));
    out.push_str(&format!(
        "  refreshIntervalMs: {} (effective: {})\n",
        opt_display_u64(s.refresh_interval_ms),
        s.effective_refresh_ms()
    ));
    out.push_str(&format!(
        "  timeoutMs:         {} (effective: {})\n",
        opt_display_u64(s.timeout_ms),
        s.effective_timeout_ms()
    ));
    out.push_str(&format!(
        "  would run now:     {}\n",
        if s.is_command_mode() { "yes" } else { "no" }
    ));

    let (runs, errors) = runner.stats();
    out.push_str(&format!("  runs:              {}\n", runs));
    out.push_str(&format!("  errors:            {}\n", errors));
    let latest = runner.latest();
    if let Some(err) = &latest.error {
        out.push_str(&format!("  last error:        {}\n", err));
    }
    if !latest.stdout.is_empty() {
        out.push_str("\nLast output:\n");
        // Truncate to 3 lines; the full thing is available via `/statusline test`.
        for line in latest.lines(3) {
            out.push_str(&format!("  {}\n", line));
        }
    }

    out.push_str("\nConfig path: ");
    out.push_str(&settings::user_settings_path().display().to_string());
    out.push('\n');
    out.push_str("\nTip: `/statusline set echo 'demo'` to try, `/statusline clear` to remove.\n");
    Ok(out)
}

fn opt_display_u64(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "(unset)".into())
}

async fn set_command(rest: &str, ctx: &mut CommandContext) -> Result<String> {
    let cmd = rest.trim();
    if cmd.is_empty() {
        return Ok("Usage: /statusline set <command ...>".into());
    }
    let written = mutate_user_settings(|raw| {
        let sl = raw
            .status_line
            .get_or_insert_with(StatusLineSettings::default);
        sl.r#type = Some("command".into());
        sl.command = Some(cmd.to_string());
        if sl.enabled == Some(false) {
            sl.enabled = Some(true);
        }
    })?;
    // Sync the in-memory snapshot so the runner picks it up this turn.
    {
        let sl = &mut ctx.app_state.settings.status_line;
        sl.r#type = Some("command".into());
        sl.command = Some(cmd.to_string());
        if sl.enabled == Some(false) {
            sl.enabled = Some(true);
        }
    }
    Ok(format!(
        "Set statusLine.command = {}\n→ persisted to {}",
        cmd,
        written.display()
    ))
}

async fn clear_command(ctx: &mut CommandContext) -> Result<String> {
    let written = mutate_user_settings(|raw| {
        let sl = raw
            .status_line
            .get_or_insert_with(StatusLineSettings::default);
        sl.command = None;
        sl.script = None;
    })?;
    {
        let sl = &mut ctx.app_state.settings.status_line;
        sl.command = None;
        sl.script = None;
    }
    Ok(format!(
        "Cleared statusLine.command\n→ persisted to {}",
        written.display()
    ))
}

async fn toggle_enabled(enabled: Option<bool>, ctx: &mut CommandContext) -> Result<String> {
    let written = mutate_user_settings(|raw| {
        let sl = raw
            .status_line
            .get_or_insert_with(StatusLineSettings::default);
        sl.enabled = enabled;
    })?;
    ctx.app_state.settings.status_line.enabled = enabled;
    let state = match enabled {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "auto",
    };
    Ok(format!(
        "statusLine {} (persisted to {})",
        state,
        written.display()
    ))
}

async fn set_u64_field(field: &str, rest: &str, ctx: &mut CommandContext) -> Result<String> {
    let n: u64 = rest
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("expected a positive integer, got: {:?}", rest))?;
    let written = mutate_user_settings(|raw| {
        let sl = raw
            .status_line
            .get_or_insert_with(StatusLineSettings::default);
        match field {
            "refreshIntervalMs" => sl.refresh_interval_ms = Some(n),
            "timeoutMs" => sl.timeout_ms = Some(n),
            _ => {}
        }
    })?;
    {
        let sl = &mut ctx.app_state.settings.status_line;
        match field {
            "refreshIntervalMs" => sl.refresh_interval_ms = Some(n),
            "timeoutMs" => sl.timeout_ms = Some(n),
            _ => {}
        }
    }
    Ok(format!(
        "Set statusLine.{} = {}\n→ persisted to {}",
        field,
        n,
        written.display()
    ))
}

async fn set_padding(rest: &str, ctx: &mut CommandContext) -> Result<String> {
    let n: u16 = rest
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("expected a non-negative integer, got: {:?}", rest))?;
    let written = mutate_user_settings(|raw| {
        let sl = raw
            .status_line
            .get_or_insert_with(StatusLineSettings::default);
        sl.padding = Some(n);
    })?;
    ctx.app_state.settings.status_line.padding = Some(n);
    Ok(format!(
        "Set statusLine.padding = {}\n→ persisted to {}",
        n,
        written.display()
    ))
}

async fn test_run(rest: &str, ctx: &mut CommandContext) -> Result<String> {
    let override_cmd = rest.trim();
    // Compose a settings view: either the current one, or the override if
    // the user provided an inline command.
    let mut settings = ctx.app_state.settings.status_line.clone();
    if !override_cmd.is_empty() {
        settings.r#type = Some("command".into());
        settings.command = Some(override_cmd.to_string());
        settings.enabled = Some(true);
    }
    if !settings.is_command_mode() {
        return Ok(
            "No statusLine.command configured. Use `/statusline set <cmd ...>` first, or pass a \
             command inline: `/statusline test echo hello`."
                .into(),
        );
    }

    // Build a synthetic payload so tests don't depend on an active session.
    let payload = build_test_payload(ctx);

    // Use a brand-new runner so the test result doesn't pollute the TUI's
    // cached last-output.
    let runner = StatusLineRunner::new();
    let handle = runner
        .refresh(&settings, &payload)
        .ok_or_else(|| anyhow::anyhow!("runner refused to start — check config"))?;
    // Give the subprocess up to timeout + 500 ms to finish before we bail.
    let deadline = Duration::from_millis(settings.effective_timeout_ms() + 500);
    let joined = tokio::time::timeout(deadline, handle).await;
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(anyhow::anyhow!("runner task failed: {}", e)),
        Err(_) => return Err(anyhow::anyhow!("runner exceeded deadline")),
    }

    Ok(format_output(&runner.latest(), override_cmd))
}

fn format_output(out: &StatusLineOutput, override_cmd: &str) -> String {
    let mut s = String::new();
    if !override_cmd.is_empty() {
        s.push_str(&format!("Command: {}\n", override_cmd));
    }
    s.push_str("── stdout ───────────────────────\n");
    if out.stdout.is_empty() {
        s.push_str("(empty)\n");
    } else {
        s.push_str(&out.stdout);
        if !out.stdout.ends_with('\n') {
            s.push('\n');
        }
    }
    if let Some(e) = &out.error {
        s.push_str("── error ────────────────────────\n");
        s.push_str(e);
        s.push('\n');
    }
    s
}

fn print_payload(ctx: &CommandContext) -> Result<String> {
    let payload = build_test_payload(ctx);
    let json = serde_json::to_string_pretty(&payload)?;
    Ok(json)
}

fn build_test_payload(ctx: &CommandContext) -> StatusLinePayload {
    let mut p = StatusLinePayload::new();
    p.session_id = Some(ctx.session_id.to_string());
    let model_id = ctx.app_state.main_loop_model.clone();
    if !model_id.is_empty() {
        p.model = Some(ModelInfo {
            id: model_id,
            display_name: ctx
                .app_state
                .settings
                .model
                .as_ref()
                .and_then(|m| m.split('-').nth(1).map(|s| s.to_string())),
            backend: Some(ctx.app_state.main_loop_backend.clone()),
        });
    }
    p.workspace = Some(WorkspaceStatus {
        cwd: ctx.cwd.display().to_string(),
        ..Default::default()
    });
    p.context = Some(ContextWindowStatus::default());
    p.cost = Some(CostStatus::default());
    p.message_count = ctx.messages.len();
    p
}

fn mutate_user_settings<F>(mutate: F) -> Result<std::path::PathBuf>
where
    F: FnOnce(&mut RawSettings),
{
    // 1. Read current user settings (or empty default).
    let path = settings::user_settings_path();
    let mut raw: RawSettings = if path.exists() {
        let txt = std::fs::read_to_string(&path)?;
        serde_json::from_str(&txt)?
    } else {
        RawSettings::default()
    };
    mutate(&mut raw);
    settings::write_user_settings(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn status_subcommand_shows_config_path_and_defaults() {
        let handler = StatusLineHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Status line"));
                assert!(s.contains("Config path:"));
                assert!(s.contains("effective:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn payload_subcommand_emits_valid_json_with_event_name() {
        let handler = StatusLineHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("payload", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
                assert_eq!(
                    v.get("hookEventName").and_then(|x| x.as_str()),
                    Some("StatusLine")
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_emits_usage() {
        let handler = StatusLineHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("bogus", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /statusline"));
                assert!(s.contains("Usage:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn test_subcommand_without_config_explains_how_to_fix() {
        let handler = StatusLineHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("test", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(
                    s.contains("No statusLine.command"),
                    "expected hint, got: {}",
                    s
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn test_subcommand_runs_inline_command() {
        let handler = StatusLineHandler;
        let mut ctx = make_ctx();
        #[cfg(unix)]
        let cmd = "test echo statusline-ok";
        #[cfg(windows)]
        let cmd = "test echo statusline-ok";
        let r = handler.execute(cmd, &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(
                    s.contains("statusline-ok"),
                    "expected command output, got: {}",
                    s
                );
            }
            _ => panic!("expected Output"),
        }
    }
}
