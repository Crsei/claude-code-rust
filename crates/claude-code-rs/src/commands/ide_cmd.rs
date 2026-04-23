//! /ide command — IDE detection + selection + MCP bridge (issue #41).
//!
//! Subcommands:
//! - `/ide`            — show help.
//! - `/ide detect`     — run detection; print one row per IDE.
//! - `/ide status`     — show detected IDEs + current selection.
//! - `/ide select <id>`— persist selection; triggers a bridge reconnect.
//! - `/ide clear`      — remove the persisted selection.
//! - `/ide reconnect`  — re-establish the MCP bridge for the selected IDE.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::ide;
use crate::ipc::subsystem_types::IdeInfo;

/// Handler for the `/ide` slash command.
pub struct IdeHandler;

#[async_trait]
impl CommandHandler for IdeHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let sub = parts.next();
        let rest: Vec<&str> = parts.collect();

        match sub {
            None => Ok(CommandResult::Output(render_help())),
            Some("detect") => Ok(CommandResult::Output(render_list(
                &ide::detect_ides(),
                "IDE detection results",
            ))),
            Some("status") => Ok(CommandResult::Output(render_status())),
            Some("select") => handle_select(&rest),
            Some("clear") => handle_clear(),
            Some("reconnect") => handle_reconnect(),
            Some(other) => Ok(CommandResult::Output(format!(
                "Unknown ide subcommand: '{}'\n\n{}",
                other,
                render_help()
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

fn handle_select(rest: &[&str]) -> Result<CommandResult> {
    let Some(id) = rest.first().copied() else {
        return Ok(CommandResult::Output(
            "Usage: /ide select <id>\n\nRun `/ide detect` to see the available IDE ids."
                .to_string(),
        ));
    };
    match ide::select_ide(id) {
        Ok(()) => {
            let ides = ide::detect_ides();
            Ok(CommandResult::Output(render_list(
                &ides,
                &format!("Selected IDE: {}", id),
            )))
        }
        Err(e) => Ok(CommandResult::Output(format!(
            "Failed to select IDE: {}",
            e
        ))),
    }
}

fn handle_clear() -> Result<CommandResult> {
    match ide::clear_selection() {
        Ok(()) => Ok(CommandResult::Output("Cleared IDE selection.".to_string())),
        Err(e) => Ok(CommandResult::Output(format!(
            "Failed to clear IDE selection: {}",
            e
        ))),
    }
}

fn handle_reconnect() -> Result<CommandResult> {
    match ide::reconnect_selected() {
        Ok(()) => Ok(CommandResult::Output(
            "Scheduled an IDE MCP bridge reconnect.".to_string(),
        )),
        Err(e) => Ok(CommandResult::Output(format!("Reconnect failed: {}", e))),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_help() -> String {
    "IDE integration (MCP bridge) management.\n\n\
     Usage:\n  \
       /ide                   -- show this help\n  \
       /ide detect            -- run OS-level detection, print results\n  \
       /ide status            -- show detected IDEs + current selection\n  \
       /ide select <id>       -- persist selection and trigger bridge reconnect\n  \
       /ide clear             -- remove the persisted selection\n  \
       /ide reconnect         -- re-establish MCP bridge for the selected IDE\n\n\
     Supported IDEs: vscode, cursor, intellij, goland, pycharm, rubymine, webstorm.\n\n\
     Detection uses PATH lookups (e.g. `code`, `cursor`) and terminal env\n\
     vars (TERM_PROGRAM, VSCODE_PID, IDEA_INITIAL_DIRECTORY) to decide if\n\
     each IDE is installed and/or currently running.\n\n\
     Selection is persisted under `selectedIde` in\n\
     `{data_root}/settings.json` (usually `~/.cc-rust/settings.json`).\n"
        .to_string()
}

fn render_status() -> String {
    let ides = ide::detect_ides();
    let selected = ide::selected_ide();
    let heading = match &selected {
        Some(id) => format!("IDE status (selected: {}):", id),
        None => "IDE status (no selection):".to_string(),
    };
    render_list(&ides, &heading)
}

fn render_list(ides: &[IdeInfo], heading: &str) -> String {
    let mut lines = Vec::new();
    lines.push(heading.to_string());
    lines.push(String::new());

    if ides.is_empty() {
        lines.push("  (no IDEs known)".to_string());
        return lines.join("\n");
    }

    for info in ides {
        let installed = if info.installed { "yes" } else { "no " };
        let running = if info.running { "yes" } else { "no " };
        let marker = if info.selected { "*" } else { " " };
        lines.push(format!(
            "  {} {:<9} {:<24} installed={} running={}",
            marker, info.id, info.name, installed, running
        ));
    }
    lines.push(String::new());
    lines.push("  * = currently selected".to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

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

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn no_args_shows_help() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("IDE integration"));
                assert!(text.contains("/ide detect"));
                assert!(text.contains("/ide select"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn detect_lists_supported_ides() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("detect", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("IDE detection results"));
                assert!(text.contains("vscode"));
                assert!(text.contains("cursor"));
                assert!(text.contains("intellij"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn status_reports_no_selection_by_default() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("status", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("no selection"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn unknown_subcommand_returns_help_hint() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("wat", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown ide subcommand"));
                assert!(text.contains("/ide detect"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn select_without_id_shows_usage() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("select", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage: /ide select <id>"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn select_then_clear_round_trip() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();

        // Select vscode.
        let result = handler.execute("select vscode", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Selected IDE: vscode"));
            }
            _ => panic!("expected Output"),
        }
        assert_eq!(ide::selected_ide().as_deref(), Some("vscode"));

        // Clear.
        let result = handler.execute("clear", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Cleared IDE selection"));
            }
            _ => panic!("expected Output"),
        }
        assert!(ide::selected_ide().is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn select_unknown_id_reports_error() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler
            .execute("select nonexistent", &mut ctx)
            .await
            .unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("Failed to select IDE"),
                    "unexpected output: {}",
                    text
                );
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn reconnect_without_selection_reports_error() {
        let tmp = TempDir::new().unwrap();
        let _guard = HomeGuard::set(tmp.path());

        let handler = IdeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("reconnect", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Reconnect failed"));
            }
            _ => panic!("expected Output"),
        }
    }
}
