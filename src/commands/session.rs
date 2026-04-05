//! /session command -- list and show session information.
//!
//! Subcommands:
//! - `/session`          -- show the current session info
//! - `/session list`     -- list saved sessions
//!
//! The TypeScript version shows a remote session QR code via React.
//! In the Rust CLI we show a text listing of saved sessions instead.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::session::storage;

/// Handler for the `/session` slash command.
pub struct SessionHandler;

#[async_trait]
impl CommandHandler for SessionHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("list") | Some("ls") => handle_list(ctx),
            None => handle_show(ctx),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown session subcommand: '{}'\n\
                 Usage:\n  \
                   /session          -- show current session info\n  \
                   /session list     -- list saved sessions",
                sub
            ))),
        }
    }
}

/// Show information about the current session.
fn handle_show(ctx: &CommandContext) -> Result<CommandResult> {
    let msg_count = ctx.messages.len();
    let cwd = ctx.cwd.display();

    let mut lines = Vec::new();
    lines.push("Current session:".into());
    lines.push(String::new());
    lines.push(format!("  Working directory: {}", cwd));
    lines.push(format!("  Messages:          {}", msg_count));
    lines.push(format!("  Model:             {}", ctx.app_state.main_loop_model));

    Ok(CommandResult::Output(lines.join("\n")))
}

/// List saved sessions from disk.
fn handle_list(ctx: &CommandContext) -> Result<CommandResult> {
    let sessions = storage::list_sessions()?;

    if sessions.is_empty() {
        return Ok(CommandResult::Output(
            "No saved sessions found.".into(),
        ));
    }

    let mut lines = Vec::new();
    lines.push(format!("Saved sessions ({}):", sessions.len()));
    lines.push(String::new());

    // Header
    lines.push(format!(
        "  {:<38} {:>6}  {:<20}  {}",
        "Session ID", "Msgs", "Last Modified", "Directory"
    ));
    lines.push(format!(
        "  {:<38} {:>6}  {:<20}  {}",
        "----------", "----", "-------------", "---------"
    ));

    for session in sessions.iter().take(20) {
        let ts = chrono::DateTime::from_timestamp(session.last_modified, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".into());

        // Truncate long directory paths.
        let dir = if session.cwd.len() > 40 {
            format!("...{}", &session.cwd[session.cwd.len() - 37..])
        } else {
            session.cwd.clone()
        };

        lines.push(format!(
            "  {:<38} {:>6}  {:<20}  {}",
            session.session_id, session.message_count, ts, dir
        ));
    }

    if sessions.len() > 20 {
        lines.push(format!(
            "\n  ... and {} more sessions",
            sessions.len() - 20
        ));
    }

    lines.push(String::new());
    lines.push("Use /resume <session_id> to resume a session.".into());

    Ok(CommandResult::Output(lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::types::app_state::AppState;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
            session_id: "test-session".to_string(),
        }
    }

    #[tokio::test]
    async fn test_session_show() {
        let handler = SessionHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current session"));
                assert!(text.contains("Working directory"));
                assert!(text.contains("Messages"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_session_unknown_subcommand() {
        let handler = SessionHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown session subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
