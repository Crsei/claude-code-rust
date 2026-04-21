//! `/memory` command — default entry point is a memory selector that
//! surfaces auto-memory state and exposes every scope (global, project,
//! team, auto) together with the nearest CLAUDE.md files.
//!
//! Subcommands (issue #45):
//!   (no args)  — Print the selector (grouped listing of every scope +
//!                 auto-memory header + directory shortcuts)
//!   show       — Display CLAUDE.md content
//!   path       — Show CLAUDE.md file locations
//!   edit       — Create/locate CLAUDE.md for editing
//!   list       — List memdir entries across every scope
//!   get <key>  — Read a memdir entry
//!   set <key> <value> [--global|--team|--auto] [--category=<cat>]
//!   rm <key>   [--global|--team|--auto]
//!   search <query>
//!   auto on|off|status  — Toggle auto-memory capture/injection
//!   open  <auto|team|global|project>  — Print/ensure-and-open a dir
//!
//! # TODO
//! - The selector is currently a formatted listing; a real interactive TUI
//!   picker is a future improvement that belongs in the ink-terminal
//!   frontend, not here.
//! - The `auto` toggle only persists `auto_memory_enabled`; the actual
//!   capture hook is a separate change.

use anyhow::Result;
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::claude_md;
use crate::config::features::{self, Feature};
use crate::config::paths as cfg_paths;
use crate::config::settings;
use crate::session::memdir::{self, MemoryEntry, MemoryScope};

pub struct MemoryHandler;

#[async_trait]
impl CommandHandler for MemoryHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.trim().splitn(3, char::is_whitespace).collect();
        let subcommand = parts.first().copied().unwrap_or("");

        match subcommand {
            // Default entry point — selector view (issue #45).
            "" => selector(ctx),
            "show" => show_memory(&ctx.cwd),
            "path" => show_paths(&ctx.cwd),
            "edit" => edit_memory(&ctx.cwd),
            "list" | "ls" => list_entries(ctx),
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
                        "Usage: /memory set <key> <value> [--global|--team|--auto] [--category=<cat>]"
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
                        "Usage: /memory rm <key> [--global|--team|--auto]".to_string(),
                    ));
                }
                let scope = parse_scope_flag(flag);
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
            "auto" => {
                let action = parts.get(1).copied().unwrap_or("status");
                auto_toggle(action, ctx)
            }
            "open" => {
                let which = parts.get(1).copied().unwrap_or("");
                open_dir(which, &ctx.cwd)
            }
            _ => Ok(CommandResult::Output(
                "Usage: /memory [show|path|edit|list|get|set|rm|search|auto|open]\n\n\
                 (no args)           — Interactive memory selector (default)\n\n\
                 CLAUDE.md:\n\
                 \x20 show           — Display current CLAUDE.md content\n\
                 \x20 path           — Show CLAUDE.md file locations\n\
                 \x20 edit           — Create/locate CLAUDE.md for editing\n\n\
                 Memory entries:\n\
                 \x20 list                       — List entries across all scopes\n\
                 \x20 get <key>                  — Read an entry (searches all scopes)\n\
                 \x20 set <key> <val> [--global|--team|--auto] [--category=<cat>]\n\
                 \x20 rm <key> [--global|--team|--auto]\n\
                 \x20 search <query>             — Substring match across entries\n\n\
                 Auto-memory (issue #45):\n\
                 \x20 auto on|off|status        — Toggle auto-capture\n\
                 \x20 open <auto|team|global|project>\n\
                 \x20                             — Print/open a scope directory"
                    .to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Scope-flag parsing
// ---------------------------------------------------------------------------

fn parse_scope_flag(flag: &str) -> MemoryScope {
    if flag.contains("--global") {
        MemoryScope::Global
    } else if flag.contains("--team") {
        MemoryScope::Team
    } else if flag.contains("--auto") {
        MemoryScope::Auto
    } else {
        MemoryScope::Project
    }
}

// ---------------------------------------------------------------------------
// CLAUDE.md subcommands
// ---------------------------------------------------------------------------

fn show_memory(cwd: &Path) -> Result<CommandResult> {
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

fn show_paths(cwd: &Path) -> Result<CommandResult> {
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

fn edit_memory(cwd: &Path) -> Result<CommandResult> {
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
// Selector (default entry point — issue #45)
// ---------------------------------------------------------------------------

/// Build the default selector listing: auto-memory header + grouped entries
/// from every enabled scope + directory shortcuts.
///
/// Output format:
///   Memory selector
///   Auto-memory: OFF (enable via /memory auto on)
///
///   [1] [global] my_key — 2026-04-20
///   [2] [project] auth_notes — 2026-04-18
///   ...
///   [a] open auto-memory dir   — <path>
///   [t] open team-memory dir   — <path>
///   [g] open global memory dir — <path>
///   [p] open project memory dir— <path>
///
/// The true interactive TUI picker belongs in the ink-terminal frontend;
/// see the module-level TODO.
fn selector(ctx: &CommandContext) -> Result<CommandResult> {
    let cwd = &ctx.cwd;
    let auto_on = ctx.app_state.settings.auto_memory_enabled.unwrap_or(false);
    let team_gate = features::enabled(Feature::TeamMemory);

    let mut lines = Vec::new();
    lines.push("**Memory selector**".to_string());
    lines.push(format!(
        "Auto-memory: {} (toggle via /memory auto on|off)",
        if auto_on { "ON" } else { "OFF" }
    ));
    lines.push(String::new());

    // CLAUDE.md files at the top — unnumbered because they're content-only.
    let md_files = claude_md::find_claude_md_files(cwd);
    if !md_files.is_empty() {
        lines.push(format!("CLAUDE.md files ({}):", md_files.len()));
        for p in &md_files {
            lines.push(format!("  {}", p.display()));
        }
        lines.push(String::new());
    }

    // Enumerate memory entries, grouped by scope.
    let mut idx: usize = 0;
    let mut any_entries = false;

    let mut emit_group = |label: &str, entries: &[MemoryEntry], lines: &mut Vec<String>| {
        if entries.is_empty() {
            return;
        }
        any_entries = true;
        for e in entries {
            idx += 1;
            let date = e
                .updated_at
                .split('T')
                .next()
                .unwrap_or(&e.updated_at)
                .to_string();
            lines.push(format!("  [{}] [{}] {} — {}", idx, label, e.key, date));
        }
    };

    let global = memdir::list_memories(MemoryScope::Global, cwd).unwrap_or_default();
    let project = memdir::list_memories(MemoryScope::Project, cwd).unwrap_or_default();
    let team = memdir::list_memories(MemoryScope::Team, cwd).unwrap_or_default();
    let auto = memdir::list_memories(MemoryScope::Auto, cwd).unwrap_or_default();

    emit_group("global", &global, &mut lines);
    emit_group("project", &project, &mut lines);
    if team_gate || !team.is_empty() {
        // Show team entries even when the feature is gated off so legacy
        // data is never stranded — only injection into prompts is gated.
        emit_group("team", &team, &mut lines);
    }
    if auto_on || !auto.is_empty() {
        // Same principle: show auto entries even when the toggle is off
        // so users can inspect/purge past captures.
        emit_group("auto", &auto, &mut lines);
    }

    if !any_entries {
        lines.push("  (no memory entries — use `/memory set <key> <value>` to create one)".into());
    }

    lines.push(String::new());
    lines.push("Directory shortcuts:".into());
    lines.push(format!(
        "  [a] auto-memory dir    — {}",
        cfg_paths::auto_memory_dir().display()
    ));
    lines.push(format!(
        "  [t] team-memory dir    — {}",
        cfg_paths::team_memory_dir(cwd).display()
    ));
    lines.push(format!(
        "  [g] global memory dir  — {}",
        cfg_paths::memory_dir_global().display()
    ));
    lines.push(format!(
        "  [p] project memory dir — {}",
        cwd.join(".cc-rust").join("memory").display()
    ));
    lines.push(String::new());
    lines.push("Open a directory with `/memory open <auto|team|global|project>`.".into());

    Ok(CommandResult::Output(lines.join("\n")))
}

// ---------------------------------------------------------------------------
// Memdir subcommands
// ---------------------------------------------------------------------------

fn list_entries(ctx: &CommandContext) -> Result<CommandResult> {
    let cwd = &ctx.cwd;
    let project = memdir::list_memories(MemoryScope::Project, cwd).unwrap_or_default();
    let global = memdir::list_memories(MemoryScope::Global, cwd).unwrap_or_default();
    let team = memdir::list_memories(MemoryScope::Team, cwd).unwrap_or_default();
    let auto = memdir::list_memories(MemoryScope::Auto, cwd).unwrap_or_default();

    if project.is_empty() && global.is_empty() && team.is_empty() && auto.is_empty() {
        return Ok(CommandResult::Output(
            "No memory entries found.\n\nUse `/memory set <key> <value>` to create one."
                .to_string(),
        ));
    }

    let mut lines = Vec::new();
    append_group(&mut lines, "Project memories", &project);
    append_group(&mut lines, "Global memories", &global);
    if features::enabled(Feature::TeamMemory) || !team.is_empty() {
        append_group(&mut lines, "Team memories", &team);
    }
    let auto_on = ctx.app_state.settings.auto_memory_enabled.unwrap_or(false);
    if auto_on || !auto.is_empty() {
        append_group(&mut lines, "Auto memories", &auto);
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

fn append_group(lines: &mut Vec<String>, header: &str, entries: &[MemoryEntry]) {
    if entries.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!("**{}** ({})", header, entries.len()));
    for e in entries {
        let cat = if e.category.is_empty() {
            String::new()
        } else {
            format!(" [{}]", e.category)
        };
        lines.push(format!("  {} — {}{}", e.key, truncate(&e.value, 60), cat));
    }
}

fn get_entry(key: &str, cwd: &Path) -> Result<CommandResult> {
    // Search project → global → team → auto. First hit wins.
    for scope in [
        MemoryScope::Project,
        MemoryScope::Global,
        MemoryScope::Team,
        MemoryScope::Auto,
    ] {
        if let Ok(entry) = memdir::read_memory(key, scope, cwd) {
            return Ok(CommandResult::Output(format_entry(&entry, scope.as_str())));
        }
    }
    Ok(CommandResult::Output(format!(
        "Memory '{}' not found.",
        key
    )))
}

fn set_entry(key: &str, rest: &str, cwd: &Path) -> Result<CommandResult> {
    // Parse flags from the value string
    let mut value_parts = Vec::new();
    let mut scope = MemoryScope::Project;
    let mut category = String::new();

    for token in rest.split_whitespace() {
        match token {
            "--global" => scope = MemoryScope::Global,
            "--team" => scope = MemoryScope::Team,
            "--auto" => scope = MemoryScope::Auto,
            _ => {
                if let Some(cat) = token.strip_prefix("--category=") {
                    category = cat.to_string();
                } else {
                    value_parts.push(token);
                }
            }
        }
    }

    let value = value_parts.join(" ");
    if value.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /memory set <key> <value> [--global|--team|--auto] [--category=<cat>]"
                .to_string(),
        ));
    }

    let entry = memdir::write_memory(key, &value, &category, scope, cwd)?;
    Ok(CommandResult::Output(format!(
        "Saved {} memory '{}': {}",
        scope.as_str(),
        entry.key,
        entry.value
    )))
}

fn rm_entry(key: &str, scope: MemoryScope, cwd: &Path) -> Result<CommandResult> {
    let deleted = memdir::delete_memory(key, scope, cwd)?;
    if deleted {
        Ok(CommandResult::Output(format!(
            "Deleted {} memory '{}'.",
            scope.as_str(),
            key
        )))
    } else {
        Ok(CommandResult::Output(format!(
            "Memory '{}' not found in {} scope.",
            key,
            scope.as_str()
        )))
    }
}

fn search_entries(query: &str, cwd: &Path) -> Result<CommandResult> {
    let mut results = Vec::new();
    for scope in [
        MemoryScope::Project,
        MemoryScope::Global,
        MemoryScope::Team,
        MemoryScope::Auto,
    ] {
        let hits = memdir::search_memories(query, scope, cwd).unwrap_or_default();
        for entry in hits {
            results.push((scope, entry));
        }
    }

    if results.is_empty() {
        return Ok(CommandResult::Output(format!(
            "No memories matching '{}'.",
            query
        )));
    }

    let mut lines = vec![format!(
        "Found {} result(s) for '{}':",
        results.len(),
        query
    )];
    for (scope, e) in &results {
        let cat = if e.category.is_empty() {
            String::new()
        } else {
            format!(" [{}]", e.category)
        };
        lines.push(format!(
            "  [{}] {} — {}{}",
            scope.as_str(),
            e.key,
            truncate(&e.value, 60),
            cat
        ));
    }
    Ok(CommandResult::Output(lines.join("\n")))
}

// ---------------------------------------------------------------------------
// Auto-memory toggle (issue #45)
// ---------------------------------------------------------------------------

fn auto_toggle(action: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
    match action {
        "on" | "off" => {
            let on = action == "on";
            ctx.app_state.settings.auto_memory_enabled = Some(on);

            // Persist to the user-level settings.json so the toggle
            // survives restarts. A write failure surfaces in the output
            // but doesn't abort the session change.
            let persist_msg = persist_auto_memory(on);

            Ok(CommandResult::Output(format!(
                "Auto-memory: {}\n{}\n\nNote: the auto-capture hook is not yet wired — \
                 this toggle persists the setting only.",
                if on { "ON" } else { "OFF" },
                persist_msg
            )))
        }
        "status" | "" => {
            let on = ctx.app_state.settings.auto_memory_enabled.unwrap_or(false);
            Ok(CommandResult::Output(format!(
                "Auto-memory: {} (setting key: autoMemoryEnabled)",
                if on { "ON" } else { "OFF" }
            )))
        }
        _ => Ok(CommandResult::Output(
            "Usage: /memory auto [on|off|status]".into(),
        )),
    }
}

fn persist_auto_memory(on: bool) -> String {
    let path = settings::user_settings_path();
    // Load-or-default so we don't clobber other fields.
    let mut raw = settings::load_global_config().unwrap_or_default();
    raw.auto_memory_enabled = Some(on);
    match settings::write_settings_file(&path, &raw) {
        Ok(()) => format!("Persisted to {}", path.display()),
        Err(e) => format!(
            "Warning: could not persist setting to {}: {}",
            path.display(),
            e
        ),
    }
}

// ---------------------------------------------------------------------------
// /memory open <scope>
// ---------------------------------------------------------------------------

fn open_dir(which: &str, cwd: &Path) -> Result<CommandResult> {
    let (label, dir): (&str, PathBuf) = match which {
        "auto" => ("auto-memory", cfg_paths::auto_memory_dir()),
        "team" => ("team-memory", cfg_paths::team_memory_dir(cwd)),
        "global" => ("global memory", cfg_paths::memory_dir_global()),
        "project" => ("project memory", cwd.join(".cc-rust").join("memory")),
        "" => {
            return Ok(CommandResult::Output(
                "Usage: /memory open <auto|team|global|project>".into(),
            ));
        }
        other => {
            return Ok(CommandResult::Output(format!(
                "Unknown scope: '{}'. Use auto|team|global|project.",
                other
            )));
        }
    };

    // Ensure the directory exists so the path resolves to something
    // openable. We intentionally don't spawn an external editor — the UI
    // layer (or the user) picks the right opener.
    if let Err(e) = fs::create_dir_all(&dir) {
        return Ok(CommandResult::Output(format!(
            "Could not create {} dir {}: {}",
            label,
            dir.display(),
            e
        )));
    }

    Ok(CommandResult::Output(format!(
        "{} directory:\n  {}",
        label,
        dir.display()
    )))
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

    /// Default entry point (no args) is the selector, not `show`.
    /// This is the core UX change for issue #45.
    #[tokio::test]
    async fn test_memory_empty_args_is_selector() {
        let handler = MemoryHandler;
        let mut ctx = test_ctx(PathBuf::from("/nonexistent/fake/path"));
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("Memory selector"),
                    "expected selector header, got: {}",
                    text
                );
                assert!(
                    text.contains("Auto-memory:"),
                    "expected auto-memory header, got: {}",
                    text
                );
                assert!(
                    text.contains("Directory shortcuts"),
                    "expected directory shortcuts block, got: {}",
                    text
                );
            }
            _ => panic!("Expected Output"),
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
        let tmp =
            std::env::temp_dir().join(format!("cc_rust_mem_cmd_test_{}", uuid::Uuid::new_v4()));
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
        let tmp =
            std::env::temp_dir().join(format!("cc_rust_mem_cat_test_{}", uuid::Uuid::new_v4()));
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

    /// Auto-toggle updates the in-memory setting. We pin `CC_RUST_HOME`
    /// to a tempdir so the persistence side-effect lands there instead of
    /// the real `~/.cc-rust/settings.json`.
    #[tokio::test]
    async fn test_memory_auto_toggle_updates_state() {
        let root =
            std::env::temp_dir().join(format!("cc_rust_mem_auto_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();

        let previous = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", &root);

        let handler = MemoryHandler;
        let mut ctx = test_ctx(root.clone());
        assert_eq!(ctx.app_state.settings.auto_memory_enabled, None);

        // auto on
        let result = handler.execute("auto on", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("ON")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.settings.auto_memory_enabled, Some(true));

        // status
        let result = handler.execute("auto status", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("ON")),
            _ => panic!("Expected Output"),
        }

        // auto off
        let result = handler.execute("auto off", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("OFF")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(ctx.app_state.settings.auto_memory_enabled, Some(false));

        match previous {
            Some(v) => std::env::set_var("CC_RUST_HOME", v),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `/memory open` prints the path for each valid scope and rejects
    /// unknown scopes.
    #[tokio::test]
    async fn test_memory_open_scope_paths() {
        let tmp =
            std::env::temp_dir().join(format!("cc_rust_mem_open_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();

        let handler = MemoryHandler;
        let mut ctx = test_ctx(tmp.clone());

        for scope in ["auto", "team", "global", "project"] {
            let result = handler
                .execute(&format!("open {}", scope), &mut ctx)
                .await
                .unwrap();
            match &result {
                CommandResult::Output(text) => {
                    assert!(
                        text.contains("directory:"),
                        "expected directory line for {}, got: {}",
                        scope,
                        text
                    );
                }
                _ => panic!("Expected Output"),
            }
        }

        let result = handler.execute("open bogus", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Unknown scope")),
            _ => panic!("Expected Output"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_memory_selector_reflects_auto_state() {
        let tmp = std::env::temp_dir()
            .join(format!("cc_rust_mem_sel_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();

        let handler = MemoryHandler;
        let mut ctx = test_ctx(tmp.clone());

        // Default (None → OFF)
        let result = handler.execute("", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Auto-memory: OFF")),
            _ => panic!("Expected Output"),
        }

        // Flip directly in app_state to sidestep persistence.
        ctx.app_state.settings.auto_memory_enabled = Some(true);
        let result = handler.execute("", &mut ctx).await.unwrap();
        match &result {
            CommandResult::Output(text) => assert!(text.contains("Auto-memory: ON")),
            _ => panic!("Expected Output"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
