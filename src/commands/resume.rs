//! /resume command -- resume a previous session.
//!
//! Usage:
//! - `/resume`            -- resume the most recent session for the current directory
//! - `/resume <id>`       -- resume a specific session by ID (or prefix)
//!
//! The TypeScript version uses an interactive React-based log selector.
//! In the Rust CLI we accept a session ID argument directly.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::session::resume as session_resume;
use crate::session::storage;

/// Handler for the `/resume` slash command.
pub struct ResumeHandler;

#[async_trait]
impl CommandHandler for ResumeHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let target = args.trim();

        if target.is_empty() {
            // Try to resume the most recent session for the current directory.
            return handle_resume_latest(ctx);
        }

        // Try exact match first, then prefix match.
        handle_resume_by_id(target, ctx)
    }
}

/// Resume the most recent session matching the current working directory.
fn handle_resume_latest(ctx: &mut CommandContext) -> Result<CommandResult> {
    match session_resume::get_last_session(&ctx.cwd)? {
        Some(info) => resume_session_by_id(&info.session_id, ctx),
        None => Ok(CommandResult::Output(format!(
            "No previous session found for directory: {}",
            ctx.cwd.display()
        ))),
    }
}

/// Resume a session by ID or prefix.
fn handle_resume_by_id(target: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
    let sessions = storage::list_sessions()?;

    // Exact match.
    if let Some(session) = sessions.iter().find(|s| s.session_id == target) {
        return resume_session_by_id(&session.session_id, ctx);
    }

    // Prefix match.
    let matches: Vec<_> = sessions
        .iter()
        .filter(|s| s.session_id.starts_with(target))
        .collect();

    match matches.len() {
        0 => Ok(CommandResult::Output(format!(
            "Session '{}' was not found.\n\
             Use /session list to see available sessions.",
            target
        ))),
        1 => resume_session_by_id(&matches[0].session_id, ctx),
        n => {
            let mut lines = Vec::new();
            lines.push(format!(
                "Found {} sessions matching '{}'. Please be more specific:",
                n, target
            ));
            lines.push(String::new());
            for s in matches.iter().take(10) {
                let ts = chrono::DateTime::from_timestamp(s.last_modified, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "unknown".into());
                lines.push(format!(
                    "  {} ({} msgs, {})",
                    s.session_id, s.message_count, ts
                ));
            }
            Ok(CommandResult::Output(lines.join("\n")))
        }
    }
}

/// Load a session's messages into the command context.
fn resume_session_by_id(session_id: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
    let messages = session_resume::resume_session(session_id)?;
    let msg_count = messages.len();
    ctx.messages = messages;

    Ok(CommandResult::Output(format!(
        "Resumed session {} ({} messages).",
        session_id, msg_count
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/nonexistent/test/path"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_resume_no_session() {
        let handler = ResumeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("No previous session")
                        || text.contains("not found")
                        || text.contains("session")
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_resume_nonexistent_id() {
        let handler = ResumeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("nonexistent-id-12345", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("not found"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
