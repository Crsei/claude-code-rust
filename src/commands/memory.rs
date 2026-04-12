//! `/memory` command — view and manage CLAUDE.md project instructions + memdir entries.
//!
//! Subcommands:
//!   show       — Display CLAUDE.md content (default)
//!   path       — Show CLAUDE.md file locations
//!   edit       — Create/locate CLAUDE.md for editing
//!   list       — List all memdir entries (project + global)
//!   get <key>  — Read a memdir entry
//!   set <key> <value> [--global] [--category=<cat>] — Write a memdir entry
//!   rm <key> [--global]  — Delete a memdir entry
//!   search <query>       — Search memdir entries

use anyhow::Result;
use async_trait::async_trait;
use std::fs;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::claude_md;
use crate::session::memdir::{self, MemoryScope};

pub struct MemoryHandler;

#[async_trait]
impl CommandHandler for MemoryHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.trim().splitn(3, char::is_whitespace).collect();
        let subcommand = parts.first().copied().unwrap_or("show");

        match subcommand {
            "" | "show" => show_memory(&ctx.cwd),
            "path" => show_paths(&ctx.cwd),
            "edit" => edit_memory(&ctx.cwd),
            "list" | "ls" => list_entries(&ctx.cwd),
            "get" => {
                let key = parts.get(1).unwrap_or(&"");
                if key.is_empty() {
                    return Ok(CommandResult::Output(
                        "Usage: /memory get <key>".to_string(),
                    ));
                }
                get_entry(key, &ctx.cwd)
            }
            "set" => {
                let key = parts.get(1).copied().unwrap_or("");
                let rest = parts.get(2).copied().unwrap_or("");
                if key.is_empty() || rest.is_empty() {
                    return Ok(CommandResult::Output(
                        "Usage: /memory set <key> <value> [--global] [--category=<cat>]"
                            .to_string(),
                    ));
                }
                set_entry(key, rest, &ctx.cwd)
            }
            "rm" | "delete" | "del" => {
                let key = parts.get(1).unwrap_or(&"");
                let flag = parts.get(2).copied().unwrap_or("");
                if key.is_empty() {
                    return Ok(CommandResult::Output(
                        "Usage: /memory rm <key> [--global]".to_string(),
                    ));
                }
                let scope = if flag.contains("--global") {
                    MemoryScope::Global
                } else {
                    MemoryScope::Project
                };
                rm_entry(key, scope, &ctx.cwd)
            }
            "search" | "find" => {
                let query = parts.get(1).unwrap_or(&"");
                if query.is_empty() {
                    return Ok(CommandResult::Output(
                        "Usage: /memory search <query>".to_string(),
                    ));
                }
                search_entries(query, &ctx.cwd)
            }
            _ => Ok(CommandResult::Output(
                "Usage: /memory [show|path|edit|list|get|set|rm|search]\n\n\
                 CLAUDE.md:\n\
                 \x20 show           — Display current CLAUDE.md content (default)\n\
                 \x20 path           — Show CLAUDE.md file locations\n\
                 \x20 edit           — Create/locate CLAUDE.md for editing\n\n\
                 Memory entries:\n\
                 \x20 list           — List all memory entries\n\
                 \x20 get <key>      — Read a memory entry\n\
                 \x20 set <key> <val> [--global] [--category=<cat>]\n\
                 \x20                — Write/update a memory entry\n\
                 \x20 rm <key> [--global]  — Delete a memory entry\n\
                 \x20 search <query> — Search memory entries"
                    .to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// CLAUDE.md subcommands (unchanged)
// ---------------------------------------------------------------------------

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
            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            lines.push(format!("  {} ({} bytes)", path.display(), size));
        }
        Ok(CommandResult::Output(lines.join("\n")))
    }
}

fn edit_memory(cwd: &std::path::Path) -> Result<CommandResult> {
    let claude_md_path = cwd.join("CLAUDE.md");
    if !claude_md_path.exists() {
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

// ---------------------------------------------------------------------------
// Memdir subcommands
// ---------------------------------------------------------------------------

fn list_entries(cwd: &std::path::Path) -> Result<CommandResult> {
    let project = memdir::list_memories(MemoryScope::Project, cwd).unwrap_or_default();
    let global = memdir::list_memories(MemoryScope::Global, cwd).unwrap_or_default();

    if project.is_empty() && global.is_empty() {
        return Ok(CommandResult::Output(
            "No memory entries found.\n\nUse `/memory set <key> <value>` to create one."
                .to_string(),
        ));
    }

    let mut lines = Vec::new();

    if !project.is_empty() {
        lines.push(format!("**Project memories** ({})", project.len()));
        for e in &project {
            let cat = if e.category.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.category)
            };
            lines.push(format!("  {} — {}{}", e.key, truncate(&e.value, 60), cat));
        }
    }

    if !global.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("**Global memories** ({})", global.len()));
        for e in &global {
            let cat = if e.category.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.category)
            };
            lines.push(format!("  {} — {}{}", e.key, truncate(&e.value, 60), cat));
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

fn get_entry(key: &str, cwd: &std::path::Path) -> Result<CommandResult> {
    // Try project first, then global
    if let Ok(entry) = memdir::read_memory(key, MemoryScope::Project, cwd) {
        return Ok(CommandResult::Output(format_entry(&entry, "project")));
    }
    if let Ok(entry) = memdir::read_memory(key, MemoryScope::Global, cwd) {
        return Ok(CommandResult::Output(format_entry(&entry, "global")));
    }
    Ok(CommandResult::Output(format!(
        "Memory '{}' not found.",
        key
    )))
}

fn set_entry(key: &str, rest: &str, cwd: &std::path::Path) -> Result<CommandResult> {
    // Parse flags from the value string
    let mut value_parts = Vec::new();
    let mut scope = MemoryScope::Project;
    let mut category = String::new();

    for token in rest.split_whitespace() {
        if token == "--global" {
            scope = MemoryScope::Global;
        } else if let Some(cat) = token.strip_prefix("--category=") {
            category = cat.to_string();
        } else {
            value_parts.push(token);
        }
    }

    let value = value_parts.join(" ");
    if value.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /memory set <key> <value> [--global] [--category=<cat>]".to_string(),
        ));
    }

    let scope_label = match scope {
        MemoryScope::Project => "project",
        MemoryScope::Global => "global",
    };

    let entry = memdir::write_memory(key, &value, &category, scope, cwd)?;
    Ok(CommandResult::Output(format!(
        "Saved {} memory '{}': {}",
        scope_label, entry.key, entry.value
    )))
}

fn rm_entry(
    key: &str,
    scope: MemoryScope,
    cwd: &std::path::Path,
) -> Result<CommandResult> {
    let deleted = memdir::delete_memory(key, scope, cwd)?;
    if deleted {
        Ok(CommandResult::Output(format!(
            "Deleted memory '{}'.",
            key
        )))
    } else {
        Ok(CommandResult::Output(format!(
            "Memory '{}' not found.",
            key
        )))
    }
}

fn search_entries(query: &str, cwd: &std::path::Path) -> Result<CommandResult> {
    let mut results = memdir::search_memories(query, MemoryScope::Project, cwd).unwrap_or_default();
    let global = memdir::search_memories(query, MemoryScope::Global, cwd).unwrap_or_default();
    results.extend(global);

    if results.is_empty() {
        return Ok(CommandResult::Output(format!(
            "No memories matching '{}'.",
            query
        )));
    }

    let mut lines = vec![format!("Found {} result(s) for '{}':", results.len(), query)];
    for e in &results {
        let cat = if e.category.is_empty() {
            String::new()
        } else {
            format!(" [{}]", e.category)
        };
        lines.push(format!("  {} — {}{}", e.key, truncate(&e.value, 60), cat));
    }
    Ok(CommandResult::Output(lines.join("\n")))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn format_entry(entry: &memdir::MemoryEntry, scope: &str) -> String {
    let mut lines = vec![
        format!("**{}** ({})", entry.key, scope),
        entry.value.clone(),
    ];
    if !entry.category.is_empty() {
        lines.push(format!("Category: {}", entry.category));
    }
    lines.push(format!("Created:  {}", entry.created_at));
    lines.push(format!("Updated:  {}", entry.updated_at));
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
    use std::path::PathBuf;

    fn test_ctx(cwd: PathBuf) -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd,
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_memory_show_nonexistent_dir_returns_output() {
        let handler = MemoryHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result = handler.execute("show", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(!text.is_empty());
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_memory_empty_args_defaults_to_show() {
        let handler = MemoryHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result_empty = handler.execute("", &mut ctx).await.unwrap();
        let result_show = handler.execute("show", &mut ctx).await.unwrap();
        match (result_empty, result_show) {
            (CommandResult::Output(a), CommandResult::Output(b)) => {
                assert_eq!(a, b, "empty args should equal 'show'");
            }
            _ => panic!("Expected Output for both"),
        }
    }

    #[tokio::test]
    async fn test_memory_unknown_subcommand_shows_usage() {
        let handler = MemoryHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result = handler.execute("unknown-subcmd", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage"), "expected usage info, got: {}", text);
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_memory_set_get_rm_roundtrip() {
        let tmp = std::env::temp_dir().join(format!(
            "cc_rust_mem_cmd_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let handler = MemoryHandler;
        let mut ctx = test_ctx(tmp.clone());

        // set
        let result = handler
            .execute("set my-key hello world", &mut ctx)
            .await
            .unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Saved")),
            _ => panic!("Expected Output"),
        }

        // get
        let result = handler.execute("get my-key", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => {
                assert!(text.contains("my-key"));
                assert!(text.contains("hello world"));
            }
            _ => panic!("Expected Output"),
        }

        // list
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("my-key")),
            _ => panic!("Expected Output"),
        }

        // search
        let result = handler.execute("search hello", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("my-key")),
            _ => panic!("Expected Output"),
        }

        // rm
        let result = handler.execute("rm my-key", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Deleted")),
            _ => panic!("Expected Output"),
        }

        // get again (should be not found)
        let result = handler.execute("get my-key", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("not found")),
            _ => panic!("Expected Output"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_memory_set_with_category() {
        let tmp = std::env::temp_dir().join(format!(
            "cc_rust_mem_cat_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let handler = MemoryHandler;
        let mut ctx = test_ctx(tmp.clone());

        let result = handler
            .execute("set pref dark-mode --category=ui", &mut ctx)
            .await
            .unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Saved")),
            _ => panic!("Expected Output"),
        }

        let result = handler.execute("get pref", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => {
                assert!(text.contains("ui"));
                assert!(text.contains("dark-mode"));
            }
            _ => panic!("Expected Output"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
