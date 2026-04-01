//! `/memory` command — view and manage CLAUDE.md project instructions.
//!
//! Shows the current CLAUDE.md content, or opens it for editing.
//! Supports `show`, `edit`, and `path` subcommands.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;
use std::fs;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::claude_md;

pub struct MemoryHandler;

#[async_trait]
impl CommandHandler for MemoryHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcommand = args.trim().split_whitespace().next().unwrap_or("show");

        match subcommand {
            "show" | "" => show_memory(&ctx.cwd),
            "path" => show_paths(&ctx.cwd),
            "edit" => {
                let claude_md_path = ctx.cwd.join("CLAUDE.md");
                if !claude_md_path.exists() {
                    // Create a template
                    let template = "# CLAUDE.md\n\n\
                        This file provides guidance to Claude Code when working with code in this repository.\n\n\
                        ## Project Overview\n\n\
                        <!-- Describe your project here -->\n";
                    fs::write(&claude_md_path, template)?;
                }
                Ok(CommandResult::Output(format!(
                    "CLAUDE.md location: {}\n\nEdit this file to add project instructions.",
                    claude_md_path.display()
                )))
            }
            _ => Ok(CommandResult::Output(
                "Usage: /memory [show|path|edit]\n\n\
                 show  — Display current CLAUDE.md content (default)\n\
                 path  — Show CLAUDE.md file locations\n\
                 edit  — Create/locate CLAUDE.md for editing"
                    .to_string(),
            )),
        }
    }
}

fn show_memory(cwd: &std::path::Path) -> Result<CommandResult> {
    let context = claude_md::build_claude_md_context(cwd)?;

    if context.is_empty() {
        Ok(CommandResult::Output(
            "No CLAUDE.md found in project hierarchy.\n\n\
             Use `/memory edit` to create one."
                .to_string(),
        ))
    } else {
        Ok(CommandResult::Output(format!(
            "**Project Instructions (CLAUDE.md)**\n\n{}",
            context
        )))
    }
}

fn show_paths(cwd: &std::path::Path) -> Result<CommandResult> {
    let files = claude_md::find_claude_md_files(cwd);

    if files.is_empty() {
        Ok(CommandResult::Output(
            "No CLAUDE.md files found.".to_string(),
        ))
    } else {
        let mut lines = vec!["CLAUDE.md files found:".to_string()];
        for path in &files {
            let size = fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(0);
            lines.push(format!("  {} ({} bytes)", path.display(), size));
        }
        Ok(CommandResult::Output(lines.join("\n")))
    }
}
