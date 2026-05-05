//! Task management tools: create, get, update, list, stop, and output.
//!
//! Tool tasks are persisted under the cc-rust data root (`~/.cc-rust/tasks`
//! by default, or `$CC_RUST_HOME/tasks`). `TaskStore` keeps a process-local
//! index for fast reads, while `TaskRepository` owns the versioned on-disk
//! schema, output retention, restart recovery, and lightweight migrations.
//!
//! Runtime cancellation handles are intentionally separate from persisted
//! task metadata: persisted records survive restart, cancellation tokens do
//! not. On startup, unfinished local tasks without a live supervisor are
//! migrated to `interrupted`; remote tasks keep enough identity to be marked
//! `recoverable` until a poller can reconnect or the user stops them.

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

const TASK_SCHEMA_VERSION: u32 = 4;
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const OUTPUT_SUMMARY_MAX_CHARS: usize = 2_000;
const DEFAULT_TASK_OUTPUT_TIMEOUT_MS: u64 = 30_000;
const MAX_TASK_OUTPUT_TIMEOUT_MS: u64 = 600_000;
const TASK_OUTPUT_POLL_INTERVAL_MS: u64 = 100;
const REMOTE_REVIEW_TIMEOUT_MS: i64 = 30 * 60 * 1000;
const TASK_KIND_TOOL: &str = "tool";
const TASK_KIND_LOCAL_BASH: &str = "local_bash";
const TASK_KIND_LOCAL_AGENT: &str = "local_agent";
const TASK_KIND_REMOTE_AGENT: &str = "remote_agent";
const TASK_KIND_IN_PROCESS_TEAMMATE: &str = "in_process_teammate";
const TASK_KIND_LOCAL_WORKFLOW: &str = "local_workflow";
const TASK_KIND_MONITOR_MCP: &str = "monitor_mcp";
const TASK_KIND_DREAM: &str = "dream";
const REMOTE_TASK_TYPE_REMOTE_AGENT: &str = "remote-agent";
const REMOTE_TASK_TYPE_ULTRAPLAN: &str = "ultraplan";
const REMOTE_TASK_TYPE_ULTRAREVIEW: &str = "ultrareview";
const REMOTE_TASK_TYPE_AUTOFIX_PR: &str = "autofix-pr";
const REMOTE_TASK_TYPE_BACKGROUND_PR: &str = "background-pr";
const TASK_CREATE_KIND_ENUM: &[&str] = &[
    TASK_KIND_TOOL,
    TASK_KIND_LOCAL_BASH,
    TASK_KIND_LOCAL_AGENT,
    TASK_KIND_REMOTE_AGENT,
    TASK_KIND_IN_PROCESS_TEAMMATE,
    TASK_KIND_LOCAL_WORKFLOW,
    TASK_KIND_MONITOR_MCP,
    TASK_KIND_DREAM,
];
const REMOTE_TASK_TYPE_ENUM: &[&str] = &[
    REMOTE_TASK_TYPE_REMOTE_AGENT,
    REMOTE_TASK_TYPE_ULTRAPLAN,
    REMOTE_TASK_TYPE_ULTRAREVIEW,
    REMOTE_TASK_TYPE_AUTOFIX_PR,
    REMOTE_TASK_TYPE_BACKGROUND_PR,
];

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
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub supervisor_id: Option<String>,
    pub isolation: Option<String>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub remote_task_type: Option<String>,
    pub remote_session_id: Option<String>,
    pub remote_task_metadata: Option<Value>,
    pub poll_started_at: Option<i64>,
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
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub supervisor_id: Option<String>,
    pub isolation: Option<String>,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub remote_task_type: Option<String>,
    pub remote_session_id: Option<String>,
    pub remote_task_metadata: Option<Value>,
    pub poll_started_at: Option<i64>,
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

    fn is_active_for_output_wait(self) -> bool {
        matches!(
            self,
            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Recoverable
        )
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
            tool_use_id: normalize_optional_string(options.tool_use_id),
            agent_id: normalize_optional_string(options.agent_id),
            supervisor_id: normalize_optional_string(options.supervisor_id),
            isolation: normalize_optional_string(options.isolation),
            worktree_path: normalize_optional_string(options.worktree_path),
            worktree_branch: normalize_optional_string(options.worktree_branch),
            remote_task_type: normalize_remote_task_type(options.remote_task_type),
            remote_session_id: normalize_optional_string(options.remote_session_id),
            remote_task_metadata: options
                .remote_task_metadata
                .filter(|value| !value.is_null()),
            poll_started_at: options.poll_started_at,
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
        self.refresh_remote_review_timeout(id)
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
        self.refresh_remote_review_timeouts();
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

    fn refresh_remote_review_timeouts(&self) {
        let ids: Vec<String> = self.tasks.lock().keys().cloned().collect();
        for id in ids {
            self.refresh_remote_review_timeout(&id);
        }
    }

    fn refresh_remote_review_timeout(&self, id: &str) -> Option<TaskEntry> {
        let now = chrono::Utc::now();
        let now_ms = now.timestamp_millis();
        let mut tasks = self.tasks.lock();
        let entry = tasks.get_mut(id)?;

        if !remote_review_timed_out(entry, now_ms) {
            return Some(entry.clone());
        }

        if entry.previous_status.is_none() {
            entry.previous_status = Some(entry.status);
        }
        entry.status = TaskStatus::Failed;
        entry.updated_at = now.timestamp();
        if entry.output.trim().is_empty() {
            entry.output =
                "Remote review did not produce output (remote session exceeded 30 minutes)."
                    .to_string();
        } else if !entry.output.contains("remote session exceeded 30 minutes") {
            entry
                .output
                .push_str("\nRemote review timed out after 30 minutes.");
        }
        refresh_output_metadata(entry);
        let cloned = entry.clone();
        drop(tasks);
        self.persist_entry(&cloned);
        Some(cloned)
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
        "tool_use_id": entry.tool_use_id,
        "agent_id": entry.agent_id,
        "supervisor_id": entry.supervisor_id,
        "isolation": entry.isolation,
        "worktree_path": entry.worktree_path,
        "worktree_branch": entry.worktree_branch,
        "remote_task_type": entry.remote_task_type,
        "remote_session_id": entry.remote_session_id,
        "remote_task_metadata": entry.remote_task_metadata,
        "poll_started_at": entry.poll_started_at,
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
    tool_use_id: Option<String>,
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
    remote_task_type: Option<String>,
    #[serde(default)]
    remote_session_id: Option<String>,
    #[serde(default)]
    remote_task_metadata: Option<Value>,
    #[serde(default)]
    poll_started_at: Option<i64>,
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
                        tool_use_id: None,
                        agent_id: None,
                        supervisor_id: None,
                        isolation: None,
                        worktree_path: None,
                        worktree_branch: None,
                        remote_task_type: None,
                        remote_session_id: None,
                        remote_task_metadata: None,
                        poll_started_at: None,
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
            tool_use_id: normalize_optional_string(record.tool_use_id),
            agent_id: normalize_optional_string(record.agent_id),
            supervisor_id: normalize_optional_string(record.supervisor_id),
            isolation: normalize_optional_string(record.isolation),
            worktree_path: normalize_optional_string(record.worktree_path),
            worktree_branch: normalize_optional_string(record.worktree_branch),
            remote_task_type: normalize_remote_task_type(record.remote_task_type),
            remote_session_id: normalize_optional_string(record.remote_session_id),
            remote_task_metadata: record.remote_task_metadata.filter(|value| !value.is_null()),
            poll_started_at: record.poll_started_at,
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
            tool_use_id: entry.tool_use_id.clone(),
            agent_id: entry.agent_id.clone(),
            supervisor_id: entry.supervisor_id.clone(),
            isolation: entry.isolation.clone(),
            worktree_path: entry.worktree_path.clone(),
            worktree_branch: entry.worktree_branch.clone(),
            remote_task_type: entry.remote_task_type.clone(),
            remote_session_id: entry.remote_session_id.clone(),
            remote_task_metadata: entry.remote_task_metadata.clone(),
            poll_started_at: entry.poll_started_at,
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
        let remote_recoverable = is_remote_recoverable_task(entry);
        let recovered_status = if remote_recoverable {
            TaskStatus::Recoverable
        } else {
            TaskStatus::Interrupted
        };
        let now = chrono::Utc::now();
        let mut recovered = false;

        if entry.status != recovered_status || entry.recovered_at.is_none() {
            if entry.status != recovered_status || entry.previous_status.is_none() {
                entry.previous_status = Some(entry.status);
            }
            entry.status = recovered_status;
            entry.recovered_at = Some(now.timestamp());
            recovered = true;
        }

        if remote_recoverable {
            let poll_started_at = now.timestamp_millis();
            if entry.poll_started_at != Some(poll_started_at) {
                entry.poll_started_at = Some(poll_started_at);
                recovered = true;
            }
        }

        if !recovered {
            return changed;
        }
        entry.updated_at = now.timestamp();
        return true;
    }

    changed
}

fn is_remote_recoverable_task(entry: &TaskEntry) -> bool {
    entry.kind == TASK_KIND_REMOTE_AGENT
        || entry.remote_session_id.is_some()
        || entry.remote_task_type.is_some()
}

fn is_remote_review_task(entry: &TaskEntry) -> bool {
    entry.remote_task_type.as_deref() == Some(REMOTE_TASK_TYPE_ULTRAREVIEW)
        || entry
            .remote_task_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("isRemoteReview"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn remote_review_timed_out(entry: &TaskEntry, now_ms: i64) -> bool {
    entry.status.is_active_for_output_wait()
        && is_remote_review_task(entry)
        && entry
            .poll_started_at
            .is_some_and(|started| now_ms.saturating_sub(started) > REMOTE_REVIEW_TIMEOUT_MS)
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

fn normalize_remote_task_type(value: Option<String>) -> Option<String> {
    let value = normalize_optional_string(value)?;
    let normalized = value.to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        REMOTE_TASK_TYPE_REMOTE_AGENT => Some(REMOTE_TASK_TYPE_REMOTE_AGENT.to_string()),
        REMOTE_TASK_TYPE_ULTRAPLAN => Some(REMOTE_TASK_TYPE_ULTRAPLAN.to_string()),
        REMOTE_TASK_TYPE_ULTRAREVIEW => Some(REMOTE_TASK_TYPE_ULTRAREVIEW.to_string()),
        REMOTE_TASK_TYPE_AUTOFIX_PR => Some(REMOTE_TASK_TYPE_AUTOFIX_PR.to_string()),
        REMOTE_TASK_TYPE_BACKGROUND_PR => Some(REMOTE_TASK_TYPE_BACKGROUND_PR.to_string()),
        _ => Some(value),
    }
}

fn sanitize_kind(kind: &str) -> String {
    let trimmed = kind.trim();
    if trimmed.is_empty() {
        return default_task_kind();
    }
    let sanitized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    match sanitized.to_ascii_lowercase().as_str() {
        "local_shell" | "local-bash" | "bash" => TASK_KIND_LOCAL_BASH.to_string(),
        "local-agent" => TASK_KIND_LOCAL_AGENT.to_string(),
        "remote-agent" => TASK_KIND_REMOTE_AGENT.to_string(),
        "teammate" | "team" | "in-process-teammate" => TASK_KIND_IN_PROCESS_TEAMMATE.to_string(),
        "workflow" | "local-workflow" => TASK_KIND_LOCAL_WORKFLOW.to_string(),
        "monitor" | "monitor-mcp" => TASK_KIND_MONITOR_MCP.to_string(),
        "dream" => TASK_KIND_DREAM.to_string(),
        "tool" => TASK_KIND_TOOL.to_string(),
        _ => sanitized,
    }
}

fn default_task_kind() -> String {
    TASK_KIND_TOOL.to_string()
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

fn plan_workflow_cwd() -> PathBuf {
    let cwd = crate::bootstrap::state::original_cwd();
    if !cwd.as_os_str().is_empty() {
        return cwd;
    }

    let fallback = std::env::temp_dir().join("cc-rust-plan-workflow");
    let _ = std::fs::create_dir_all(fallback.join(".cc-rust"));
    fallback
}

fn maybe_link_plan_workflow_task(
    ctx: &ToolUseContext,
    entry: &TaskEntry,
) -> Result<Option<crate::plan_workflow::PlanWorkflowRecord>> {
    let cwd = plan_workflow_cwd();
    let existing = match crate::plan_workflow::load(&cwd) {
        Ok(record) => record,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "failed to load plan workflow for task link; continuing without link"
            );
            None
        }
    };
    let persist_cwd = cwd.clone();
    let task_id = entry.id.clone();
    let summary = Some(entry.subject.clone());
    let slot: Arc<Mutex<Option<Option<crate::plan_workflow::PlanWorkflowRecord>>>> =
        Arc::new(Mutex::new(None));
    let slot_for_update = Arc::clone(&slot);

    (ctx.set_app_state)(Box::new(move |mut state| {
        let linked = crate::plan_workflow::maybe_link_implementation_task_state(
            &mut state,
            &cwd,
            existing,
            "main",
            "task_create",
            task_id,
            summary,
        );
        *slot_for_update.lock() = Some(linked);
        state
    }));

    let record = slot.lock().clone().unwrap_or(None);
    if let Some(record) = &record {
        crate::plan_workflow::persist(&persist_cwd, record)?;
    }
    Ok(record)
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
                    "description": "Stable task type for persisted records; legacy aliases are accepted and normalized",
                    "enum": TASK_CREATE_KIND_ENUM
                },
                "parent_id": {
                    "type": "string",
                    "description": "Optional parent task ID"
                },
                "depends_on": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that should complete before this task"
                },
                "tool_use_id": {
                    "type": "string",
                    "description": "Optional upstream tool use ID associated with this task"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Optional agent ID associated with this task"
                },
                "supervisor_id": {
                    "type": "string",
                    "description": "Optional runtime supervisor ID for this task"
                },
                "isolation": {
                    "type": "string",
                    "description": "Optional runtime isolation label, such as worktree"
                },
                "worktree_path": {
                    "type": "string",
                    "description": "Optional worktree path for isolated task execution"
                },
                "worktree_branch": {
                    "type": "string",
                    "description": "Optional worktree branch for isolated task execution"
                },
                "remote_task_type": {
                    "type": "string",
                    "enum": REMOTE_TASK_TYPE_ENUM,
                    "description": "Optional remote task subtype used by remote-agent supervisors"
                },
                "remote_session_id": {
                    "type": "string",
                    "description": "Optional remote session ID used to restore or poll a remote task"
                },
                "remote_task_metadata": {
                    "type": "object",
                    "description": "Optional remote task metadata, such as repository or pull request identifiers"
                },
                "poll_started_at": {
                    "type": "integer",
                    "description": "Optional remote poll start timestamp in milliseconds since epoch"
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
        let tool_use_id = input
            .get("tool_use_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let agent_id = input
            .get("agent_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let supervisor_id = input
            .get("supervisor_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let isolation = input
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let worktree_path = input
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let worktree_branch = input
            .get("worktree_branch")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let remote_task_type = input
            .get("remote_task_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let remote_session_id = input
            .get("remote_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let remote_task_metadata = input
            .get("remote_task_metadata")
            .filter(|v| v.is_object())
            .cloned();
        let poll_started_at = input.get("poll_started_at").and_then(|v| v.as_i64());
        let has_options = kind.is_some()
            || parent_id.is_some()
            || !depends_on.is_empty()
            || tool_use_id.is_some()
            || agent_id.is_some()
            || supervisor_id.is_some()
            || isolation.is_some()
            || worktree_path.is_some()
            || worktree_branch.is_some()
            || remote_task_type.is_some()
            || remote_session_id.is_some()
            || remote_task_metadata.is_some()
            || poll_started_at.is_some();

        let entry = if has_options {
            store().create_with_options(
                subject,
                description,
                TaskCreateOptions {
                    kind,
                    parent_id,
                    depends_on,
                    tool_use_id,
                    agent_id,
                    supervisor_id,
                    isolation,
                    worktree_path,
                    worktree_branch,
                    remote_task_type,
                    remote_session_id,
                    remote_task_metadata,
                    poll_started_at,
                    ..TaskCreateOptions::default()
                },
            )
        } else {
            store().create(subject, description)
        };

        let linked_plan_workflow = maybe_link_plan_workflow_task(ctx, &entry)?;

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

        let mut data = json!({
            "task": task_to_json(&entry),
            "message": format!("Created task: {}", entry.subject)
        });
        if let Some(record) = linked_plan_workflow {
            if let Some(map) = data.as_object_mut() {
                map.insert("plan_workflow".to_string(), json!(record));
            }
        }

        Ok(ToolResult {
            data,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskOutputRetrievalStatus {
    Success,
    Timeout,
    NotReady,
}

impl TaskOutputRetrievalStatus {
    fn as_str(self) -> &'static str {
        match self {
            TaskOutputRetrievalStatus::Success => "success",
            TaskOutputRetrievalStatus::Timeout => "timeout",
            TaskOutputRetrievalStatus::NotReady => "not_ready",
        }
    }
}

#[derive(Debug)]
enum TaskOutputWaitResult {
    Ready(TaskEntry),
    TimedOut(Option<TaskEntry>),
}

fn parse_task_output_timeout_ms(input: &Value) -> Result<u64> {
    let Some(timeout) = input.get("timeout") else {
        return Ok(DEFAULT_TASK_OUTPUT_TIMEOUT_MS);
    };
    let value = if let Some(value) = timeout.as_u64() {
        value
    } else if let Some(value) = timeout.as_f64() {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
            anyhow::bail!("timeout must be an integer between 0 and {MAX_TASK_OUTPUT_TIMEOUT_MS}");
        }
        value as u64
    } else {
        anyhow::bail!("timeout must be an integer between 0 and {MAX_TASK_OUTPUT_TIMEOUT_MS}");
    };

    if value > MAX_TASK_OUTPUT_TIMEOUT_MS {
        anyhow::bail!("timeout must be between 0 and {MAX_TASK_OUTPUT_TIMEOUT_MS} milliseconds");
    }

    Ok(value)
}

fn task_output_payload(entry: &TaskEntry, retrieval_status: TaskOutputRetrievalStatus) -> Value {
    let output = if entry.output.is_empty() {
        "(no output yet)".to_string()
    } else {
        entry.output.clone()
    };
    let task = json!({
        "task_id": entry.id,
        "task_type": entry.kind,
        "status": entry.status.as_str(),
        "description": entry.description,
        "output": output.clone(),
        "subject": entry.subject,
        "tool_use_id": entry.tool_use_id,
        "agent_id": entry.agent_id,
        "supervisor_id": entry.supervisor_id,
        "isolation": entry.isolation,
        "worktree_path": entry.worktree_path,
        "worktree_branch": entry.worktree_branch,
        "remote_task_type": entry.remote_task_type,
        "remote_session_id": entry.remote_session_id,
        "remote_task_metadata": entry.remote_task_metadata,
        "poll_started_at": entry.poll_started_at,
        "output_summary": entry.output_summary,
        "output_bytes": entry.output_bytes,
        "output_truncated": entry.output_truncated,
    });

    json!({
        "retrieval_status": retrieval_status.as_str(),
        "task": task,
        // Legacy flat fields remain for existing callers.
        "task_id": entry.id,
        "subject": entry.subject,
        "tool_use_id": entry.tool_use_id,
        "agent_id": entry.agent_id,
        "supervisor_id": entry.supervisor_id,
        "isolation": entry.isolation,
        "worktree_path": entry.worktree_path,
        "worktree_branch": entry.worktree_branch,
        "remote_task_type": entry.remote_task_type,
        "remote_session_id": entry.remote_session_id,
        "remote_task_metadata": entry.remote_task_metadata,
        "poll_started_at": entry.poll_started_at,
        "output": output,
        "output_summary": entry.output_summary,
        "output_bytes": entry.output_bytes,
        "output_truncated": entry.output_truncated,
    })
}

async fn wait_for_task_output(
    task_store: TaskStore,
    task_id: &str,
    timeout_ms: u64,
    mut abort_signal: tokio::sync::watch::Receiver<bool>,
) -> Result<TaskOutputWaitResult> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let entry = task_store.get(task_id);
        if let Some(entry) = &entry {
            if !entry.status.is_active_for_output_wait() {
                return Ok(TaskOutputWaitResult::Ready(entry.clone()));
            }
        } else {
            return Ok(TaskOutputWaitResult::TimedOut(None));
        }

        let now = Instant::now();
        if timeout_ms == 0 || now >= deadline {
            return Ok(TaskOutputWaitResult::TimedOut(entry));
        }

        let remaining = deadline.saturating_duration_since(now);
        let delay = remaining.min(Duration::from_millis(TASK_OUTPUT_POLL_INTERVAL_MS));
        tokio::select! {
            changed = abort_signal.changed() => {
                if changed.is_ok() && *abort_signal.borrow() {
                    anyhow::bail!("TaskOutput wait aborted");
                }
                if changed.is_err() {
                    sleep(delay).await;
                }
            }
            _ = sleep(delay) => {}
        }
    }
}

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
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to wait for the task to leave pending/running state",
                    "default": true
                },
                "timeout": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": MAX_TASK_OUTPUT_TIMEOUT_MS,
                    "description": "Maximum wait time in milliseconds when block=true",
                    "default": DEFAULT_TASK_OUTPUT_TIMEOUT_MS
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
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let block = input.get("block").and_then(|v| v.as_bool()).unwrap_or(true);
        let timeout_ms = parse_task_output_timeout_ms(&input)?;

        let initial = store().get(id);
        match initial {
            Some(entry) => Ok(ToolResult {
                data: if !block {
                    let retrieval_status = if entry.status.is_active_for_output_wait() {
                        TaskOutputRetrievalStatus::NotReady
                    } else {
                        TaskOutputRetrievalStatus::Success
                    };
                    task_output_payload(&entry, retrieval_status)
                } else if !entry.status.is_active_for_output_wait() {
                    task_output_payload(&entry, TaskOutputRetrievalStatus::Success)
                } else {
                    match wait_for_task_output(
                        store().clone(),
                        id,
                        timeout_ms,
                        ctx.abort_signal.clone(),
                    )
                    .await?
                    {
                        TaskOutputWaitResult::Ready(entry) => {
                            task_output_payload(&entry, TaskOutputRetrievalStatus::Success)
                        }
                        TaskOutputWaitResult::TimedOut(Some(entry)) => {
                            task_output_payload(&entry, TaskOutputRetrievalStatus::Timeout)
                        }
                        TaskOutputWaitResult::TimedOut(None) => json!({
                            "retrieval_status": TaskOutputRetrievalStatus::Timeout.as_str(),
                            "task": null,
                            "error": format!("Task not found: {}", id),
                        }),
                    }
                },
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
    fn test_remote_task_metadata_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let task = store.create_with_options(
            "remote review",
            "poll remote session",
            TaskCreateOptions {
                kind: Some("remote-agent".to_string()),
                tool_use_id: Some("toolu_123".to_string()),
                remote_task_type: Some("remote_agent".to_string()),
                remote_session_id: Some("session-123".to_string()),
                remote_task_metadata: Some(json!({
                    "owner": "acme",
                    "repo": "widget",
                    "prNumber": 42
                })),
                poll_started_at: Some(1_714_000_000_000),
                ..TaskCreateOptions::default()
            },
        );

        assert_eq!(task.kind, "remote_agent");
        assert_eq!(task.tool_use_id.as_deref(), Some("toolu_123"));
        assert_eq!(task.remote_task_type.as_deref(), Some("remote-agent"));
        assert_eq!(task.remote_session_id.as_deref(), Some("session-123"));
        assert_eq!(task.remote_task_metadata.as_ref().unwrap()["prNumber"], 42);
        assert_eq!(task.poll_started_at, Some(1_714_000_000_000));
        store.update_status(&task.id, TaskStatus::Completed);

        let restarted = TaskStore::with_dir(tmp.path());
        let restored = restarted.get(&task.id).unwrap();
        assert_eq!(restored.status, TaskStatus::Completed);
        assert_eq!(restored.kind, "remote_agent");
        assert_eq!(restored.tool_use_id.as_deref(), Some("toolu_123"));
        assert_eq!(restored.remote_task_type.as_deref(), Some("remote-agent"));
        assert_eq!(restored.remote_session_id.as_deref(), Some("session-123"));
        assert_eq!(
            restored.remote_task_metadata.as_ref().unwrap()["repo"],
            "widget"
        );
        assert_eq!(restored.poll_started_at, Some(1_714_000_000_000));
    }

    #[test]
    fn test_restart_marks_remote_tasks_recoverable_and_resets_poll_timer() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let task = store.create_with_options(
            "remote agent",
            "resume remote session",
            TaskCreateOptions {
                kind: Some("remote_agent".to_string()),
                remote_task_type: Some("ultrareview".to_string()),
                remote_session_id: Some("session-restore".to_string()),
                remote_task_metadata: Some(json!({
                    "owner": "acme",
                    "repo": "widget",
                    "prNumber": 7
                })),
                poll_started_at: Some(1_714_000_000_000),
                ..TaskCreateOptions::default()
            },
        );
        store.update_status(&task.id, TaskStatus::InProgress);

        let restarted = TaskStore::with_dir(tmp.path());
        let restored = restarted.get(&task.id).unwrap();

        assert_eq!(restored.status, TaskStatus::Recoverable);
        assert_eq!(restored.previous_status, Some(TaskStatus::InProgress));
        assert!(restored.recovered_at.is_some());
        assert_eq!(restored.remote_task_type.as_deref(), Some("ultrareview"));
        assert_eq!(
            restored.remote_session_id.as_deref(),
            Some("session-restore")
        );
        assert_eq!(
            restored.remote_task_metadata.as_ref().unwrap()["prNumber"],
            7
        );
        let poll_started_at = restored.poll_started_at.unwrap();
        assert!(poll_started_at > 1_714_000_000_000);

        let previous_recovered_at = restored.recovered_at;
        let restarted_again = TaskStore::with_dir(tmp.path());
        let restored_again = restarted_again.get(&task.id).unwrap();
        assert_eq!(restored_again.status, TaskStatus::Recoverable);
        assert_eq!(restored_again.previous_status, Some(TaskStatus::InProgress));
        assert_eq!(restored_again.recovered_at, previous_recovered_at);
        assert!(restored_again.poll_started_at.unwrap() >= poll_started_at);
    }

    #[test]
    fn test_remote_review_timeout_marks_task_failed_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let stale_poll_started_at =
            chrono::Utc::now().timestamp_millis() - REMOTE_REVIEW_TIMEOUT_MS - 1;
        let task = store.create_with_options(
            "remote review",
            "wait for remote review output",
            TaskCreateOptions {
                kind: Some("remote_agent".to_string()),
                remote_task_type: Some("ultrareview".to_string()),
                remote_session_id: Some("session-timeout".to_string()),
                poll_started_at: Some(stale_poll_started_at),
                ..TaskCreateOptions::default()
            },
        );
        store.update_status(&task.id, TaskStatus::InProgress);

        let timed_out = store.get(&task.id).unwrap();
        assert_eq!(timed_out.status, TaskStatus::Failed);
        assert_eq!(timed_out.previous_status, Some(TaskStatus::InProgress));
        assert!(timed_out
            .output
            .contains("remote session exceeded 30 minutes"));

        let restarted = TaskStore::with_dir(tmp.path());
        let restored = restarted.get(&task.id).unwrap();
        assert_eq!(restored.status, TaskStatus::Failed);
        assert!(restored
            .output
            .contains("remote session exceeded 30 minutes"));
    }

    #[test]
    fn test_remote_review_timeout_does_not_apply_to_generic_remote_agents() {
        let (_tmp, store) = temp_store();
        let stale_poll_started_at =
            chrono::Utc::now().timestamp_millis() - REMOTE_REVIEW_TIMEOUT_MS - 1;
        let task = store.create_with_options(
            "remote agent",
            "wait for generic remote agent",
            TaskCreateOptions {
                kind: Some("remote_agent".to_string()),
                remote_task_type: Some("remote-agent".to_string()),
                remote_session_id: Some("session-generic".to_string()),
                poll_started_at: Some(stale_poll_started_at),
                ..TaskCreateOptions::default()
            },
        );
        store.update_status(&task.id, TaskStatus::InProgress);

        let still_running = store.get(&task.id).unwrap();
        assert_eq!(still_running.status, TaskStatus::InProgress);
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
    fn test_task_create_schema_uses_upstream_task_type_taxonomy() {
        let schema = TaskCreateTool.input_json_schema();
        let variants: Vec<&str> = schema["properties"]["kind"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            variants,
            vec![
                "tool",
                "local_bash",
                "local_agent",
                "remote_agent",
                "in_process_teammate",
                "local_workflow",
                "monitor_mcp",
                "dream",
            ]
        );
        assert!(!variants.contains(&"local_shell"));
        assert!(!variants.contains(&"workflow"));
        assert!(!variants.contains(&"team"));
    }

    #[test]
    fn test_task_kind_aliases_canonicalize_to_upstream_types() {
        assert_eq!(sanitize_kind(" local_shell "), "local_bash");
        assert_eq!(sanitize_kind("local-bash"), "local_bash");
        assert_eq!(sanitize_kind("remote-agent"), "remote_agent");
        assert_eq!(sanitize_kind("team"), "in_process_teammate");
        assert_eq!(sanitize_kind("workflow"), "local_workflow");
        assert_eq!(sanitize_kind("monitor"), "monitor_mcp");
        assert_eq!(sanitize_kind("custom kind"), "custom_kind");
    }

    #[test]
    fn test_remote_task_type_aliases_canonicalize_to_upstream_values() {
        assert_eq!(
            normalize_remote_task_type(Some("remote_agent".to_string())).as_deref(),
            Some("remote-agent")
        );
        assert_eq!(
            normalize_remote_task_type(Some("autofix_pr".to_string())).as_deref(),
            Some("autofix-pr")
        );
        assert_eq!(
            normalize_remote_task_type(Some("background-pr".to_string())).as_deref(),
            Some("background-pr")
        );
        assert_eq!(
            normalize_remote_task_type(Some("custom-remote".to_string())).as_deref(),
            Some("custom-remote")
        );
        assert_eq!(normalize_remote_task_type(Some(" ".to_string())), None);
    }

    #[test]
    fn test_task_store_persists_canonicalized_task_kinds() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let shell = store.create_with_options(
            "shell",
            "",
            TaskCreateOptions {
                kind: Some("local_shell".to_string()),
                ..TaskCreateOptions::default()
            },
        );
        let workflow = store.create_with_options(
            "workflow",
            "",
            TaskCreateOptions {
                kind: Some("workflow".to_string()),
                ..TaskCreateOptions::default()
            },
        );

        assert_eq!(shell.kind, "local_bash");
        assert_eq!(workflow.kind, "local_workflow");

        let restarted = TaskStore::with_dir(tmp.path());
        assert_eq!(restarted.get(&shell.id).unwrap().kind, "local_bash");
        assert_eq!(restarted.get(&workflow.id).unwrap().kind, "local_workflow");
    }

    #[test]
    fn test_task_create_schema_exposes_supervisor_metadata_fields() {
        let schema = TaskCreateTool.input_json_schema();
        let props = &schema["properties"];
        for field in [
            "tool_use_id",
            "agent_id",
            "supervisor_id",
            "remote_task_type",
            "remote_session_id",
            "remote_task_metadata",
            "poll_started_at",
        ] {
            assert!(props.get(field).is_some(), "schema should expose {field}");
        }
        let remote_types: Vec<&str> = props["remote_task_type"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            remote_types,
            vec![
                "remote-agent",
                "ultraplan",
                "ultrareview",
                "autofix-pr",
                "background-pr",
            ]
        );
    }

    #[test]
    fn test_task_output_schema_exposes_block_timeout_controls() {
        let schema = TaskOutputTool.input_json_schema();
        let props = &schema["properties"];
        assert_eq!(props["block"]["type"], "boolean");
        assert_eq!(props["block"]["default"], true);
        assert_eq!(props["timeout"]["type"], "integer");
        assert_eq!(props["timeout"]["minimum"], 0);
        assert_eq!(props["timeout"]["maximum"], MAX_TASK_OUTPUT_TIMEOUT_MS);
        assert_eq!(props["timeout"]["default"], DEFAULT_TASK_OUTPUT_TIMEOUT_MS);
    }

    #[test]
    fn test_task_output_timeout_validation_matches_upstream_bounds() {
        assert_eq!(parse_task_output_timeout_ms(&json!({})).unwrap(), 30_000);
        assert_eq!(
            parse_task_output_timeout_ms(&json!({ "timeout": 600_000 })).unwrap(),
            600_000
        );
        assert!(parse_task_output_timeout_ms(&json!({ "timeout": 600_001 })).is_err());
        assert!(parse_task_output_timeout_ms(&json!({ "timeout": -1 })).is_err());
        assert!(parse_task_output_timeout_ms(&json!({ "timeout": 1.5 })).is_err());
    }

    #[test]
    fn test_task_output_payload_preserves_legacy_fields_and_status() {
        let (_tmp, store) = temp_store();
        let task = store.create_with_options(
            "agent task",
            "collect output",
            TaskCreateOptions {
                kind: Some("local_agent".to_string()),
                agent_id: Some("agent-1".to_string()),
                supervisor_id: Some("supervisor-1".to_string()),
                ..TaskCreateOptions::default()
            },
        );
        store.update_status(&task.id, TaskStatus::InProgress);
        store.append_output(&task.id, "partial output");
        let entry = store.get(&task.id).unwrap();

        let payload = task_output_payload(&entry, TaskOutputRetrievalStatus::NotReady);
        assert_eq!(payload["retrieval_status"], "not_ready");
        assert_eq!(payload["task"]["task_id"], task.id);
        assert_eq!(payload["task"]["task_type"], "local_agent");
        assert_eq!(payload["task"]["status"], "in_progress");
        assert_eq!(payload["task"]["output"], "partial output");
        assert_eq!(payload["task_id"], task.id);
        assert_eq!(payload["output"], "partial output");
        assert_eq!(payload["agent_id"], "agent-1");
    }

    #[tokio::test]
    async fn test_task_output_treats_recoverable_as_active_wait_state() {
        let (_tmp, store) = temp_store();
        let task = store.create("remote", "");
        store.update_status(&task.id, TaskStatus::Recoverable);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = wait_for_task_output(store, &task.id, 0, rx).await.unwrap();
        match result {
            TaskOutputWaitResult::TimedOut(Some(entry)) => {
                assert_eq!(entry.status, TaskStatus::Recoverable);
            }
            other => panic!("expected timeout with recoverable task, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_wait_for_task_output_times_out_active_task() {
        let (_tmp, store) = temp_store();
        let task = store.create("running", "");
        store.update_status(&task.id, TaskStatus::InProgress);
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = wait_for_task_output(store, &task.id, 0, rx).await.unwrap();
        match result {
            TaskOutputWaitResult::TimedOut(Some(entry)) => {
                assert_eq!(entry.status, TaskStatus::InProgress);
            }
            other => panic!("expected timeout with current task, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_wait_for_task_output_observes_completion() {
        let (_tmp, store) = temp_store();
        let task = store.create("running", "");
        store.update_status(&task.id, TaskStatus::InProgress);
        let writer = store.clone();
        let task_id = task.id.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(20)).await;
            writer.append_output(&task_id, "done");
            writer.update_status(&task_id, TaskStatus::Completed);
        });
        let (_tx, rx) = tokio::sync::watch::channel(false);

        let result = wait_for_task_output(store, &task.id, 1_000, rx)
            .await
            .unwrap();
        match result {
            TaskOutputWaitResult::Ready(entry) => {
                assert_eq!(entry.status, TaskStatus::Completed);
                assert_eq!(entry.output, "done");
            }
            other => panic!("expected completed task, got {other:?}"),
        }
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
            "tool_use_id": entry.tool_use_id,
            "agent_id": entry.agent_id,
            "supervisor_id": entry.supervisor_id,
            "isolation": entry.isolation,
            "worktree_path": entry.worktree_path,
            "worktree_branch": entry.worktree_branch,
            "remote_task_type": entry.remote_task_type,
            "remote_session_id": entry.remote_session_id,
            "remote_task_metadata": entry.remote_task_metadata,
            "poll_started_at": entry.poll_started_at,
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
