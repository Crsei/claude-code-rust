//! Memory directory system — manages CLAUDE.md-based session memories.
//!
//! Provides reading, writing, and listing of memory entries stored alongside
//! session data. Memories are key-value pairs persisted as individual files
//! under `~/.cc-rust/memory/` (global) or `.cc-rust/memory/` (project-local).
//!
//! Corresponds to TypeScript: memdir/ (8 files)

#![allow(unused)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique key for this memory.
    pub key: String,
    /// The memory value / content.
    pub value: String,
    /// Category tag (e.g. "project", "preference", "context").
    #[serde(default)]
    pub category: String,
    /// When this entry was created (ISO 8601).
    pub created_at: String,
    /// When this entry was last updated (ISO 8601).
    pub updated_at: String,
}

/// Scope of memory storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    /// Global memories: `{data_root}/memory/`
    Global,
    /// Project-local memories: `.cc-rust/memory/` relative to cwd
    Project,
    /// Team-shared memories: `{data_root}/projects/{sanitized_cwd}/memory/team/`.
    /// Gated by `FEATURE_TEAMMEM`; the directory itself is readable/writable
    /// even when the feature is off so legacy data is never stranded.
    Team,
    /// Auto-captured memories: `{data_root}/auto_memory/`.
    /// Gated at the context-injection layer by the `auto_memory_enabled`
    /// toggle; the directory is always readable so prior captures can be
    /// inspected and purged.
    Auto,
}

impl MemoryScope {
    /// Short label used in selector output and JSON representations.
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryScope::Global => "global",
            MemoryScope::Project => "project",
            MemoryScope::Team => "team",
            MemoryScope::Auto => "auto",
        }
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Get the memory directory for a given scope.
pub fn memory_dir(scope: MemoryScope, cwd: &Path) -> Result<PathBuf> {
    match scope {
        MemoryScope::Global => Ok(cc_config::paths::memory_dir_global()),
        MemoryScope::Project => Ok(cwd.join(".cc-rust").join("memory")),
        MemoryScope::Team => Ok(cc_config::paths::team_memory_dir(cwd)),
        MemoryScope::Auto => Ok(cc_config::paths::auto_memory_dir()),
    }
}

/// Ensure the memory directory exists.
fn ensure_memory_dir(scope: MemoryScope, cwd: &Path) -> Result<PathBuf> {
    let dir = memory_dir(scope, cwd)?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create memory directory: {}", dir.display()))?;
    }
    Ok(dir)
}

/// Sanitize a key for use as a filename.
fn key_to_filename(key: &str) -> String {
    let sanitized: String = key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.json", sanitized)
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Write a memory entry.
pub fn write_memory(
    key: &str,
    value: &str,
    category: &str,
    scope: MemoryScope,
    cwd: &Path,
) -> Result<MemoryEntry> {
    let dir = ensure_memory_dir(scope, cwd)?;
    let filename = key_to_filename(key);
    let file_path = dir.join(&filename);

    let now = Utc::now().to_rfc3339();

    // Check if entry exists to preserve created_at
    let created_at = if file_path.exists() {
        read_memory(key, scope, cwd)
            .ok()
            .map(|e| e.created_at)
            .unwrap_or_else(|| now.clone())
    } else {
        now.clone()
    };

    let entry = MemoryEntry {
        key: key.to_string(),
        value: value.to_string(),
        category: category.to_string(),
        created_at,
        updated_at: now,
    };

    let json = serde_json::to_string_pretty(&entry).context("Failed to serialize memory entry")?;
    std::fs::write(&file_path, json)
        .with_context(|| format!("Failed to write memory file: {}", file_path.display()))?;

    Ok(entry)
}

/// Read a memory entry by key.
pub fn read_memory(key: &str, scope: MemoryScope, cwd: &Path) -> Result<MemoryEntry> {
    let dir = memory_dir(scope, cwd)?;
    let filename = key_to_filename(key);
    let file_path = dir.join(&filename);

    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Memory '{}' not found", key))?;

    serde_json::from_str(&content).context("Failed to parse memory entry")
}

/// Delete a memory entry.
pub fn delete_memory(key: &str, scope: MemoryScope, cwd: &Path) -> Result<bool> {
    let dir = memory_dir(scope, cwd)?;
    let filename = key_to_filename(key);
    let file_path = dir.join(&filename);

    if file_path.exists() {
        std::fs::remove_file(&file_path)
            .with_context(|| format!("Failed to delete memory: {}", file_path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// List all memory entries in a scope.
pub fn list_memories(scope: MemoryScope, cwd: &Path) -> Result<Vec<MemoryEntry>> {
    let dir = memory_dir(scope, cwd)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("Failed to read memory directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if let Ok(mem) = serde_json::from_str::<MemoryEntry>(&content) {
                    entries.push(mem);
                }
            }
            Err(_) => continue,
        }
    }

    // Sort by updated_at descending (most recent first)
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(entries)
}

/// Search memories by keyword across keys, values, and categories.
pub fn search_memories(query: &str, scope: MemoryScope, cwd: &Path) -> Result<Vec<MemoryEntry>> {
    let all = list_memories(scope, cwd)?;
    let query_lower = query.to_lowercase();

    Ok(all
        .into_iter()
        .filter(|e| {
            e.key.to_lowercase().contains(&query_lower)
                || e.value.to_lowercase().contains(&query_lower)
                || e.category.to_lowercase().contains(&query_lower)
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Context injection
// ---------------------------------------------------------------------------

/// Build a memory context string for injection into system prompts.
///
/// Collects relevant memories and formats them for the model's context.
/// Equivalent to `build_memory_context_with(cwd, false)` — the auto
/// scope is skipped unless the caller opts in.
pub fn build_memory_context(cwd: &Path) -> Result<String> {
    build_memory_context_with(cwd, false)
}

/// See [`build_memory_context`]. Extra `include_auto` flag lets the root
/// crate wire in the per-session `auto_memory_enabled` toggle without
/// dragging settings types into this crate.
///
/// Scopes included:
/// - `Project` and `Global` are always considered.
/// - `Team` is included when `FEATURE_TEAMMEM` is enabled.
/// - `Auto` is included when `include_auto` is true.
pub fn build_memory_context_with(cwd: &Path, include_auto: bool) -> Result<String> {
    let mut sections = Vec::new();

    // Collect project memories
    if let Ok(project_mems) = list_memories(MemoryScope::Project, cwd) {
        if !project_mems.is_empty() {
            let mut s = String::from("## Project Memories\n");
            for mem in &project_mems {
                s.push_str(&format!("- **{}**: {}\n", mem.key, mem.value));
            }
            sections.push(s);
        }
    }

    // Collect global memories
    if let Ok(global_mems) = list_memories(MemoryScope::Global, cwd) {
        if !global_mems.is_empty() {
            let mut s = String::from("## Global Memories\n");
            for mem in &global_mems {
                s.push_str(&format!("- **{}**: {}\n", mem.key, mem.value));
            }
            sections.push(s);
        }
    }

    // Team memories — gated on FEATURE_TEAMMEM at the context-injection
    // layer. The dir is readable regardless so the selector can still show
    // legacy entries even when the feature is off.
    if cc_config::features::enabled(cc_config::features::Feature::TeamMemory) {
        if let Ok(team_mems) = list_memories(MemoryScope::Team, cwd) {
            if !team_mems.is_empty() {
                let mut s = String::from("## Team Memories\n");
                for mem in &team_mems {
                    s.push_str(&format!("- **{}**: {}\n", mem.key, mem.value));
                }
                sections.push(s);
            }
        }
    }

    // Auto memories — injected only when the caller opts in via toggle.
    if include_auto {
        if let Ok(auto_mems) = list_memories(MemoryScope::Auto, cwd) {
            if !auto_mems.is_empty() {
                let mut s = String::from("## Auto Memories\n");
                for mem in &auto_mems {
                    s.push_str(&format!("- **{}**: {}\n", mem.key, mem.value));
                }
                sections.push(s);
            }
        }
    }

    if sections.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!(
            "<memory-context>\n{}</memory-context>",
            sections.join("\n")
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a unique temporary directory for testing.
    fn make_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-memdir-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Clean up a temp directory.
    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_key_to_filename() {
        assert_eq!(key_to_filename("simple"), "simple.json");
        assert_eq!(key_to_filename("my-key"), "my-key.json");
        assert_eq!(key_to_filename("has spaces"), "has_spaces.json");
        assert_eq!(key_to_filename("a/b\\c"), "a_b_c.json");
    }

    #[test]
    fn test_write_and_read_memory() {
        let cwd = make_temp_dir();

        let entry =
            write_memory("test-key", "test value", "test", MemoryScope::Project, &cwd).unwrap();
        assert_eq!(entry.key, "test-key");
        assert_eq!(entry.value, "test value");
        assert_eq!(entry.category, "test");

        let read = read_memory("test-key", MemoryScope::Project, &cwd).unwrap();
        assert_eq!(read.key, "test-key");
        assert_eq!(read.value, "test value");

        cleanup(&cwd);
    }

    #[test]
    fn test_update_preserves_created_at() {
        let cwd = make_temp_dir();

        let first = write_memory("key1", "value1", "cat", MemoryScope::Project, &cwd).unwrap();
        let created = first.created_at.clone();

        // Update the same key
        let second = write_memory("key1", "value2", "cat", MemoryScope::Project, &cwd).unwrap();
        assert_eq!(second.created_at, created);
        assert_eq!(second.value, "value2");

        cleanup(&cwd);
    }

    #[test]
    fn test_delete_memory() {
        let cwd = make_temp_dir();

        write_memory("to-delete", "val", "", MemoryScope::Project, &cwd).unwrap();
        assert!(delete_memory("to-delete", MemoryScope::Project, &cwd).unwrap());
        assert!(!delete_memory("to-delete", MemoryScope::Project, &cwd).unwrap());
        assert!(read_memory("to-delete", MemoryScope::Project, &cwd).is_err());

        cleanup(&cwd);
    }

    #[test]
    fn test_list_memories() {
        let cwd = make_temp_dir();

        write_memory("alpha", "val-a", "cat1", MemoryScope::Project, &cwd).unwrap();
        write_memory("beta", "val-b", "cat2", MemoryScope::Project, &cwd).unwrap();

        let all = list_memories(MemoryScope::Project, &cwd).unwrap();
        assert_eq!(all.len(), 2);

        cleanup(&cwd);
    }

    #[test]
    fn test_list_empty_dir() {
        let cwd = make_temp_dir();
        let all = list_memories(MemoryScope::Project, &cwd).unwrap();
        assert!(all.is_empty());
        cleanup(&cwd);
    }

    #[test]
    fn test_search_memories() {
        let cwd = make_temp_dir();

        write_memory(
            "rust-setup",
            "cargo build",
            "dev",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();
        write_memory(
            "python-env",
            "virtualenv",
            "dev",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();
        write_memory(
            "meeting-notes",
            "discussed rust",
            "notes",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let results = search_memories("rust", MemoryScope::Project, &cwd).unwrap();
        assert_eq!(results.len(), 2); // rust-setup + meeting-notes

        cleanup(&cwd);
    }

    #[test]
    fn test_build_memory_context_empty() {
        let cwd = make_temp_dir();
        let ctx = build_memory_context(&cwd).unwrap();
        assert!(ctx.is_empty());
        cleanup(&cwd);
    }

    #[test]
    fn test_build_memory_context_with_entries() {
        let cwd = make_temp_dir();

        write_memory("pref", "dark mode", "ui", MemoryScope::Project, &cwd).unwrap();

        let ctx = build_memory_context(&cwd).unwrap();
        assert!(ctx.contains("<memory-context>"));
        assert!(ctx.contains("pref"));
        assert!(ctx.contains("dark mode"));

        cleanup(&cwd);
    }

    /// Every `MemoryScope` variant resolves to a concrete path.
    /// Uses a `CC_RUST_HOME` override so tests don't touch real
    /// `~/.cc-rust/`.
    #[test]
    #[serial_test::serial]
    fn test_memory_dir_resolves_all_scopes() {
        let root = make_temp_dir();
        let previous = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", &root);

        let cwd = root.join("my_project");
        std::fs::create_dir_all(&cwd).unwrap();

        let global = memory_dir(MemoryScope::Global, &cwd).unwrap();
        assert_eq!(global, root.join("memory"));

        let project = memory_dir(MemoryScope::Project, &cwd).unwrap();
        assert_eq!(project, cwd.join(".cc-rust").join("memory"));

        let team = memory_dir(MemoryScope::Team, &cwd).unwrap();
        let s = team.to_string_lossy().replace('\\', "/");
        assert!(
            s.ends_with("/memory/team"),
            "unexpected team path: {}",
            team.display()
        );

        let auto = memory_dir(MemoryScope::Auto, &cwd).unwrap();
        assert_eq!(auto, root.join("auto_memory"));

        match previous {
            Some(v) => std::env::set_var("CC_RUST_HOME", v),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
        cleanup(&root);
    }

    /// Auto scope round-trip write/list/delete under a sandboxed
    /// `CC_RUST_HOME` so the real auto_memory/ is untouched.
    #[test]
    #[serial_test::serial]
    fn test_auto_scope_roundtrip() {
        let root = make_temp_dir();
        let previous = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", &root);

        let cwd = root.join("scratch");
        std::fs::create_dir_all(&cwd).unwrap();

        write_memory("auto-key", "captured note", "auto", MemoryScope::Auto, &cwd).unwrap();
        let all = list_memories(MemoryScope::Auto, &cwd).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].key, "auto-key");
        assert!(delete_memory("auto-key", MemoryScope::Auto, &cwd).unwrap());

        match previous {
            Some(v) => std::env::set_var("CC_RUST_HOME", v),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
        cleanup(&root);
    }

    #[test]
    fn test_scope_as_str_labels() {
        assert_eq!(MemoryScope::Global.as_str(), "global");
        assert_eq!(MemoryScope::Project.as_str(), "project");
        assert_eq!(MemoryScope::Team.as_str(), "team");
        assert_eq!(MemoryScope::Auto.as_str(), "auto");
    }
}
