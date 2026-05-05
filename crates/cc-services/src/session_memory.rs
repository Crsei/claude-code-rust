//! Session memory service — extracts conversation insights into persistent
//! memory entries on disk.
//!
//! Memory entries are stored as individual JSON files under `~/.cc-rust/memory/`.
//! This is complementary to the `session::memdir` module which manages
//! CLAUDE.md-based memories; this service focuses on structured, searchable
//! per-session insights.

#![allow(unused)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the session memory service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryConfig {
    /// Whether the service is enabled.
    pub enabled: bool,
    /// Directory where memory entries are stored.
    pub memory_dir: PathBuf,
    /// Maximum number of memory entries per session.
    pub max_entries: usize,
    /// Minimum messages in a conversation before extraction is triggered.
    pub min_messages_before_extract: usize,
}

impl Default for SessionMemoryConfig {
    fn default() -> Self {
        SessionMemoryConfig {
            enabled: true,
            memory_dir: cc_config::paths::session_insights_dir(),
            max_entries: 50,
            min_messages_before_extract: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Memory entry
// ---------------------------------------------------------------------------

/// A single memory entry extracted from a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier for this entry.
    pub id: String,
    /// Unix timestamp (seconds since epoch) when the entry was created.
    pub timestamp: i64,
    /// Session ID this entry was extracted from.
    pub session_id: String,
    /// Workspace path this entry was extracted from, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The insight content extracted from the conversation.
    pub content: String,
    /// Categorization tags.
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Background service for extracting and persisting conversation insights.
pub struct SessionMemoryService {
    config: SessionMemoryConfig,
    entries: Vec<MemoryEntry>,
}

impl SessionMemoryService {
    /// Create a new service with the given configuration.
    pub fn new(config: SessionMemoryConfig) -> Self {
        SessionMemoryService {
            config,
            entries: Vec::new(),
        }
    }

    /// Load all existing memory entries from disk.
    pub fn load_from_disk(&mut self) -> Result<()> {
        let dir = &self.config.memory_dir;
        if !dir.exists() {
            return Ok(());
        }

        let mut loaded: Vec<MemoryEntry> = Vec::new();

        let read_dir = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read memory directory: {}", dir.display()))?;

        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<MemoryEntry>(&content) {
                        Ok(mem_entry) => loaded.push(mem_entry),
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Skipping malformed memory entry"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to read memory entry"
                        );
                    }
                }
            }
        }

        // Sort by timestamp, newest first
        loaded.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.entries = loaded;
        Ok(())
    }

    /// Save a single entry to disk and add it to the in-memory collection.
    pub fn save_entry(&mut self, entry: MemoryEntry) -> Result<()> {
        // Ensure the memory directory exists
        if !self.config.memory_dir.exists() {
            std::fs::create_dir_all(&self.config.memory_dir).with_context(|| {
                format!(
                    "Failed to create memory directory: {}",
                    self.config.memory_dir.display()
                )
            })?;
        }

        let filename = format!("{}.json", entry.id);
        let path = self.config.memory_dir.join(&filename);
        let json =
            serde_json::to_string_pretty(&entry).context("Failed to serialize memory entry")?;
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write memory entry: {}", path.display()))?;

        self.entries.insert(0, entry); // newest first

        // Enforce max entries limit
        while self.entries.len() > self.config.max_entries {
            if let Some(removed) = self.entries.pop() {
                let remove_path = self.config.memory_dir.join(format!("{}.json", removed.id));
                let _ = std::fs::remove_file(&remove_path);
            }
        }

        Ok(())
    }

    /// Check whether the extraction threshold has been met.
    pub fn should_extract(&self, message_count: usize) -> bool {
        self.config.enabled && message_count >= self.config.min_messages_before_extract
    }

    /// Get the most recent memory entries for context injection.
    pub fn get_memory_context(&self, limit: usize) -> Vec<MemoryEntry> {
        self.entries.iter().take(limit).cloned().collect()
    }

    /// Format recent entries for injection into the system prompt.
    pub fn format_memory_context(&self, limit: usize) -> Option<String> {
        self.format_memory_context_for_workspace(limit, None)
    }

    /// Format recent entries scoped to a workspace for prompt injection.
    pub fn format_memory_context_for_workspace(
        &self,
        limit: usize,
        workspace: Option<&Path>,
    ) -> Option<String> {
        if !self.config.enabled {
            return None;
        }

        let workspace = workspace.map(|p| p.to_string_lossy().to_string());
        let entries: Vec<MemoryEntry> = self
            .entries
            .iter()
            .filter(|entry| match workspace.as_deref() {
                Some(expected) => entry.workspace.as_deref() == Some(expected),
                None => true,
            })
            .take(limit)
            .cloned()
            .collect();
        if entries.is_empty() {
            return None;
        }

        let mut context = String::from("<session-insights>\n");
        for entry in entries {
            let content = entry.content.replace(['\r', '\n'], " ");
            let tags = if entry.tags.is_empty() {
                String::new()
            } else {
                format!(" tags={}", entry.tags.join(","))
            };
            context.push_str(&format!("- [{}{}] {}\n", entry.session_id, tags, content));
        }
        context.push_str("</session-insights>");
        Some(context)
    }

    /// Simple substring search across all entries' content and tags.
    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.content.to_lowercase().contains(&query_lower)
                    || e.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dir: &Path) -> SessionMemoryConfig {
        SessionMemoryConfig {
            enabled: true,
            memory_dir: dir.to_path_buf(),
            max_entries: 5,
            min_messages_before_extract: 3,
        }
    }

    fn make_entry(id: &str, content: &str, tags: &[&str]) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            session_id: "test-session".to_string(),
            workspace: None,
            content: content.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn should_extract_respects_threshold() {
        let cfg = SessionMemoryConfig {
            min_messages_before_extract: 5,
            ..Default::default()
        };
        let svc = SessionMemoryService::new(cfg);
        assert!(!svc.should_extract(2));
        assert!(!svc.should_extract(4));
        assert!(svc.should_extract(5));
        assert!(svc.should_extract(10));
    }

    #[test]
    fn should_extract_disabled_returns_false() {
        let cfg = SessionMemoryConfig {
            enabled: false,
            ..Default::default()
        };
        let svc = SessionMemoryService::new(cfg);
        assert!(!svc.should_extract(100));
    }

    #[test]
    fn search_matches_content_and_tags() {
        let tmp = std::env::temp_dir().join("cc_rust_test_session_mem_search");
        let mut svc = SessionMemoryService::new(test_config(&tmp));
        svc.entries.push(make_entry("1", "Rust is fast", &["lang"]));
        svc.entries.push(make_entry(
            "2",
            "Python is flexible",
            &["lang", "scripting"],
        ));
        svc.entries
            .push(make_entry("3", "Using cargo build", &["rust", "tooling"]));

        let results = svc.search("rust");
        assert_eq!(results.len(), 2); // entry 1 (content) + entry 3 (tag)

        let results = svc.search("scripting");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "2");
    }

    #[test]
    fn get_memory_context_respects_limit() {
        let tmp = std::env::temp_dir().join("cc_rust_test_session_mem_ctx");
        let mut svc = SessionMemoryService::new(test_config(&tmp));
        for i in 0..10 {
            svc.entries
                .push(make_entry(&format!("e{}", i), "content", &[]));
        }
        let ctx = svc.get_memory_context(3);
        assert_eq!(ctx.len(), 3);
    }

    #[test]
    fn format_memory_context_wraps_recent_entries() {
        let tmp = std::env::temp_dir().join("cc_rust_test_session_mem_format");
        let mut svc = SessionMemoryService::new(test_config(&tmp));
        svc.entries.push(make_entry(
            "1",
            "Use cargo test filters\nfor focused checks.",
            &["testing"],
        ));
        svc.entries
            .push(make_entry("2", "Prefer existing patterns", &[]));

        let ctx = svc.format_memory_context(1).unwrap();
        assert!(ctx.starts_with("<session-insights>"));
        assert!(ctx.contains("Use cargo test filters for focused checks."));
        assert!(ctx.contains("tags=testing"));
        assert!(!ctx.contains("Prefer existing patterns"));
        assert!(ctx.ends_with("</session-insights>"));
    }

    #[test]
    fn format_memory_context_for_workspace_filters_entries() {
        let tmp = std::env::temp_dir().join("cc_rust_test_session_mem_workspace");
        let mut svc = SessionMemoryService::new(test_config(&tmp));
        let workspace = tmp.join("workspace-a");
        let other = tmp.join("workspace-b");

        let mut matching = make_entry("1", "workspace insight", &[]);
        matching.workspace = Some(workspace.to_string_lossy().to_string());
        svc.entries.push(matching);

        let mut non_matching = make_entry("2", "other workspace insight", &[]);
        non_matching.workspace = Some(other.to_string_lossy().to_string());
        svc.entries.push(non_matching);

        let ctx = svc
            .format_memory_context_for_workspace(5, Some(&workspace))
            .unwrap();
        assert!(ctx.contains("workspace insight"));
        assert!(!ctx.contains("other workspace insight"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("cc_rust_test_session_mem_roundtrip");
        let _ = std::fs::remove_dir_all(&tmp);

        let cfg = test_config(&tmp);
        let mut svc = SessionMemoryService::new(cfg.clone());

        let entry = make_entry("rt1", "roundtrip test", &["test"]);
        svc.save_entry(entry).unwrap();
        assert_eq!(svc.entries.len(), 1);

        // Load into a fresh service
        let mut svc2 = SessionMemoryService::new(cfg);
        svc2.load_from_disk().unwrap();
        assert_eq!(svc2.entries.len(), 1);
        assert_eq!(svc2.entries[0].id, "rt1");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
