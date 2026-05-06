//! Memory directory system — manages CLAUDE.md-based session memories.
//!
//! Provides reading, writing, and listing of memory entries stored alongside
//! session data. Memories are key-value pairs persisted as individual files
//! under `~/.cc-rust/memory/` (global) or `.cc-rust/memory/` (project-local).
//!
//! Corresponds to TypeScript: memdir/ (8 files)

#![allow(unused)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

pub const MEMORY_ENTRYPOINT_NAME: &str = "MEMORY.md";
pub const MEMORY_ENTRYPOINT_MAX_LINES: usize = 200;
pub const MEMORY_ENTRYPOINT_MAX_BYTES: usize = 25_000;
const MEMORY_INDEX_HOOK_MAX_CHARS: usize = 150;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique key for this memory.
    pub key: String,
    /// The memory value / content.
    pub value: String,
    /// Category tag (e.g. "project", "preference", "context").
    #[serde(default)]
    pub category: String,
    /// Closed memory taxonomy aligned with Bun's user / feedback / project /
    /// reference types. Older entries may not have this field; in that case we
    /// infer it from `category` when possible.
    #[serde(
        default,
        rename = "type",
        alias = "memory_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub memory_type: Option<MemoryType>,
    /// Optional short description used by relevant-memory recall.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional search terms used by relevant-memory recall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_terms: Vec<String>,
    /// When this entry was created (ISO 8601).
    pub created_at: String,
    /// When this entry was last updated (ISO 8601).
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelevantMemory {
    pub identity: String,
    pub scope: MemoryScope,
    pub entry: MemoryEntry,
    pub score: u32,
    pub matched_terms: Vec<String>,
}

impl MemoryEntry {
    /// Effective closed taxonomy type, including legacy `category` fallback.
    pub fn effective_memory_type(&self) -> Option<MemoryType> {
        self.memory_type
            .or_else(|| MemoryType::parse(&self.category))
    }

    fn display_label(&self) -> Option<&str> {
        self.effective_memory_type()
            .map(MemoryType::as_str)
            .or_else(|| {
                let category = self.category.trim();
                (!category.is_empty()).then_some(category)
            })
    }
}

/// Bun-compatible closed memory taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryType {
    pub const ALL: [MemoryType; 4] = [
        MemoryType::User,
        MemoryType::Feedback,
        MemoryType::Project,
        MemoryType::Reference,
    ];

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "user" => Some(MemoryType::User),
            "feedback" => Some(MemoryType::Feedback),
            "project" => Some(MemoryType::Project),
            "reference" => Some(MemoryType::Reference),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MemoryType::User => "user",
            MemoryType::Feedback => "feedback",
            MemoryType::Project => "project",
            MemoryType::Reference => "reference",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            MemoryType::User => "User role, preferences, goals, responsibilities, or background.",
            MemoryType::Feedback => {
                "User guidance about behavior to avoid or repeat, including corrections and validated approaches."
            }
            MemoryType::Project => {
                "Project context, goals, deadlines, incidents, or motivations that cannot be inferred from code."
            }
            MemoryType::Reference => {
                "Pointers to external systems or resources where current information can be found."
            }
        }
    }
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

    fn context_title(self) -> &'static str {
        match self {
            MemoryScope::Global => "Global Memories",
            MemoryScope::Project => "Project Memories",
            MemoryScope::Team => "Team Memories",
            MemoryScope::Auto => "Auto Memories",
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

fn memory_entrypoint_path(scope: MemoryScope, cwd: &Path) -> Result<PathBuf> {
    Ok(memory_dir(scope, cwd)?.join(MEMORY_ENTRYPOINT_NAME))
}

fn one_line_hook(value: &str) -> String {
    let hook = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    truncate_chars(hook, MEMORY_INDEX_HOOK_MAX_CHARS)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut truncated = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            truncated.push_str("...");
            return truncated;
        }
        truncated.push(ch);
    }
    truncated
}

fn escape_markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn memory_index_line(entry: &MemoryEntry) -> String {
    let title = escape_markdown_link_text(&entry.key);
    let filename = key_to_filename(&entry.key);
    let hook = one_line_hook(&entry.value);

    if let Some(label) = entry.display_label() {
        format!("- [{title}]({filename}) - {label}: {hook}")
    } else {
        format!("- [{title}]({filename}) - {hook}")
    }
}

fn truncate_memory_index_content(content: &str) -> String {
    let mut output = String::new();
    let mut line_count = 0usize;
    let mut byte_count = 0usize;
    let mut truncated = false;

    for line in content.lines() {
        let separator_len = usize::from(!output.is_empty());
        let next_len = separator_len + line.len();

        if line_count >= MEMORY_ENTRYPOINT_MAX_LINES
            || byte_count + next_len > MEMORY_ENTRYPOINT_MAX_BYTES
        {
            truncated = true;
            break;
        }

        if separator_len == 1 {
            output.push('\n');
            byte_count += 1;
        }
        output.push_str(line);
        byte_count += line.len();
        line_count += 1;
    }

    if truncated {
        append_memory_index_warning(output)
    } else {
        output
    }
}

fn append_memory_index_warning(mut output: String) -> String {
    let warning = "- [truncated] MEMORY.md exceeded cc-rust index limits.";
    let separator_len = usize::from(!output.is_empty());
    let line_count = output.lines().count();

    if line_count < MEMORY_ENTRYPOINT_MAX_LINES
        && output.len() + separator_len + warning.len() <= MEMORY_ENTRYPOINT_MAX_BYTES
    {
        if separator_len == 1 {
            output.push('\n');
        }
        output.push_str(warning);
        return output;
    }

    if let Some(last_break) = output.rfind('\n') {
        let prefix = &output[..last_break];
        let candidate = if prefix.is_empty() {
            warning.to_string()
        } else {
            format!("{prefix}\n{warning}")
        };
        if candidate.lines().count() <= MEMORY_ENTRYPOINT_MAX_LINES
            && candidate.len() <= MEMORY_ENTRYPOINT_MAX_BYTES
        {
            return candidate;
        }
    }

    if warning.len() <= MEMORY_ENTRYPOINT_MAX_BYTES {
        warning.to_string()
    } else {
        output
    }
}

fn build_memory_index_from_entries(entries: &[MemoryEntry]) -> String {
    let lines = entries
        .iter()
        .map(memory_index_line)
        .collect::<Vec<_>>()
        .join("\n");
    truncate_memory_index_content(&lines)
}

pub fn build_memory_index(scope: MemoryScope, cwd: &Path) -> Result<String> {
    let entries = list_memories(scope, cwd)?;
    Ok(build_memory_index_from_entries(&entries))
}

pub fn read_memory_index(scope: MemoryScope, cwd: &Path) -> Result<Option<String>> {
    let path = memory_entrypoint_path(scope, cwd)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read memory index: {}", path.display()))?;
    Ok(Some(truncate_memory_index_content(&content)))
}

pub fn refresh_memory_index(scope: MemoryScope, cwd: &Path) -> Result<Option<PathBuf>> {
    let dir = ensure_memory_dir(scope, cwd)?;
    let path = dir.join(MEMORY_ENTRYPOINT_NAME);
    let entries = list_memories(scope, cwd)?;

    if entries.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove memory index: {}", path.display()))?;
        }
        return Ok(None);
    }

    let index = build_memory_index_from_entries(&entries);
    std::fs::write(&path, index)
        .with_context(|| format!("Failed to write memory index: {}", path.display()))?;
    Ok(Some(path))
}

fn format_memory_context_section(
    title: &str,
    scope: MemoryScope,
    cwd: &Path,
    memories: &[MemoryEntry],
) -> String {
    let mut section = format!("## {title}\n");

    let index = read_memory_index(scope, cwd)
        .ok()
        .flatten()
        .unwrap_or_else(|| build_memory_index_from_entries(memories));

    if !index.trim().is_empty() {
        section.push_str("### MEMORY.md Index\n");
        section.push_str(index.trim_end());
        section.push('\n');
    }

    for mem in memories {
        let label = mem
            .display_label()
            .map(|label| format!(" [{label}]"))
            .unwrap_or_default();
        section.push_str(&format!("- **{}**{}: {}\n", mem.key, label, mem.value));
    }

    section
}

fn memory_identity(scope: MemoryScope, entry: &MemoryEntry) -> String {
    format!("{}:{}", scope.as_str(), entry.key)
}

pub fn query_requests_memory_ignore(query: &str) -> bool {
    let normalized = query.to_ascii_lowercase();
    normalized.contains("ignore memory")
        || normalized.contains("ignore memories")
        || normalized.contains("do not use memory")
        || normalized.contains("don't use memory")
        || normalized.contains("dont use memory")
        || normalized.contains("without memory")
}

fn recall_scopes(include_auto: bool) -> Vec<MemoryScope> {
    let mut scopes = vec![MemoryScope::Project, MemoryScope::Global];
    if cc_config::features::enabled(cc_config::features::Feature::TeamMemory) {
        scopes.push(MemoryScope::Team);
    }
    if include_auto {
        scopes.push(MemoryScope::Auto);
    }
    scopes
}

fn tokenize_for_recall(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();

    for ch in value.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch.to_ascii_lowercase());
        } else if current.len() >= 3 {
            terms.push(current.clone());
            current.clear();
        } else {
            current.clear();
        }
    }

    if current.len() >= 3 {
        terms.push(current);
    }

    terms.sort();
    terms.dedup();
    terms
}

fn entry_recall_text(entry: &MemoryEntry) -> String {
    let mut text = format!("{} {} {}", entry.key, entry.category, entry.value);
    if let Some(memory_type) = entry.effective_memory_type() {
        text.push(' ');
        text.push_str(memory_type.as_str());
    }
    if let Some(description) = &entry.description {
        text.push(' ');
        text.push_str(description);
    }
    for term in &entry.search_terms {
        text.push(' ');
        text.push_str(term);
    }
    text
}

fn score_memory_for_query(entry: &MemoryEntry, query: &str) -> Option<(u32, Vec<String>)> {
    let query_terms = tokenize_for_recall(query);
    if query_terms.is_empty() {
        return None;
    }

    let key = entry.key.to_ascii_lowercase();
    let category = entry.category.to_ascii_lowercase();
    let value = entry.value.to_ascii_lowercase();
    let description = entry
        .description
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let search_terms = entry
        .search_terms
        .iter()
        .map(|term| term.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let memory_type = entry
        .effective_memory_type()
        .map(|memory_type| memory_type.as_str().to_string())
        .unwrap_or_default();

    let mut score = 0u32;
    let mut matched = Vec::new();
    for term in query_terms {
        let mut term_score = 0u32;
        if key.contains(&term) {
            term_score += 8;
        }
        if category.contains(&term) || memory_type.contains(&term) {
            term_score += 5;
        }
        if description.contains(&term) || search_terms.iter().any(|value| value.contains(&term)) {
            term_score += 4;
        }
        if value.contains(&term) {
            term_score += 2;
        }

        if term_score > 0 {
            score += term_score;
            matched.push(term);
        }
    }

    let query_lower = query.trim().to_ascii_lowercase();
    if query_lower.len() >= 8
        && entry_recall_text(entry)
            .to_ascii_lowercase()
            .contains(&query_lower)
    {
        score += 12;
    }

    (score > 0).then_some((score, matched))
}

fn is_generic_recent_tool_memory(entry: &MemoryEntry, recent_tool_names: &[String]) -> bool {
    if recent_tool_names.is_empty() {
        return false;
    }

    let text = entry_recall_text(entry).to_ascii_lowercase();
    let high_value_markers = [
        "avoid", "bug", "danger", "do not", "error", "failure", "issue", "known", "pitfall",
        "risk", "security", "warning",
    ];
    if high_value_markers
        .iter()
        .any(|marker| text.contains(marker))
    {
        return false;
    }

    let generic_markers = [
        "docs",
        "example",
        "examples",
        "guide",
        "how to",
        "manual",
        "reference",
        "syntax",
        "tool",
        "usage",
    ];
    let looks_generic = generic_markers.iter().any(|marker| text.contains(marker))
        || entry.effective_memory_type() == Some(MemoryType::Reference);
    if !looks_generic {
        return false;
    }

    recent_tool_names.iter().any(|tool| {
        let tool = tool.trim().to_ascii_lowercase();
        !tool.is_empty() && text.contains(&tool)
    })
}

/// Recall at most `max_results` memories relevant to the current query.
///
/// This deterministic path is the offline fallback for Bun-style relevant
/// memory recall. It scans all enabled scopes, scores key/category/type/value
/// plus optional description/search terms, skips generic docs for recently used
/// tools, and excludes session-surfaced identities.
pub fn recall_relevant_memories(
    cwd: &Path,
    include_auto: bool,
    query: &str,
    recent_tool_names: &[String],
    already_surfaced: &HashSet<String>,
    max_results: usize,
) -> Result<Vec<RelevantMemory>> {
    if max_results == 0 || query.trim().is_empty() || query_requests_memory_ignore(query) {
        return Ok(Vec::new());
    }

    let mut relevant = Vec::new();
    for scope in recall_scopes(include_auto) {
        for entry in list_memories(scope, cwd).unwrap_or_default() {
            let identity = memory_identity(scope, &entry);
            if already_surfaced.contains(&identity)
                || is_generic_recent_tool_memory(&entry, recent_tool_names)
            {
                continue;
            }

            let Some((score, matched_terms)) = score_memory_for_query(&entry, query) else {
                continue;
            };
            relevant.push(RelevantMemory {
                identity,
                scope,
                entry,
                score,
                matched_terms,
            });
        }
    }

    relevant.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.entry.updated_at.cmp(&a.entry.updated_at))
            .then_with(|| a.identity.cmp(&b.identity))
    });
    relevant.truncate(max_results);
    Ok(relevant)
}

fn format_relevant_memory_context(relevant: &[RelevantMemory]) -> String {
    let mut sections = Vec::new();
    for scope in recall_scopes(true) {
        let memories = relevant
            .iter()
            .filter(|memory| memory.scope == scope)
            .collect::<Vec<_>>();
        if memories.is_empty() {
            continue;
        }

        let mut section = format!("## {}\n", scope.context_title());
        for memory in memories {
            let label = memory
                .entry
                .display_label()
                .map(|label| format!(" [{label}]"))
                .unwrap_or_default();
            let matched = if memory.matched_terms.is_empty() {
                String::new()
            } else {
                format!(" matched: {}", memory.matched_terms.join(", "))
            };
            section.push_str(&format!(
                "- **{}**{} ({}; score {}{}): {}\n",
                memory.entry.key, label, memory.identity, memory.score, matched, memory.entry.value
            ));
        }
        sections.push(section);
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!(
            "<memory-context>\n## Relevant Memories\nUse these recalled memories only when they are relevant to the current request. If the user explicitly asks to ignore memory, do not use memory context.\n{}</memory-context>",
            sections.join("\n")
        )
    }
}

/// Build query-scoped memory context and return the surfaced identities.
pub fn build_relevant_memory_context_with(
    cwd: &Path,
    include_auto: bool,
    query: &str,
    recent_tool_names: &[String],
    already_surfaced: &HashSet<String>,
    max_results: usize,
) -> Result<(String, Vec<String>)> {
    let relevant = recall_relevant_memories(
        cwd,
        include_auto,
        query,
        recent_tool_names,
        already_surfaced,
        max_results,
    )?;
    let surfaced = relevant
        .iter()
        .map(|memory| memory.identity.clone())
        .collect::<Vec<_>>();
    Ok((format_relevant_memory_context(&relevant), surfaced))
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
        memory_type: MemoryType::parse(category),
        description: None,
        search_terms: Vec::new(),
        created_at,
        updated_at: now,
    };

    let json = serde_json::to_string_pretty(&entry).context("Failed to serialize memory entry")?;
    std::fs::write(&file_path, json)
        .with_context(|| format!("Failed to write memory file: {}", file_path.display()))?;

    refresh_memory_index(scope, cwd)?;

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
        refresh_memory_index(scope, cwd)?;
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
                || e.effective_memory_type()
                    .map(|memory_type| memory_type.as_str().contains(&query_lower))
                    .unwrap_or(false)
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
            sections.push(format_memory_context_section(
                "Project Memories",
                MemoryScope::Project,
                cwd,
                &project_mems,
            ));
        }
    }

    // Collect global memories
    if let Ok(global_mems) = list_memories(MemoryScope::Global, cwd) {
        if !global_mems.is_empty() {
            sections.push(format_memory_context_section(
                "Global Memories",
                MemoryScope::Global,
                cwd,
                &global_mems,
            ));
        }
    }

    // Team memories — gated on FEATURE_TEAMMEM at the context-injection
    // layer. The dir is readable regardless so the selector can still show
    // legacy entries even when the feature is off.
    if cc_config::features::enabled(cc_config::features::Feature::TeamMemory) {
        if let Ok(team_mems) = list_memories(MemoryScope::Team, cwd) {
            if !team_mems.is_empty() {
                sections.push(format_memory_context_section(
                    "Team Memories",
                    MemoryScope::Team,
                    cwd,
                    &team_mems,
                ));
            }
        }
    }

    // Auto memories — injected only when the caller opts in via toggle.
    if include_auto {
        if let Ok(auto_mems) = list_memories(MemoryScope::Auto, cwd) {
            if !auto_mems.is_empty() {
                sections.push(format_memory_context_section(
                    "Auto Memories",
                    MemoryScope::Auto,
                    cwd,
                    &auto_mems,
                ));
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
    fn test_write_memory_refreshes_memory_md_index() {
        let cwd = make_temp_dir();

        write_memory(
            "rust-setup",
            "cargo build\nsecond line should not be in the hook",
            "dev",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();
        write_memory(
            "release-notes",
            "ship notes",
            "",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let index_path = memory_entrypoint_path(MemoryScope::Project, &cwd).unwrap();
        let index = std::fs::read_to_string(index_path).unwrap();

        assert!(index.contains("- [rust-setup](rust-setup.json) - dev: cargo build"));
        assert!(index.contains("- [release-notes](release-notes.json) - ship notes"));
        assert!(!index.contains("second line should not be in the hook"));

        cleanup(&cwd);
    }

    #[test]
    fn test_delete_memory_removes_empty_memory_md_index() {
        let cwd = make_temp_dir();

        write_memory("temp", "temporary", "notes", MemoryScope::Project, &cwd).unwrap();
        let index_path = memory_entrypoint_path(MemoryScope::Project, &cwd).unwrap();
        assert!(index_path.exists());

        assert!(delete_memory("temp", MemoryScope::Project, &cwd).unwrap());
        assert!(!index_path.exists());

        cleanup(&cwd);
    }

    #[test]
    fn test_memory_md_index_obeys_entrypoint_limits() {
        let entries = (0..250)
            .map(|idx| MemoryEntry {
                key: format!("memory-{idx}"),
                value: "x".repeat(300),
                category: "project".to_string(),
                memory_type: Some(MemoryType::Project),
                description: None,
                search_terms: Vec::new(),
                created_at: "2026-05-06T00:00:00Z".to_string(),
                updated_at: "2026-05-06T00:00:00Z".to_string(),
            })
            .collect::<Vec<_>>();

        let index = build_memory_index_from_entries(&entries);

        assert!(index.lines().count() <= MEMORY_ENTRYPOINT_MAX_LINES);
        assert!(index.len() <= MEMORY_ENTRYPOINT_MAX_BYTES);
        assert!(index.contains("[truncated]"));
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
        assert!(ctx.contains("### MEMORY.md Index"));
        assert!(ctx.contains("[pref](pref.json) - ui: dark mode"));
        assert!(ctx.contains("pref"));
        assert!(ctx.contains("dark mode"));

        cleanup(&cwd);
    }

    #[test]
    fn test_recall_relevant_memories_scores_and_limits_results() {
        let cwd = make_temp_dir();
        for idx in 0..6 {
            write_memory(
                &format!("rust-build-{idx}"),
                "Use cargo test before cargo build when touching Rust context code.",
                "project",
                MemoryScope::Project,
                &cwd,
            )
            .unwrap();
        }
        write_memory(
            "unrelated-design",
            "Figma spacing notes for dashboard screens.",
            "reference",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let results =
            recall_relevant_memories(&cwd, false, "rust build context", &[], &HashSet::new(), 5)
                .unwrap();

        assert_eq!(results.len(), 5);
        assert!(results
            .iter()
            .all(|memory| memory.entry.key.starts_with("rust-build-")));

        cleanup(&cwd);
    }

    #[test]
    fn test_recall_relevant_memories_filters_already_surfaced() {
        let cwd = make_temp_dir();
        write_memory(
            "rust-build",
            "Use cargo test before cargo build.",
            "project",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let already = HashSet::from(["project:rust-build".to_string()]);
        let results =
            recall_relevant_memories(&cwd, false, "rust build", &[], &already, 5).unwrap();

        assert!(results.is_empty());
        cleanup(&cwd);
    }

    #[test]
    fn test_recall_relevant_memories_denoises_recent_tool_docs_but_keeps_warnings() {
        let cwd = make_temp_dir();
        write_memory(
            "bash-reference",
            "Bash tool usage guide and examples.",
            "reference",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();
        write_memory(
            "bash-warning",
            "Bash warning: avoid destructive git commands in dirty worktrees.",
            "reference",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let recent_tools = vec!["Bash".to_string()];
        let results = recall_relevant_memories(
            &cwd,
            false,
            "bash git commands",
            &recent_tools,
            &HashSet::new(),
            5,
        )
        .unwrap();

        let keys = results
            .iter()
            .map(|memory| memory.entry.key.as_str())
            .collect::<Vec<_>>();
        assert!(!keys.contains(&"bash-reference"));
        assert!(keys.contains(&"bash-warning"));

        cleanup(&cwd);
    }

    #[test]
    fn test_build_relevant_memory_context_returns_surfaced_identities() {
        let cwd = make_temp_dir();
        write_memory(
            "migration-risk",
            "Rust migration risk: keep session export round trips covered.",
            "project",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();
        write_memory(
            "unrelated",
            "Weekly roadmap note.",
            "project",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let (context, surfaced) = build_relevant_memory_context_with(
            &cwd,
            false,
            "migration risk",
            &[],
            &HashSet::new(),
            5,
        )
        .unwrap();

        assert!(context.contains("## Relevant Memories"));
        assert!(context.contains("migration-risk"));
        assert!(!context.contains("unrelated"));
        assert_eq!(surfaced, vec!["project:migration-risk".to_string()]);

        cleanup(&cwd);
    }

    #[test]
    fn test_recall_relevant_memories_respects_ignore_memory_request() {
        let cwd = make_temp_dir();
        write_memory(
            "rust-build",
            "Use cargo test before cargo build.",
            "project",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        let results = recall_relevant_memories(
            &cwd,
            false,
            "ignore memory and explain rust build",
            &[],
            &HashSet::new(),
            5,
        )
        .unwrap();

        assert!(results.is_empty());
        cleanup(&cwd);
    }

    #[test]
    fn test_legacy_memory_entries_without_recall_metadata_still_load() {
        let raw = r#"{
          "key": "legacy",
          "value": "legacy value",
          "category": "project",
          "created_at": "2026-05-06T00:00:00Z",
          "updated_at": "2026-05-06T00:00:00Z"
        }"#;

        let entry: MemoryEntry = serde_json::from_str(raw).unwrap();

        assert_eq!(entry.key, "legacy");
        assert_eq!(entry.description, None);
        assert!(entry.search_terms.is_empty());
        assert_eq!(entry.effective_memory_type(), Some(MemoryType::Project));
    }

    #[test]
    fn test_memory_type_closed_taxonomy_parse() {
        assert_eq!(MemoryType::ALL.len(), 4);
        assert_eq!(MemoryType::parse("user"), Some(MemoryType::User));
        assert_eq!(MemoryType::parse("feedback"), Some(MemoryType::Feedback));
        assert_eq!(MemoryType::parse("project"), Some(MemoryType::Project));
        assert_eq!(MemoryType::parse("reference"), Some(MemoryType::Reference));
        assert_eq!(MemoryType::parse("preference"), None);
    }

    #[test]
    fn test_write_memory_records_closed_memory_type() {
        let cwd = make_temp_dir();

        let entry = write_memory(
            "testing-feedback",
            "Prefer integration tests here.\n**Why:** catches migrations.",
            "feedback",
            MemoryScope::Project,
            &cwd,
        )
        .unwrap();

        assert_eq!(entry.memory_type, Some(MemoryType::Feedback));

        let read = read_memory("testing-feedback", MemoryScope::Project, &cwd).unwrap();
        assert_eq!(read.effective_memory_type(), Some(MemoryType::Feedback));

        let raw = std::fs::read_to_string(
            memory_dir(MemoryScope::Project, &cwd)
                .unwrap()
                .join(key_to_filename("testing-feedback")),
        )
        .unwrap();
        assert!(raw.contains(r#""type": "feedback""#));

        let ctx = build_memory_context(&cwd).unwrap();
        assert!(ctx.contains("[testing-feedback](testing-feedback.json) - feedback:"));
        assert!(ctx.contains("**testing-feedback** [feedback]:"));

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
