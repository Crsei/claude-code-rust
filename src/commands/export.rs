//! /export command — export conversation to Markdown.
//!
//! Usage:
//!   /export                  — export current session to ~/.cc-rust/exports/
//!   /export list             — list all previously exported files
//!   /export <path>           — export current session to a specific file
//!   /export <session_id>     — export a saved session by ID

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::session::export;

pub struct ExportHandler;

#[async_trait]
impl CommandHandler for ExportHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() {
            // Export current session
            return export_current(ctx);
        }

        // /export list — show all previously exported files
        if args == "list" {
            return list_exported_files();
        }

        // Check if arg looks like a file path (contains / or \ or ends with .md)
        if args.contains('/') || args.contains('\\') || args.ends_with(".md") {
            let path = std::path::Path::new(args);
            return export_current_to_path(ctx, path);
        }

        // Otherwise treat as a session ID (or prefix)
        export_by_id(args, ctx)
    }
}

fn export_current(ctx: &CommandContext) -> Result<CommandResult> {
    let path = export::export_messages_markdown(
        ctx.session_id.as_str(),
        &ctx.messages,
        &ctx.cwd.to_string_lossy(),
        None,
    )?;
    Ok(CommandResult::Output(format!(
        "Session exported to: {}",
        path.display()
    )))
}

fn export_current_to_path(ctx: &CommandContext, path: &std::path::Path) -> Result<CommandResult> {
    let path = export::export_messages_markdown(
        ctx.session_id.as_str(),
        &ctx.messages,
        &ctx.cwd.to_string_lossy(),
        Some(path),
    )?;
    Ok(CommandResult::Output(format!(
        "Session exported to: {}",
        path.display()
    )))
}

fn list_exported_files() -> Result<CommandResult> {
    let files = export::list_exports()?;
    if files.is_empty() {
        return Ok(CommandResult::Output(
            "No exports found. Use /export to export the current session.".into(),
        ));
    }
    let mut out = format!("Exported sessions ({}):\n", files.len());
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        out.push_str(&format!("  {}\n", name));
    }
    Ok(CommandResult::Output(out))
}

fn export_by_id(session_id: &str, ctx: &CommandContext) -> Result<CommandResult> {
    // Try exact match first, then prefix match
    let sessions = crate::session::storage::list_sessions()?;
    let matched = sessions
        .iter()
        .find(|s| s.session_id == session_id || s.session_id.starts_with(session_id));

    match matched {
        Some(info) => {
            let path = export::export_session_markdown(&info.session_id, None)?;
            Ok(CommandResult::Output(format!(
                "Session {} exported to: {}",
                &info.session_id[..8],
                path.display()
            )))
        }
        None => Ok(CommandResult::Output(format!(
            "No session found matching '{}'. Use /session list to see available sessions.",
            session_id
        ))),
    }
}
