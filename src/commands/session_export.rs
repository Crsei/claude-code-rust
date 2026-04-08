//! /session-export command — export session as a structured JSON data package.
//!
//! Usage:
//!   /session-export              — export current session to ~/.cc-rust/exports/
//!   /session-export list         — list all session export files
//!   /session-export <path>       — export current session to a specific file
//!   /session-export <session_id> — export a saved session by ID

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::session::session_export;

pub struct SessionExportHandler;

#[async_trait]
impl CommandHandler for SessionExportHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() {
            return export_current(ctx);
        }

        if args == "list" {
            return list_exports();
        }

        // If it looks like a file path, export to that path
        if args.contains('/') || args.contains('\\') || args.ends_with(".json") {
            let path = std::path::Path::new(args);
            return export_current_to_path(ctx, path);
        }

        // Otherwise treat as session ID
        export_by_id(args)
    }
}

fn export_current(ctx: &CommandContext) -> Result<CommandResult> {
    let (path, export) = session_export::export_session(
        ctx.session_id.as_str(),
        &ctx.messages,
        &ctx.cwd.to_string_lossy(),
        None,
    )?;

    let summary = format_export_summary(&export);
    Ok(CommandResult::Output(format!(
        "Session exported to: {}\n\n{}",
        path.display(),
        summary
    )))
}

fn export_current_to_path(ctx: &CommandContext, path: &std::path::Path) -> Result<CommandResult> {
    let (path, _) = session_export::export_session(
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

fn list_exports() -> Result<CommandResult> {
    let files = session_export::list_session_exports()?;
    if files.is_empty() {
        return Ok(CommandResult::Output(
            "No session exports found. Use /session-export to export the current session.".into(),
        ));
    }
    let mut out = format!("Session exports ({}):\n", files.len());
    for f in &files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        out.push_str(&format!("  {}\n", name));
    }
    Ok(CommandResult::Output(out))
}

fn export_by_id(session_id: &str) -> Result<CommandResult> {
    let sessions = crate::session::storage::list_sessions()?;
    let matched = sessions
        .iter()
        .find(|s| s.session_id == session_id || s.session_id.starts_with(session_id));

    match matched {
        Some(info) => {
            let (path, _) = session_export::export_saved_session(&info.session_id, None)?;
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

/// Format a brief summary of the export for the user.
fn format_export_summary(export: &session_export::SessionExport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Messages: {}", export.transcript.message_count));
    lines.push(format!(
        "  User: {}, Assistant: {}, System: {}",
        export.transcript.user_message_count,
        export.transcript.assistant_message_count,
        export.transcript.system_message_count,
    ));
    lines.push(format!("Tool calls: {}", export.tool_calls.len()));
    if !export.context.unique_tools_used.is_empty() {
        lines.push(format!(
            "  Tools used: {}",
            export.context.unique_tools_used.join(", ")
        ));
    }
    lines.push(format!(
        "Compactions: {}",
        export.compression.total_compactions
    ));
    if !export.compression.content_replacements.is_empty() {
        lines.push(format!(
            "Content replacements: {}",
            export.compression.content_replacements.len()
        ));
    }
    lines.push(format!(
        "Tokens: ~{} / {} ({:.1}%)",
        export.context.estimated_total_tokens,
        export.context.context_window_size,
        export.context.utilization_pct,
    ));
    if export.context.total_cost_usd > 0.0 {
        lines.push(format!("Cost: ${:.4}", export.context.total_cost_usd));
    }
    lines.join("\n")
}
