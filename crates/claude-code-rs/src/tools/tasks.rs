//! Task management tools: create, get, update, list, stop, and output.
//!
//! Tool tasks are persisted under the cc-rust data root (`~/.cc-rust/tasks`
//! by default, or `$CC_RUST_HOME/tasks`). `TaskStore` keeps a process-local
//! index for fast reads, while `TaskRepository` owns the versioned on-disk
//! schema, output retention, restart recovery, and lightweight migrations.
//!
//! Runtime cancellation handles are intentionally separate from persisted
//! task metadata: persisted records survive restart, cancellation tokens do
//! not. On startup, unfinished tasks without a live supervisor are migrated to
//! `interrupted`.

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

const TASK_SCHEMA_VERSION: u32 = 3;
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const OUTPUT_SUMMARY_MAX_CHARS: usize = 2_000;

// =============================================================================
// TaskStore: shared state
// =============================================================================

/// Shared task store backed by a durable repository and runtime handles.
#[derive(Debug, Clone)]
pub struct TaskStore {
    tasks: Arc<Mutex<HashMap<String, TaskEntry>>>,
    runtime_handles: Arc<Mutex<HashMap<String, TaskRuntimeHandle>>>,
    repository: Arc<TaskRepository>,
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Options accepted by `TaskStore::create_with_options`.
#[derive(Debug, Clone, Default)]
pub struct TaskCreateOptions {
    pub kind: Option<String>,
    pub parent_id: Option<String>,
    pub depends_on: Vec<String>,
    pub agent_id: Option<String>,
    pub supervisor_id: Option<String>,
    pub isolation: Option<String>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
}

/// A process-local handle used to cancel active task execution.
#[derive(Debug, Clone)]
pub struct TaskRuntimeHandle {
    cancellation_token: CancellationToken,
}

impl TaskRuntimeHandle {
    pub fn new(cancellation_token: CancellationToken) -> Self {
        Self { cancellation_token }
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

/// A single task entry.
#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub output: String,
    pub output_summary: String,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub parent_id: Option<String>,
    pub depends_on: Vec<String>,
    pub agent_id: Option<String>,
    pub supervisor_id: Option<String>,
    pub isolation: Option<String>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub cancel_requested_at: Option<i64>,
    pub recovered_at: Option<i64>,
    pub previous_status: Option<TaskStatus>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Task lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    Recoverable,
    /// Legacy status retained so old serialized values can migrate cleanly.
    Stopped,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Interrupted => "interrupted",
            TaskStatus::Recoverable => "recoverable",
            TaskStatus::Stopped => "stopped",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "in_progress" | "running" => Some(TaskStatus::InProgress),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            "cancelled" | "canceled" => Some(TaskStatus::Cancelled),
            "interrupted" => Some(TaskStatus::Interrupted),
            "recoverable" => Some(TaskStatus::Recoverable),
            "stopped" => Some(TaskStatus::Stopped),
            _ => None,
        }
    }

    fn should_interrupt_on_startup(self) -> bool {
        matches!(
            self,
            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Recoverable
        )
    }

    fn is_success(self) -> bool {
        matches!(self, TaskStatus::Completed)
    }
}

impl TaskStore {
    pub fn new() -> Self {
        Self::with_dir(crate::config::paths::tasks_dir())
    }

    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self::with_dir_and_output_limit(dir, DEFAULT_OUTPUT_LIMIT_BYTES)
    }

    pub fn with_dir_and_output_limit(dir: impl Into<PathBuf>, output_limit_bytes: usize) -> Self {
        let repository = Arc::new(TaskRepository::new(dir.into(), output_limit_bytes));
        let tasks = match repository.load() {
            Ok(tasks) => tasks,
            Err(err) => {
                tracing::warn!(error = %err, "failed to load persisted tasks; starting with empty task store");
                HashMap::new()
            }
        };
        Self {
            tasks: Arc::new(Mutex::new(tasks)),
            runtime_handles: Arc::new(Mutex::new(HashMap::new())),
            repository,
        }
    }

    pub fn create(&self, subject: &str, description: &str) -> TaskEntry {
        self.create_with_options(subject, description, TaskCreateOptions::default())
    }

    pub fn create_with_options(
        &self,
        subject: &str,
        description: &str,
        options: TaskCreateOptions,
    ) -> TaskEntry {
        let now = chrono::Utc::now().timestamp();
        let id = uuid::Uuid::new_v4().to_string();
        let mut entry = TaskEntry {
            id: id.clone(),
            kind: sanitize_kind(options.kind.as_deref().unwrap_or("tool")),
            subject: subject.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            output: String::new(),
            output_summary: String::new(),
            output_bytes: 0,
            output_truncated: false,
            parent_id: options.parent_id.filter(|s| !s.trim().is_empty()),
            depends_on: normalize_dependencies(options.depends_on),
            agent_id: normalize_optional_string(options.agent_id),
            supervisor_id: normalize_optional_string(options.supervisor_id),
            isolation: normalize_optional_string(options.isolation),
            worktree_path: normalize_optional_string(options.worktree_path),
            worktree_branch: normalize_optional_string(options.worktree_branch),
            cancel_requested_at: None,
            recovered_at: None,
            previous_status: None,
            created_at: now,
            updated_at: now,
        };
        refresh_output_metadata(&mut entry);

        self.tasks.lock().insert(id, entry.clone());
        self.persist_entry(&entry);
        entry
    }

    pub fn get(&self, id: &str) -> Option<TaskEntry> {
        self.tasks.lock().get(id).cloned()
    }

    pub fn update_status(&self, id: &str, status: TaskStatus) -> Option<TaskEntry> {
        let mut tasks = self.tasks.lock();
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = normalize_new_status(status);
            entry.updated_at = chrono::Utc::now().timestamp();
            if entry.status == TaskStatus::Cancelled && entry.cancel_requested_at.is_none() {
                entry.cancel_requested_at = Some(entry.updated_at);
            }
            let cloned = entry.clone();
            drop(tasks);
            self.persist_entry(&cloned);
            Some(cloned)
        } else {
            None
        }
    }

    /// Append output text to a task's retained log.
    ///
    /// Output retention is bounded. When the configured byte cap is exceeded,
    /// the oldest bytes are dropped on a UTF-8 boundary and the task is marked
    /// as truncated.
    pub fn append_output(&self, id: &str, output: &str) -> Option<TaskEntry> {
        let mut tasks = self.tasks.lock();
        if let Some(entry) = tasks.get_mut(id) {
            if !entry.output.is_empty() && !output.is_empty() {
                entry.output.push('\n');
            }
            entry.output.push_str(output);
            let (trimmed, truncated_now) =
                trim_output_to_limit(&entry.output, self.repository.output_limit_bytes);
            entry.output = trimmed;
            entry.output_truncated |= truncated_now;
            entry.updated_at = chrono::Utc::now().timestamp();
            refresh_output_metadata(entry);
            let cloned = entry.clone();
            drop(tasks);
            self.persist_entry(&cloned);
            Some(cloned)
        } else {
            None
        }
    }

    pub fn list(&self) -> Vec<TaskEntry> {
        let tasks = self.tasks.lock();
        let mut entries: Vec<TaskEntry> = tasks.values().cloned().collect();
        entries.sort_by_key(|e| (e.created_at, e.id.clone()));
        entries
    }

    pub fn delete(&self, id: &str) -> Option<TaskEntry> {
        let removed = self.tasks.lock().remove(id);
        if removed.is_some() {
            self.runtime_handles.lock().remove(id);
            if let Err(err) = self.repository.delete(id) {
                tracing::warn!(task_id = id, error = %err, "failed to delete persisted task");
            }
        }
        removed
    }

    pub fn stop(&self, id: &str) -> Option<TaskEntry> {
        if let Some(handle) = self.runtime_handles.lock().get(id).cloned() {
            handle.cancel();
        }

        let now = chrono::Utc::now().timestamp();
        let mut tasks = self.tasks.lock();
        if let Some(entry) = tasks.get_mut(id) {
            entry.cancel_requested_at = Some(now);
            entry.status = TaskStatus::Cancelled;
            entry.updated_at = now;
            let cloned = entry.clone();
            drop(tasks);
            self.persist_entry(&cloned);
            Some(cloned)
        } else {
            None
        }
    }

    pub fn register_runtime_handle(&self, id: &str, cancellation_token: CancellationToken) -> bool {
        if !self.tasks.lock().contains_key(id) {
            return false;
        }
        self.runtime_handles
            .lock()
            .insert(id.to_string(), TaskRuntimeHandle::new(cancellation_token));
        true
    }

    pub fn unregister_runtime_handle(&self, id: &str) -> Option<TaskRuntimeHandle> {
        self.runtime_handles.lock().remove(id)
    }

    pub fn has_runtime_handle(&self, id: &str) -> bool {
        self.runtime_handles.lock().contains_key(id)
    }

    pub fn get_by_agent_id(&self, agent_id: &str) -> Option<TaskEntry> {
        self.tasks
            .lock()
            .values()
            .find(|entry| entry.agent_id.as_deref() == Some(agent_id))
            .cloned()
    }

    pub fn blocked_dependencies(&self, entry: &TaskEntry) -> Vec<String> {
        let tasks = self.tasks.lock();
        entry
            .depends_on
            .iter()
            .filter(|id| {
                tasks
                    .get(*id)
                    .map(|dep| !dep.status.is_success())
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    fn persist_entry(&self, entry: &TaskEntry) {
        if let Err(err) = self.repository.persist_entry(entry) {
            tracing::warn!(
                task_id = %entry.id,
                error = %err,
                "failed to persist task"
            );
        }
    }
}

fn task_to_json(entry: &TaskEntry) -> Value {
    let blocked_dependencies = store().blocked_dependencies(entry);
    json!({
        "id": entry.id,
        "kind": entry.kind,
        "subject": entry.subject,
        "description": entry.description,
        "status": entry.status.as_str(),
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "parent_id": entry.parent_id,
        "depends_on": entry.depends_on,
        "agent_id": entry.agent_id,
        "supervisor_id": entry.supervisor_id,
        "isolation": entry.isolation,
        "worktree_path": entry.worktree_path,
        "worktree_branch": entry.worktree_branch,
        "blocked_dependencies": blocked_dependencies,
        "output_summary": entry.output_summary,
        "output_bytes": entry.output_bytes,
        "output_truncated": entry.output_truncated,
        "cancel_requested_at": entry.cancel_requested_at,
        "recovered_at": entry.recovered_at,
        "previous_status": entry.previous_status.map(|s| s.as_str()),
        "has_runtime_handle": store().has_runtime_handle(&entry.id),
    })
}

// =============================================================================
// Durable repository
// =============================================================================

#[derive(Debug)]
struct TaskRepository {
    dir: PathBuf,
    output_limit_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedTaskFile {
    schema_version: u32,
    task: PersistedTaskRecord,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedTaskRecord {
    id: String,
    #[serde(default = "default_task_kind")]
    kind: String,
    subject: String,
    description: String,
    status: String,
    #[serde(default)]
    output_file: Option<String>,
    #[serde(default)]
    output_summary: String,
    #[serde(default)]
    output_bytes: usize,
    #[serde(default)]
    output_truncated: bool,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    supervisor_id: Option<String>,
    #[serde(default)]
    isolation: Option<String>,
    #[serde(default)]
    worktree_path: Option<String>,
    #[serde(default)]
    worktree_branch: Option<String>,
    #[serde(default)]
    cancel_requested_at: Option<i64>,
    #[serde(default)]
    recovered_at: Option<i64>,
    #[serde(default)]
    previous_status: Option<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(default, skip_serializing)]
    legacy_inline_output: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyTaskRecord {
    id: String,
    subject: String,
    description: String,
    status: String,
    #[serde(default)]
    output: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TaskFileOnDisk {
    Versioned(Box<PersistedTaskFile>),
    Legacy(LegacyTaskRecord),
}

impl TaskRepository {
    fn new(dir: PathBuf, output_limit_bytes: usize) -> Self {
        Self {
            dir,
            output_limit_bytes: output_limit_bytes.max(1),
        }
    }

    fn load(&self) -> Result<HashMap<String, TaskEntry>> {
        let mut tasks = HashMap::new();
        if !self.dir.exists() {
            return Ok(tasks);
        }

        for entry in fs::read_dir(&self.dir)
            .with_context(|| format!("failed to read task dir {}", self.dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            match self.load_entry(&path) {
                Ok(Some(task)) => {
                    tasks.insert(task.id.clone(), task);
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "failed to load persisted task record"
                    );
                }
            }
        }
        Ok(tasks)
    }

    fn load_entry(&self, path: &Path) -> Result<Option<TaskEntry>> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read task file {}", path.display()))?;
        let on_disk: TaskFileOnDisk = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse task file {}", path.display()))?;

        let (record, needs_schema_rewrite) = match on_disk {
            TaskFileOnDisk::Versioned(file) => {
                (file.task, file.schema_version != TASK_SCHEMA_VERSION)
            }
            TaskFileOnDisk::Legacy(legacy) => {
                let output_file = Some(output_file_name(&legacy.id));
                (
                    PersistedTaskRecord {
                        id: legacy.id,
                        kind: default_task_kind(),
                        subject: legacy.subject,
                        description: legacy.description,
                        status: legacy.status,
                        output_file,
                        output_summary: String::new(),
                        output_bytes: 0,
                        output_truncated: false,
                        parent_id: None,
                        depends_on: Vec::new(),
                        agent_id: None,
                        supervisor_id: None,
                        isolation: None,
                        worktree_path: None,
                        worktree_branch: None,
                        cancel_requested_at: None,
                        recovered_at: None,
                        previous_status: None,
                        created_at: legacy.created_at,
                        updated_at: legacy.updated_at,
                        legacy_inline_output: Some(legacy.output),
                    },
                    true,
                )
            }
        };

        let mut task = self.record_to_entry(record)?;
        let was_recovered = recover_task_after_restart(&mut task);
        refresh_output_metadata(&mut task);

        if needs_schema_rewrite || was_recovered {
            self.persist_entry(&task)?;
        }

        Ok(Some(task))
    }

    fn record_to_entry(&self, record: PersistedTaskRecord) -> Result<TaskEntry> {
        let status = TaskStatus::from_str(&record.status).unwrap_or(TaskStatus::Interrupted);
        let previous_status = record
            .previous_status
            .as_deref()
            .and_then(TaskStatus::from_str);
        let output = match record.legacy_inline_output {
            Some(output) => output,
            None => {
                let output_path = self.dir.join(
                    record
                        .output_file
                        .unwrap_or_else(|| output_file_name(&record.id)),
                );
                fs::read_to_string(&output_path).unwrap_or_default()
            }
        };
        let (output, output_truncated_now) = trim_output_to_limit(&output, self.output_limit_bytes);

        Ok(TaskEntry {
            id: record.id,
            kind: sanitize_kind(&record.kind),
            subject: record.subject,
            description: record.description,
            status,
            output,
            output_summary: record.output_summary,
            output_bytes: record.output_bytes,
            output_truncated: record.output_truncated || output_truncated_now,
            parent_id: record.parent_id.filter(|s| !s.trim().is_empty()),
            depends_on: normalize_dependencies(record.depends_on),
            agent_id: normalize_optional_string(record.agent_id),
            supervisor_id: normalize_optional_string(record.supervisor_id),
            isolation: normalize_optional_string(record.isolation),
            worktree_path: normalize_optional_string(record.worktree_path),
            worktree_branch: normalize_optional_string(record.worktree_branch),
            cancel_requested_at: record.cancel_requested_at,
            recovered_at: record.recovered_at,
            previous_status,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    fn persist_entry(&self, entry: &TaskEntry) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create task dir {}", self.dir.display()))?;

        let mut to_write = entry.clone();
        let (trimmed, truncated_now) =
            trim_output_to_limit(&to_write.output, self.output_limit_bytes);
        to_write.output = trimmed;
        to_write.output_truncated |= truncated_now;
        refresh_output_metadata(&mut to_write);

        let output_path = self.dir.join(output_file_name(&to_write.id));
        write_text_atomic(&output_path, &to_write.output)?;

        let file = PersistedTaskFile {
            schema_version: TASK_SCHEMA_VERSION,
            task: PersistedTaskRecord::from_entry(&to_write),
        };
        let json = serde_json::to_string_pretty(&file)?;
        write_text_atomic(&self.task_json_path(&to_write.id), &json)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let json_path = self.task_json_path(id);
        let output_path = self.dir.join(output_file_name(id));
        remove_if_exists(&json_path)?;
        remove_if_exists(&output_path)?;
        Ok(())
    }

    fn task_json_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", safe_file_stem(id)))
    }
}

impl PersistedTaskRecord {
    fn from_entry(entry: &TaskEntry) -> Self {
        Self {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            subject: entry.subject.clone(),
            description: entry.description.clone(),
            status: entry.status.as_str().to_string(),
            output_file: Some(output_file_name(&entry.id)),
            output_summary: entry.output_summary.clone(),
            output_bytes: entry.output_bytes,
            output_truncated: entry.output_truncated,
            parent_id: entry.parent_id.clone(),
            depends_on: entry.depends_on.clone(),
            agent_id: entry.agent_id.clone(),
            supervisor_id: entry.supervisor_id.clone(),
            isolation: entry.isolation.clone(),
            worktree_path: entry.worktree_path.clone(),
            worktree_branch: entry.worktree_branch.clone(),
            cancel_requested_at: entry.cancel_requested_at,
            recovered_at: entry.recovered_at,
            previous_status: entry.previous_status.map(|s| s.as_str().to_string()),
            created_at: entry.created_at,
            updated_at: entry.updated_at,
            legacy_inline_output: None,
        }
    }
}

fn recover_task_after_restart(entry: &mut TaskEntry) -> bool {
    let mut changed = false;
    let status = normalize_loaded_status(entry.status);
    if status != entry.status {
        entry.previous_status = Some(entry.status);
        entry.status = status;
        changed = true;
    }

    if entry.status.should_interrupt_on_startup() {
        let now = chrono::Utc::now().timestamp();
        entry.previous_status = Some(entry.status);
        entry.status = TaskStatus::Interrupted;
        entry.recovered_at = Some(now);
        entry.updated_at = now;
        return true;
    }

    changed
}

fn normalize_loaded_status(status: TaskStatus) -> TaskStatus {
    match status {
        TaskStatus::Stopped => TaskStatus::Cancelled,
        other => other,
    }
}

fn normalize_new_status(status: TaskStatus) -> TaskStatus {
    match status {
        TaskStatus::Stopped => TaskStatus::Cancelled,
        other => other,
    }
}

fn refresh_output_metadata(entry: &mut TaskEntry) {
    entry.output_bytes = entry.output.len();
    entry.output_summary = summarize_output(&entry.output);
}

fn summarize_output(output: &str) -> String {
    if output.chars().count() <= OUTPUT_SUMMARY_MAX_CHARS {
        return output.to_string();
    }
    let tail: String = output
        .chars()
        .rev()
        .take(OUTPUT_SUMMARY_MAX_CHARS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    tail
}

fn trim_output_to_limit(output: &str, limit: usize) -> (String, bool) {
    if output.len() <= limit {
        return (output.to_string(), false);
    }

    let mut start = output.len().saturating_sub(limit);
    while start < output.len() && !output.is_char_boundary(start) {
        start += 1;
    }
    (output[start..].to_string(), true)
}

fn normalize_dependencies(depends_on: Vec<String>) -> Vec<String> {
    let mut deps = Vec::new();
    for dep in depends_on {
        let dep = dep.trim();
        if dep.is_empty() || deps.iter().any(|existing: &String| existing == dep) {
            continue;
        }
        deps.push(dep.to_string());
    }
    deps
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn sanitize_kind(kind: &str) -> String {
    let trimmed = kind.trim();
    if trimmed.is_empty() {
        return default_task_kind();
    }
    trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn default_task_kind() -> String {
    "tool".to_string()
}

fn output_file_name(id: &str) -> String {
    format!("{}.output.log", safe_file_stem(id))
}

fn safe_file_stem(id: &str) -> String {
    let stem: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "task".to_string()
    } else {
        stem
    }
}

fn write_text_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
    }

    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("task")
    ));
    fs::write(&tmp_path, contents)
        .with_context(|| format!("failed to write temp file {}", tmp_path.display()))?;

    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove old file {}", path.display()))?;
    }
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to move temp file {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

// =============================================================================
// Global task store (lazy singleton)
// =============================================================================

#[cfg(not(test))]
static GLOBAL_STORE: std::sync::LazyLock<TaskStore> = std::sync::LazyLock::new(TaskStore::new);

#[cfg(test)]
static GLOBAL_STORE: std::sync::LazyLock<TaskStore> = std::sync::LazyLock::new(|| {
    TaskStore::with_dir(std::env::temp_dir().join(format!(
        "cc-rust-test-global-tasks-{}",
        uuid::Uuid::new_v4()
    )))
});

fn store() -> &'static TaskStore {
    &GLOBAL_STORE
}

/// Read-only handle to the global task store, exposed for command surfaces
/// (`/tasks`) that want to enumerate tool-driven tasks without running a
/// tool call. The store is cheap to clone: all interior state is behind `Arc`.
pub fn global_store() -> TaskStore {
    GLOBAL_STORE.clone()
}

// =============================================================================
// TaskCreateTool
// =============================================================================

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "TaskCreate"
    }

    async fn description(&self, _: &Value) -> String {
        "Create a new task to track work progress.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "A brief title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "What needs to be done"
                },
                "kind": {
                    "type": "string",
                    "description": "Stable task kind for persisted records",
                    "enum": ["tool", "local_shell", "local_agent", "remote_agent", "workflow", "monitor", "dream", "team"]
                },
                "parent_id": {
                    "type": "string",
                    "description": "Optional parent task ID"
                },
                "depends_on": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that should complete before this task"
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let kind = input
            .get("kind")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let parent_id = input
            .get("parent_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let depends_on: Vec<String> = input
            .get("depends_on")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let has_options = kind.is_some() || parent_id.is_some() || !depends_on.is_empty();

        let entry = if has_options {
            store().create_with_options(
                subject,
                description,
                TaskCreateOptions {
                    kind,
                    parent_id,
                    depends_on,
                    ..TaskCreateOptions::default()
                },
            )
        } else {
            store().create(subject, description)
        };

        // Fire TaskCreated hook.
        {
            let app_state = (ctx.get_app_state)();
            let configs = crate::tools::hooks::load_hook_configs(&app_state.hooks, "TaskCreated");
            if !configs.is_empty() {
                let payload = json!({
                    "task_id": &entry.id,
                    "subject": &entry.subject,
                    "description": &entry.description,
                });
                let _ =
                    crate::tools::hooks::run_event_hooks("TaskCreated", &payload, &configs).await;
            }
        }

        Ok(ToolResult {
            data: json!({
                "task": task_to_json(&entry),
                "message": format!("Created task: {}", entry.subject)
            }),
            new_messages: vec![],
            ..Default::default()
        })
    }

    async fn prompt(&self) -> String {
        "Create tasks to track your progress on complex work.".to_string()
    }
}

// =============================================================================
// TaskGetTool
// =============================================================================

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "TaskGet"
    }

    async fn description(&self, _: &Value) -> String {
        "Get details of a task by ID.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to look up"
                }
            },
            "required": ["task_id"]
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");

        match store().get(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({ "task": task_to_json(&entry) }),
                new_messages: vec![],
                ..Default::default()
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Get the current status of a task.".to_string()
    }
}

// =============================================================================
// TaskUpdateTool
// =============================================================================

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "TaskUpdate"
    }

    async fn description(&self, _: &Value) -> String {
        "Update a task's status.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "failed", "cancelled", "recoverable", "interrupted"],
                    "description": "New status for the task"
                }
            },
            "required": ["task_id", "status"]
        })
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let status_str = input
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("in_progress");

        let Some(status) = TaskStatus::from_str(status_str) else {
            return Ok(ToolResult {
                data: json!({ "error": format!("Invalid task status: {}", status_str) }),
                new_messages: vec![],
                ..Default::default()
            });
        };

        match store().update_status(id, status) {
            Some(entry) => {
                // Fire TaskCompleted hook when status changes to completed.
                if entry.status == TaskStatus::Completed {
                    let app_state = (ctx.get_app_state)();
                    let configs =
                        crate::tools::hooks::load_hook_configs(&app_state.hooks, "TaskCompleted");
                    if !configs.is_empty() {
                        let payload = json!({
                            "task_id": &entry.id,
                            "subject": &entry.subject,
                            "status": entry.status.as_str(),
                        });
                        let _ = crate::tools::hooks::run_event_hooks(
                            "TaskCompleted",
                            &payload,
                            &configs,
                        )
                        .await;
                    }
                }

                Ok(ToolResult {
                    data: json!({
                        "task": task_to_json(&entry),
                        "message": format!("Task '{}' updated to {}", entry.subject, entry.status.as_str())
                    }),
                    new_messages: vec![],
                    ..Default::default()
                })
            }
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Update task status to track progress.".to_string()
    }
}

// =============================================================================
// TaskListTool
// =============================================================================

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "TaskList"
    }

    async fn description(&self, _: &Value) -> String {
        "List all tasks and their statuses.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        _input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let entries = store().list();
        let tasks: Vec<Value> = entries.iter().map(task_to_json).collect();

        Ok(ToolResult {
            data: json!({
                "tasks": tasks,
                "count": tasks.len()
            }),
            new_messages: vec![],
            ..Default::default()
        })
    }

    async fn prompt(&self) -> String {
        "List all tasks to see current progress.".to_string()
    }
}

// =============================================================================
// TaskStopTool
// =============================================================================

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "TaskStop"
    }

    async fn description(&self, _: &Value) -> String {
        "Cancel a running task.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to cancel"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");

        match store().stop(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task": task_to_json(&entry),
                    "message": format!("Task '{}' cancelled", entry.subject)
                }),
                new_messages: vec![],
                ..Default::default()
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Cancel a running task.".to_string()
    }
}

// =============================================================================
// TaskOutputTool
// =============================================================================

pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }

    async fn description(&self, _: &Value) -> String {
        "Get the retained output/log of a task.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID whose output to retrieve"
                }
            },
            "required": ["task_id"]
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");

        match store().get(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task_id": entry.id,
                    "subject": entry.subject,
                    "agent_id": entry.agent_id,
                    "supervisor_id": entry.supervisor_id,
                    "isolation": entry.isolation,
                    "worktree_path": entry.worktree_path,
                    "worktree_branch": entry.worktree_branch,
                    "output": if entry.output.is_empty() {
                        "(no output yet)".to_string()
                    } else {
                        entry.output
                    },
                    "output_summary": entry.output_summary,
                    "output_bytes": entry.output_bytes,
                    "output_truncated": entry.output_truncated,
                }),
                new_messages: vec![],
                ..Default::default()
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Get the retained output or logs from a task.".to_string()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use std::thread;

    fn temp_store() -> (tempfile::TempDir, TaskStore) {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        (tmp, store)
    }

    fn temp_store_with_limit(limit: usize) -> (tempfile::TempDir, TaskStore) {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir_and_output_limit(tmp.path(), limit);
        (tmp, store)
    }

    #[test]
    fn test_task_store_create_and_get() {
        let (_tmp, store) = temp_store();
        let task = store.create("Test task", "Do the thing");
        assert_eq!(task.subject, "Test task");
        assert_eq!(task.description, "Do the thing");
        assert_eq!(task.status, TaskStatus::Pending);

        let fetched = store.get(&task.id).unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[test]
    fn test_task_store_persists_and_recovers_completed_task() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let task = store.create("Persist me", "survive restart");
        store.append_output(&task.id, "line 1");
        store.update_status(&task.id, TaskStatus::Completed);

        let restarted = TaskStore::with_dir(tmp.path());
        let fetched = restarted.get(&task.id).unwrap();
        assert_eq!(fetched.status, TaskStatus::Completed);
        assert_eq!(fetched.output, "line 1");
        assert_eq!(fetched.output_summary, "line 1");
    }

    #[test]
    fn test_restart_marks_unfinished_tasks_interrupted() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let pending = store.create("pending", "restart");
        let running = store.create("running", "restart");
        store.update_status(&running.id, TaskStatus::InProgress);

        let restarted = TaskStore::with_dir(tmp.path());
        let pending = restarted.get(&pending.id).unwrap();
        let running = restarted.get(&running.id).unwrap();

        assert_eq!(pending.status, TaskStatus::Interrupted);
        assert_eq!(pending.previous_status, Some(TaskStatus::Pending));
        assert!(pending.recovered_at.is_some());
        assert_eq!(running.status, TaskStatus::Interrupted);
        assert_eq!(running.previous_status, Some(TaskStatus::InProgress));
    }

    #[test]
    fn test_terminal_statuses_survive_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let completed = store.create("completed", "");
        let failed = store.create("failed", "");
        let cancelled = store.create("cancelled", "");
        store.update_status(&completed.id, TaskStatus::Completed);
        store.update_status(&failed.id, TaskStatus::Failed);
        store.stop(&cancelled.id);

        let restarted = TaskStore::with_dir(tmp.path());
        assert_eq!(
            restarted.get(&completed.id).unwrap().status,
            TaskStatus::Completed
        );
        assert_eq!(
            restarted.get(&failed.id).unwrap().status,
            TaskStatus::Failed
        );
        assert_eq!(
            restarted.get(&cancelled.id).unwrap().status,
            TaskStatus::Cancelled
        );
    }

    #[test]
    fn test_task_store_update_status() {
        let (_tmp, store) = temp_store();
        let task = store.create("Update me", "...");

        let updated = store
            .update_status(&task.id, TaskStatus::InProgress)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);

        let completed = store
            .update_status(&task.id, TaskStatus::Completed)
            .unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[test]
    fn test_task_store_list() {
        let (_tmp, store) = temp_store();
        store.create("Task A", "First");
        store.create("Task B", "Second");
        let list = store.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_task_store_stop_cancels_runtime_handle() {
        let (_tmp, store) = temp_store();
        let task = store.create("Stop me", "...");
        let token = CancellationToken::new();
        assert!(store.register_runtime_handle(&task.id, token.clone()));

        let stopped = store.stop(&task.id).unwrap();
        assert_eq!(stopped.status, TaskStatus::Cancelled);
        assert!(stopped.cancel_requested_at.is_some());
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_task_store_append_output() {
        let (_tmp, store) = temp_store();
        let task = store.create("Output task", "...");
        store.append_output(&task.id, "line 1");
        store.append_output(&task.id, "line 2");
        let entry = store.get(&task.id).unwrap();
        assert_eq!(entry.output, "line 1\nline 2");
        assert_eq!(entry.output_bytes, "line 1\nline 2".len());
    }

    #[test]
    fn test_output_retention_is_bounded() {
        let (_tmp, store) = temp_store_with_limit(10);
        let task = store.create("Output task", "...");
        store.append_output(&task.id, "0123456789");
        store.append_output(&task.id, "abcdef");
        let entry = store.get(&task.id).unwrap();
        assert!(entry.output.len() <= 10);
        assert!(entry.output_truncated);
        assert!(entry.output.ends_with("abcdef"));
    }

    #[test]
    fn test_dependencies_roundtrip_and_blocking() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let dep = store.create("dep", "");
        let child = store.create_with_options(
            "child",
            "",
            TaskCreateOptions {
                kind: Some("local_agent".to_string()),
                parent_id: Some(dep.id.clone()),
                depends_on: vec![dep.id.clone(), dep.id.clone()],
                ..TaskCreateOptions::default()
            },
        );

        assert_eq!(child.kind, "local_agent");
        assert_eq!(child.parent_id.as_deref(), Some(dep.id.as_str()));
        assert_eq!(child.depends_on, vec![dep.id.clone()]);
        assert_eq!(store.blocked_dependencies(&child), vec![dep.id.clone()]);

        store.update_status(&dep.id, TaskStatus::Completed);
        let restarted = TaskStore::with_dir(tmp.path());
        let child = restarted.get(&child.id).unwrap();
        assert_eq!(child.depends_on, vec![dep.id.clone()]);
        assert!(restarted.blocked_dependencies(&child).is_empty());
    }

    #[test]
    fn test_agent_metadata_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let task = store.create_with_options(
            "agent task",
            "metadata",
            TaskCreateOptions {
                kind: Some("local_agent".to_string()),
                agent_id: Some("agent-1".to_string()),
                supervisor_id: Some("supervisor-1".to_string()),
                isolation: Some("worktree".to_string()),
                worktree_path: Some("/tmp/agent-worktree-abcd1234".to_string()),
                worktree_branch: Some("agent-worktree-abcd1234".to_string()),
                ..TaskCreateOptions::default()
            },
        );

        let by_agent = store.get_by_agent_id("agent-1").unwrap();
        assert_eq!(by_agent.id, task.id);

        let restarted = TaskStore::with_dir(tmp.path());
        let restored = restarted.get_by_agent_id("agent-1").unwrap();
        assert_eq!(restored.supervisor_id.as_deref(), Some("supervisor-1"));
        assert_eq!(restored.isolation.as_deref(), Some("worktree"));
        assert_eq!(
            restored.worktree_branch.as_deref(),
            Some("agent-worktree-abcd1234")
        );
    }

    #[test]
    fn test_task_store_not_found() {
        let (_tmp, store) = temp_store();
        assert!(store.get("nonexistent").is_none());
        assert!(store
            .update_status("nonexistent", TaskStatus::Completed)
            .is_none());
        assert!(store.stop("nonexistent").is_none());
    }

    #[test]
    fn test_task_status_roundtrip() {
        for status in [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
            TaskStatus::Interrupted,
            TaskStatus::Recoverable,
            TaskStatus::Stopped,
        ] {
            let s = status.as_str();
            assert_eq!(TaskStatus::from_str(s), Some(status));
        }
        assert_eq!(TaskStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_task_to_json() {
        let (_tmp, store) = temp_store();
        let task = store.create("JSON test", "desc");
        let json = task_to_json_for_store(&store, &task);
        assert_eq!(json["subject"], "JSON test");
        assert_eq!(json["description"], "desc");
        assert_eq!(json["status"], "pending");
        assert_eq!(json["kind"], "tool");
        assert_eq!(json["output_truncated"], false);
    }

    #[test]
    fn test_legacy_task_record_migrates() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "legacy-task";
        fs::write(
            tmp.path().join("legacy-task.json"),
            serde_json::to_string_pretty(&json!({
                "id": id,
                "subject": "legacy",
                "description": "old format",
                "status": "stopped",
                "output": "legacy output",
                "created_at": 1,
                "updated_at": 2
            }))
            .unwrap(),
        )
        .unwrap();

        let store = TaskStore::with_dir(tmp.path());
        let migrated = store.get(id).unwrap();
        assert_eq!(migrated.status, TaskStatus::Cancelled);
        assert_eq!(migrated.output, "legacy output");

        let raw = fs::read_to_string(tmp.path().join("legacy-task.json")).unwrap();
        let persisted: PersistedTaskFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(persisted.schema_version, TASK_SCHEMA_VERSION);
        assert_eq!(persisted.task.status, "cancelled");
    }

    #[test]
    fn test_concurrent_output_appends_remain_bounded() {
        let (_tmp, store) = temp_store_with_limit(256);
        let task = store.create("concurrent", "");
        let store = Arc::new(store);
        let mut threads = Vec::new();

        for i in 0..8 {
            let store = store.clone();
            let id = task.id.clone();
            threads.push(thread::spawn(move || {
                for j in 0..25 {
                    store.append_output(&id, &format!("line-{i}-{j}"));
                }
            }));
        }

        for thread in threads {
            thread.join().unwrap();
        }

        let entry = store.get(&task.id).unwrap();
        assert!(entry.output.len() <= 256);
        assert!(entry.output_truncated);
    }

    fn task_to_json_for_store(store: &TaskStore, entry: &TaskEntry) -> Value {
        let blocked_dependencies = store.blocked_dependencies(entry);
        json!({
            "id": entry.id,
            "kind": entry.kind,
            "subject": entry.subject,
            "description": entry.description,
            "status": entry.status.as_str(),
            "created_at": entry.created_at,
            "updated_at": entry.updated_at,
            "parent_id": entry.parent_id,
            "depends_on": entry.depends_on,
            "blocked_dependencies": blocked_dependencies,
            "output_summary": entry.output_summary,
            "output_bytes": entry.output_bytes,
            "output_truncated": entry.output_truncated,
            "cancel_requested_at": entry.cancel_requested_at,
            "recovered_at": entry.recovered_at,
            "previous_status": entry.previous_status.map(|s| s.as_str()),
            "has_runtime_handle": store.has_runtime_handle(&entry.id),
        })
    }
}
