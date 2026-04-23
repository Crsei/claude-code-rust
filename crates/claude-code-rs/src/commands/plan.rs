//! `/plan` command — plan-mode switch + plan file access (issue #46).
//!
//! Subcommands:
//!   (no args) or `show`/`view` — Enter plan mode, then print the plan file.
//!   `open` / `edit`            — Open plan file in external editor.
//!   `path`                     — Print the plan file path.
//!
//! The no-arg entrypoint mirrors the `pre_plan_mode` save/restore handshake
//! used by `tools::plan_mode::EnterPlanModeTool`, so a subsequent `ExitPlanMode`
//! restores whatever mode the user was in before.
//!
//! # TODO (IPC/daemon sync)
//! `ipc::ingress::*` currently syncs only `additional_working_directories` and
//! `team_context` back to the engine after a command. Mode transitions driven
//! by `/plan` (like `/permissions mode`) only take effect in the TUI runtime
//! via `sync_app_runtime_from_state`. Wiring the daemon ingress path is a
//! follow-up.

use anyhow::Result;
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::paths as cfg_paths;
use crate::types::tool::PermissionMode;
use crate::ui::browser::{ensure_and_open, format_open_outcome};

/// Template seeded into a fresh plan file on first `open`/`edit`.
const PLAN_TEMPLATE: &str = "# Plan\n\n<!-- Draft your implementation plan here. -->\n";

pub struct PlanHandler;

#[async_trait]
impl CommandHandler for PlanHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.trim().splitn(2, char::is_whitespace);
        let sub = parts.next().unwrap_or("").trim();

        match sub {
            "" | "show" | "view" => enter_and_show(ctx),
            "open" | "edit" => open_plan(&ctx.cwd),
            "path" => Ok(CommandResult::Output(format!(
                "Plan file: {}",
                cfg_paths::current_plan_file_path(&ctx.cwd).display()
            ))),
            other => Ok(CommandResult::Output(format!(
                "Unknown subcommand: `{}`\n\n\
                 Usage:\n\
                 \x20 /plan              — Enter plan mode and show the plan file\n\
                 \x20 /plan show|view    — Show the plan file (same as no-arg)\n\
                 \x20 /plan open|edit    — Open plan file in $EDITOR\n\
                 \x20 /plan path         — Print the plan file path",
                other
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Subcommand implementations
// ---------------------------------------------------------------------------

/// Enter plan mode (saving `pre_plan_mode`) and render the plan-file body.
///
/// Mirrors `tools::plan_mode::EnterPlanModeTool::call` so `/plan` and the
/// tool-call path converge on the same state transition. Entering plan mode
/// twice is idempotent: the second call preserves the original `pre_plan_mode`
/// snapshot rather than overwriting it with `Plan`.
fn enter_and_show(ctx: &mut CommandContext) -> Result<CommandResult> {
    let perm = &mut ctx.app_state.tool_permission_context;
    let was_in_plan = matches!(perm.mode, PermissionMode::Plan);
    if !was_in_plan {
        perm.pre_plan_mode = Some(perm.mode.clone());
        perm.mode = PermissionMode::Plan;
    }

    let path = cfg_paths::current_plan_file_path(&ctx.cwd);
    let body = read_plan_body(&path);

    let header = if was_in_plan {
        format!("**Plan mode** (already active) — {}", path.display())
    } else {
        format!("**Plan mode** (entered) — {}", path.display())
    };

    Ok(CommandResult::Output(match body {
        Some(content) if !content.trim().is_empty() => format!("{header}\n\n{content}"),
        _ => format!("{header}\n\n(empty plan — use `/plan open` to draft one)"),
    }))
}

/// Ensure the plan file exists (seeded with `PLAN_TEMPLATE`) and hand off to
/// `$VISUAL` / `$EDITOR`. Prints a readable outcome.
fn open_plan(cwd: &Path) -> Result<CommandResult> {
    let path: PathBuf = cfg_paths::current_plan_file_path(cwd);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let outcome = ensure_and_open(&path, PLAN_TEMPLATE);
    Ok(CommandResult::Output(format_open_outcome(&outcome, &path)))
}

/// Return `Some(body)` if the plan file exists and is readable; `None`
/// otherwise. Read errors surface as `None` so the command degrades to the
/// empty-plan message instead of failing.
fn read_plan_body(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    fs::read_to_string(path).ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use crate::types::tool::PermissionMode;
    use serial_test::serial;
    use std::env;
    use tempfile::tempdir;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = env::var(key).ok();
            env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => env::set_var(self.key, v),
                None => env::remove_var(self.key),
            }
        }
    }

    fn make_ctx(cwd: PathBuf, start_mode: PermissionMode) -> CommandContext {
        let mut app_state = AppState::default();
        app_state.tool_permission_context.mode = start_mode;
        app_state.tool_permission_context.pre_plan_mode = None;
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state,
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    #[serial]
    async fn bare_plan_enters_plan_mode_and_shows_placeholder() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());
        let _editor = EnvGuard::unset("VISUAL");
        let _editor2 = EnvGuard::unset("EDITOR");

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::Default);
        let result = PlanHandler.execute("", &mut ctx).await.unwrap();

        assert!(matches!(
            ctx.app_state.tool_permission_context.mode,
            PermissionMode::Plan
        ));
        assert_eq!(
            ctx.app_state.tool_permission_context.pre_plan_mode,
            Some(PermissionMode::Default)
        );
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Plan mode"));
                assert!(s.contains("empty plan"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn plan_is_idempotent_and_preserves_prior_mode() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::AcceptEdits);
        PlanHandler.execute("", &mut ctx).await.unwrap();
        // Second call: mode is already Plan — pre_plan_mode must keep the
        // original AcceptEdits snapshot, not get overwritten by Plan.
        PlanHandler.execute("", &mut ctx).await.unwrap();

        assert_eq!(
            ctx.app_state.tool_permission_context.pre_plan_mode,
            Some(PermissionMode::AcceptEdits),
            "second /plan invocation must not clobber pre_plan_mode"
        );
    }

    #[tokio::test]
    #[serial]
    async fn show_existing_plan_renders_body() {
        let tmp = tempdir().unwrap();
        let plan_dir = tmp.path().join(".cc-rust");
        fs::create_dir_all(&plan_dir).unwrap();
        let plan_path = plan_dir.join("plan.md");
        fs::write(&plan_path, "# My Plan\n\n1. Step one\n").unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::Default);
        let result = PlanHandler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("My Plan"));
                assert!(s.contains("Step one"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn unknown_subcommand_prints_usage() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::Default);
        let result = PlanHandler.execute("bogus", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown subcommand"));
                assert!(s.contains("bogus"));
                assert!(s.contains("/plan open"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn path_subcommand_prints_resolved_path() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::Default);
        let result = PlanHandler.execute("path", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => assert!(s.contains("Plan file:")),
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn open_without_editor_creates_template_and_reports() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());
        // Force "no editor" path so the test is hermetic.
        let _v = EnvGuard::unset("VISUAL");
        let _e = EnvGuard::unset("EDITOR");

        let mut ctx = make_ctx(tmp.path().to_path_buf(), PermissionMode::Default);
        let result = PlanHandler.execute("open", &mut ctx).await.unwrap();

        // The plan file must exist now, seeded with the template header.
        let body = fs::read_to_string(cfg_paths::current_plan_file_path(tmp.path())).unwrap();
        assert!(body.starts_with("# Plan"));

        // Output should describe the outcome (either "editor" or "no editor").
        match result {
            CommandResult::Output(s) => assert!(!s.is_empty()),
            _ => panic!("expected Output"),
        }
    }
}
