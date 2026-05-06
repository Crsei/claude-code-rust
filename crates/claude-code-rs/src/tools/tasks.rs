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

const TASK_SCHEMA_VERSION: u32 = 5;
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const OUTPUT_SUMMARY_MAX_CHARS: usize = 2_000;
const DEFAULT_TASK_OUTPUT_TIMEOUT_MS: u64 = 30_000;
const MAX_TASK_OUTPUT_TIMEOUT_MS: u64 = 600_000;
const TASK_OUTPUT_POLL_INTERVAL_MS: u64 = 100;
const REMOTE_REVIEW_TIMEOUT_MS: i64 = 30 * 60 * 1000;
const TASK_HIGHWATERMARK_FILE: &str = ".highwatermark";
const TASK_HIGHWATERMARK_LOCK_FILE: &str = ".highwatermark.lock";
const TASK_HIGHWATERMARK_LOCK_RETRIES: usize = 30;
const TASK_LIST_LOCK_FILE: &str = ".lock";
#[cfg(not(test))]
const TASK_LIST_LOCK_RETRIES: usize = 30;
#[cfg(test)]
const TASK_LIST_LOCK_RETRIES: usize = 3;
const DEFAULT_TASK_LIST_ID: &str = "tasklist";
const CC_RUST_TASK_LIST_ID_ENV: &str = "CC_RUST_TASK_LIST_ID";
const CLAUDE_CODE_TASK_LIST_ID_ENV: &str = "CLAUDE_CODE_TASK_LIST_ID";
const CLAUDE_CODE_TEAM_NAME_ENV: &str = "CLAUDE_CODE_TEAM_NAME";
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
    pub owner: Option<String>,
    pub active_form: Option<String>,
    pub metadata: Option<Value>,
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

#[derive(Debug, Clone, Default)]
struct TaskUpdateFields {
    subject: Option<String>,
    description: Option<String>,
    active_form: Option<Option<String>>,
    owner: Option<Option<String>>,
    metadata_patch: Option<Value>,
    status: Option<TaskStatus>,
    add_blocks: Vec<String>,
    add_blocked_by: Vec<String>,
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
    pub owner: Option<String>,
    pub active_form: Option<String>,
    pub metadata: Option<Value>,
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

    fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Interrupted
                | TaskStatus::Stopped
        )
    }

    fn is_active_for_output_wait(self) -> bool {
        matches!(
            self,
            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Recoverable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskClaimFailureReason {
    TaskNotFound,
    AlreadyClaimed,
    AlreadyResolved,
    Blocked,
    AgentBusy,
    OwnerRequired,
    LockUnavailable,
}

impl TaskClaimFailureReason {
    fn as_str(self) -> &'static str {
        match self {
            TaskClaimFailureReason::TaskNotFound => "task_not_found",
            TaskClaimFailureReason::AlreadyClaimed => "already_claimed",
            TaskClaimFailureReason::AlreadyResolved => "already_resolved",
            TaskClaimFailureReason::Blocked => "blocked",
            TaskClaimFailureReason::AgentBusy => "agent_busy",
            TaskClaimFailureReason::OwnerRequired => "owner_required",
            TaskClaimFailureReason::LockUnavailable => "lock_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskClaimFailure {
    reason: TaskClaimFailureReason,
    owner: Option<String>,
    blocked_by: Vec<String>,
    busy_task_id: Option<String>,
}

impl TaskClaimFailure {
    fn new(reason: TaskClaimFailureReason) -> Self {
        Self {
            reason,
            owner: None,
            blocked_by: Vec::new(),
            busy_task_id: None,
        }
    }

    fn with_owner(mut self, owner: String) -> Self {
        self.owner = Some(owner);
        self
    }

    fn with_blocked_by(mut self, blocked_by: Vec<String>) -> Self {
        self.blocked_by = blocked_by;
        self
    }

    fn with_busy_task_id(mut self, busy_task_id: String) -> Self {
        self.busy_task_id = Some(busy_task_id);
        self
    }

    fn to_json(&self) -> Value {
        json!({
            "reason": self.reason.as_str(),
            "owner": self.owner.clone(),
            "blocked_by": self.blocked_by.clone(),
            "blockedBy": self.blocked_by.clone(),
            "busy_task_id": self.busy_task_id.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TodoItem {
    content: String,
    status: String,
    #[serde(
        rename = "activeForm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    active_form: Option<String>,
}

#[derive(Debug, Clone)]
struct TodoWriteOutcome {
    todos: Vec<TodoItem>,
    cleared: bool,
    verification_nudge_needed: bool,
}

impl TaskStore {
    pub fn new() -> Self {
        Self::with_dir(task_list_dir(DEFAULT_TASK_LIST_ID))
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
        let _guard = self.acquire_task_list_lock("create task").ok();
        let mut tasks = if _guard.is_some() {
            self.load_repository_tasks()
        } else {
            self.tasks.lock().clone()
        };
        let now = chrono::Utc::now().timestamp();
        let id = match self.repository.reserve_next_task_id(&tasks) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to reserve incremental task id; falling back to uuid"
                );
                fallback_task_id(&tasks)
            }
        };
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
            owner: normalize_optional_string(options.owner),
            active_form: normalize_optional_string(options.active_form),
            metadata: options.metadata.filter(|value| !value.is_null()),
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

        tasks.insert(id, entry.clone());
        self.persist_entry(&entry);
        self.replace_tasks(tasks);
        entry
    }

    pub fn get(&self, id: &str) -> Option<TaskEntry> {
        self.refresh_from_repository();
        self.refresh_remote_review_timeout(id)
    }

    pub fn update_status(&self, id: &str, status: TaskStatus) -> Option<TaskEntry> {
        let _guard = self.acquire_task_list_lock("update task status").ok();
        let mut tasks = if _guard.is_some() {
            self.load_repository_tasks()
        } else {
            self.tasks.lock().clone()
        };
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = normalize_new_status(status);
            entry.updated_at = chrono::Utc::now().timestamp();
            if entry.status == TaskStatus::Cancelled && entry.cancel_requested_at.is_none() {
                entry.cancel_requested_at = Some(entry.updated_at);
            }
            let cloned = entry.clone();
            self.persist_entry(&cloned);
            self.replace_tasks(tasks);
            Some(cloned)
        } else {
            self.replace_tasks(tasks);
            None
        }
    }

    fn update_fields(&self, id: &str, updates: TaskUpdateFields) -> Option<TaskEntry> {
        let _guard = self.acquire_task_list_lock("update task fields").ok();
        let mut tasks = if _guard.is_some() {
            self.load_repository_tasks()
        } else {
            self.tasks.lock().clone()
        };
        let now = chrono::Utc::now().timestamp();
        let mut changed_entries = Vec::new();
        let updated = {
            let entry = tasks.get_mut(id)?;
            if let Some(subject) = updates.subject {
                entry.subject = subject;
            }
            if let Some(description) = updates.description {
                entry.description = description;
            }
            if let Some(active_form) = updates.active_form {
                entry.active_form = active_form;
            }
            if let Some(owner) = updates.owner {
                entry.owner = owner;
            }
            if let Some(metadata_patch) = updates.metadata_patch {
                entry.metadata = merge_metadata(entry.metadata.clone(), &metadata_patch);
            }
            if let Some(status) = updates.status {
                entry.status = normalize_new_status(status);
                if entry.status == TaskStatus::Cancelled && entry.cancel_requested_at.is_none() {
                    entry.cancel_requested_at = Some(now);
                }
            }
            for dependency_id in normalize_dependencies(updates.add_blocked_by) {
                if dependency_id != entry.id
                    && !entry.depends_on.iter().any(|id| id == &dependency_id)
                {
                    entry.depends_on.push(dependency_id);
                }
            }
            entry.depends_on = normalize_dependencies(std::mem::take(&mut entry.depends_on));
            entry.updated_at = now;
            entry.clone()
        };

        for blocked_task_id in normalize_dependencies(updates.add_blocks) {
            if blocked_task_id == id {
                continue;
            }
            if let Some(blocked_task) = tasks.get_mut(&blocked_task_id) {
                if !blocked_task.depends_on.iter().any(|dep_id| dep_id == id) {
                    blocked_task.depends_on.push(id.to_string());
                    blocked_task.depends_on =
                        normalize_dependencies(std::mem::take(&mut blocked_task.depends_on));
                    blocked_task.updated_at = now;
                    changed_entries.push(blocked_task.clone());
                }
            }
        }

        changed_entries.push(updated.clone());
        for entry in changed_entries {
            self.persist_entry(&entry);
        }
        self.replace_tasks(tasks);
        Some(updated)
    }

    fn claim_task(
        &self,
        id: &str,
        owner: &str,
        check_agent_busy: bool,
    ) -> std::result::Result<TaskEntry, TaskClaimFailure> {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err(TaskClaimFailure::new(TaskClaimFailureReason::OwnerRequired));
        }

        let _guard = self.acquire_task_list_lock("claim task").map_err(|err| {
            tracing::warn!(
                task_id = id,
                owner,
                error = %err,
                "failed to acquire task-list lock for claim"
            );
            TaskClaimFailure::new(TaskClaimFailureReason::LockUnavailable)
        })?;
        let mut tasks = self.load_repository_tasks();
        let Some(snapshot) = tasks.get(id).cloned() else {
            self.replace_tasks(tasks);
            return Err(TaskClaimFailure::new(TaskClaimFailureReason::TaskNotFound));
        };

        if snapshot.status.is_terminal() {
            self.replace_tasks(tasks);
            return Err(TaskClaimFailure::new(
                TaskClaimFailureReason::AlreadyResolved,
            ));
        }

        if let Some(existing_owner) = snapshot.owner.as_deref() {
            if existing_owner != owner {
                self.replace_tasks(tasks);
                return Err(
                    TaskClaimFailure::new(TaskClaimFailureReason::AlreadyClaimed)
                        .with_owner(existing_owner.to_string()),
                );
            }
        }

        let blocked_by = blocked_dependencies_with_tasks(&tasks, &snapshot);
        if !blocked_by.is_empty() {
            self.replace_tasks(tasks);
            return Err(
                TaskClaimFailure::new(TaskClaimFailureReason::Blocked).with_blocked_by(blocked_by)
            );
        }

        if check_agent_busy {
            if let Some(busy_task_id) = tasks
                .values()
                .find(|candidate| {
                    candidate.id != snapshot.id
                        && candidate.owner.as_deref() == Some(owner)
                        && !candidate.status.is_terminal()
                })
                .map(|candidate| candidate.id.clone())
            {
                self.replace_tasks(tasks);
                return Err(TaskClaimFailure::new(TaskClaimFailureReason::AgentBusy)
                    .with_busy_task_id(busy_task_id));
            }
        }

        let entry = tasks.get_mut(id).expect("snapshot came from task map");
        entry.owner = Some(owner.to_string());
        entry.status = TaskStatus::InProgress;
        entry.updated_at = chrono::Utc::now().timestamp();
        let cloned = entry.clone();
        self.persist_entry(&cloned);
        self.replace_tasks(tasks);
        Ok(cloned)
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
        self.refresh_from_repository();
        self.refresh_remote_review_timeouts();
        let tasks = self.tasks.lock();
        let mut entries: Vec<TaskEntry> = tasks.values().cloned().collect();
        entries.sort_by_key(|e| (e.created_at, e.id.clone()));
        entries
    }

    pub fn delete(&self, id: &str) -> Option<TaskEntry> {
        let _guard = self.acquire_task_list_lock("delete task").ok();
        let mut tasks = if _guard.is_some() {
            self.load_repository_tasks()
        } else {
            self.tasks.lock().clone()
        };
        let removed = tasks.remove(id);
        if removed.is_some() {
            let mut changed = Vec::new();
            for entry in tasks.values_mut() {
                let before = entry.depends_on.len();
                entry.depends_on.retain(|dep_id| dep_id != id);
                if entry.depends_on.len() != before {
                    entry.updated_at = chrono::Utc::now().timestamp();
                    changed.push(entry.clone());
                }
            }
            self.runtime_handles.lock().remove(id);
            if let Err(err) = self.repository.delete(id) {
                tracing::warn!(task_id = id, error = %err, "failed to delete persisted task");
            }
            for entry in changed {
                self.persist_entry(&entry);
            }
        }
        self.replace_tasks(tasks);
        removed
    }

    pub fn stop(&self, id: &str) -> Option<TaskEntry> {
        if let Some(handle) = self.runtime_handles.lock().get(id).cloned() {
            handle.cancel();
        }

        let now = chrono::Utc::now().timestamp();
        let _guard = self.acquire_task_list_lock("stop task").ok();
        let mut tasks = if _guard.is_some() {
            self.load_repository_tasks()
        } else {
            self.tasks.lock().clone()
        };
        if let Some(entry) = tasks.get_mut(id) {
            entry.cancel_requested_at = Some(now);
            entry.status = TaskStatus::Cancelled;
            entry.updated_at = now;
            let cloned = entry.clone();
            self.persist_entry(&cloned);
            self.replace_tasks(tasks);
            Some(cloned)
        } else {
            self.replace_tasks(tasks);
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
        blocked_dependencies_with_tasks(&tasks, entry)
    }

    pub fn blocked_tasks(&self, entry: &TaskEntry) -> Vec<String> {
        let tasks = self.tasks.lock();
        let mut ids: Vec<String> = tasks
            .values()
            .filter(|candidate| candidate.depends_on.iter().any(|id| id == &entry.id))
            .map(|candidate| candidate.id.clone())
            .collect();
        ids.sort();
        ids
    }

    fn acquire_task_list_lock(&self, operation: &str) -> Result<TaskListLock> {
        TaskListLock::acquire(self.repository.dir.clone()).with_context(|| {
            format!(
                "failed to acquire task-list lock for {operation} in {}",
                self.repository.dir.display()
            )
        })
    }

    fn load_repository_tasks(&self) -> HashMap<String, TaskEntry> {
        match self.repository.load_for_live_refresh() {
            Ok(tasks) => tasks,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to refresh persisted tasks; keeping in-memory task state"
                );
                self.tasks.lock().clone()
            }
        }
    }

    fn refresh_from_repository(&self) {
        let tasks = self.load_repository_tasks();
        self.replace_tasks(tasks);
    }

    fn replace_tasks(&self, tasks: HashMap<String, TaskEntry>) {
        *self.tasks.lock() = tasks;
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

fn task_to_json_from_store(task_store: &TaskStore, entry: &TaskEntry) -> Value {
    let blocked_dependencies = task_store.blocked_dependencies(entry);
    let blocked_tasks = task_store.blocked_tasks(entry);
    let blocked_by = entry.depends_on.clone();
    json!({
        "id": entry.id,
        "kind": entry.kind,
        "subject": entry.subject,
        "description": entry.description,
        "status": entry.status.as_str(),
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "parent_id": entry.parent_id,
        "depends_on": blocked_by.clone(),
        "blocked_by": blocked_by.clone(),
        "blockedBy": blocked_by,
        "blocks": blocked_tasks,
        "owner": entry.owner,
        "activeForm": entry.active_form,
        "metadata": entry.metadata,
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
        "has_runtime_handle": task_store.has_runtime_handle(&entry.id),
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
    #[serde(default, alias = "blocked_by", alias = "blockedBy")]
    depends_on: Vec<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default, rename = "activeForm")]
    active_form: Option<String>,
    #[serde(default)]
    metadata: Option<Value>,
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
        self.load_with_startup_recovery(true)
    }

    fn load_for_live_refresh(&self) -> Result<HashMap<String, TaskEntry>> {
        self.load_with_startup_recovery(false)
    }

    fn load_with_startup_recovery(
        &self,
        recover_on_startup: bool,
    ) -> Result<HashMap<String, TaskEntry>> {
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

            match self.load_entry(&path, recover_on_startup) {
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

    fn reserve_next_task_id(&self, tasks: &HashMap<String, TaskEntry>) -> Result<String> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create task dir {}", self.dir.display()))?;

        let _guard = HighWatermarkLock::acquire(self.dir.join(TASK_HIGHWATERMARK_LOCK_FILE))?;
        let highest_seen = self
            .read_highwatermark()?
            .max(highest_numeric_task_id(tasks));
        let next_id = highest_seen.saturating_add(1);
        write_text_atomic(
            &self.dir.join(TASK_HIGHWATERMARK_FILE),
            &format!("{next_id}\n"),
        )?;
        Ok(next_id.to_string())
    }

    fn read_highwatermark(&self) -> Result<u64> {
        let path = self.dir.join(TASK_HIGHWATERMARK_FILE);
        match fs::read_to_string(&path) {
            Ok(raw) => Ok(raw.trim().parse::<u64>().unwrap_or(0)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(err) => Err(err)
                .with_context(|| format!("failed to read task high watermark {}", path.display())),
        }
    }

    fn load_entry(&self, path: &Path, recover_on_startup: bool) -> Result<Option<TaskEntry>> {
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
                        owner: None,
                        active_form: None,
                        metadata: None,
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
        let was_recovered = recover_on_startup && recover_task_after_restart(&mut task);
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
            owner: normalize_optional_string(record.owner),
            active_form: normalize_optional_string(record.active_form),
            metadata: record.metadata.filter(|value| !value.is_null()),
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

#[derive(Debug)]
struct HighWatermarkLock {
    path: PathBuf,
}

impl HighWatermarkLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        for attempt in 0..TASK_HIGHWATERMARK_LOCK_RETRIES {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let backoff_ms = (5_u64 << attempt.min(8)).min(250);
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to create high watermark lock {}", path.display())
                    });
                }
            }
        }

        anyhow::bail!("timed out acquiring high watermark lock {}", path.display())
    }
}

impl Drop for HighWatermarkLock {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %err,
                    "failed to remove high watermark lock"
                );
            }
        }
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
            owner: entry.owner.clone(),
            active_form: entry.active_form.clone(),
            metadata: entry.metadata.clone(),
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

fn blocked_dependencies_with_tasks(
    tasks: &HashMap<String, TaskEntry>,
    entry: &TaskEntry,
) -> Vec<String> {
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

fn highest_numeric_task_id(tasks: &HashMap<String, TaskEntry>) -> u64 {
    tasks
        .keys()
        .filter_map(|id| id.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}

fn fallback_task_id(tasks: &HashMap<String, TaskEntry>) -> String {
    loop {
        let id = uuid::Uuid::new_v4().to_string();
        if !tasks.contains_key(&id) {
            return id;
        }
    }
}

fn dependency_ids_from_input(input: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for field in ["depends_on", "blocked_by", "blockedBy"] {
        ids.extend(string_array_field(input, field));
    }
    normalize_dependencies(ids)
}

fn string_array_field(input: &Value, field: &str) -> Vec<String> {
    input
        .get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
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

fn merge_metadata(existing: Option<Value>, patch: &Value) -> Option<Value> {
    let Some(patch_map) = patch.as_object() else {
        return existing;
    };
    let mut merged = existing
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    for (key, value) in patch_map {
        if value.is_null() {
            merged.remove(key);
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    Some(Value::Object(merged))
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

#[derive(Debug)]
struct TaskListLock {
    path: PathBuf,
}

impl TaskListLock {
    fn acquire(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create task dir {}", dir.display()))?;
        let path = dir.join(TASK_LIST_LOCK_FILE);
        for attempt in 0..TASK_LIST_LOCK_RETRIES {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let backoff_ms = (5_u64 << attempt.min(8)).min(250);
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to create task-list lock {}", path.display())
                    });
                }
            }
        }

        anyhow::bail!("timed out acquiring task-list lock {}", path.display())
    }
}

impl Drop for TaskListLock {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %err,
                    "failed to remove task-list lock"
                );
            }
        }
    }
}

// =============================================================================
// Global task store (lazy singleton)
// =============================================================================

#[cfg(test)]
static TEST_TASKS_ROOT: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
    std::env::temp_dir().join(format!(
        "cc-rust-test-global-tasks-{}",
        uuid::Uuid::new_v4()
    ))
});

static GLOBAL_STORES: std::sync::LazyLock<Mutex<HashMap<String, TaskStore>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

static TODO_STORE: std::sync::LazyLock<Mutex<HashMap<String, Vec<TodoItem>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn task_lists_root() -> PathBuf {
    #[cfg(test)]
    {
        if let Ok(root) = std::env::var("CC_RUST_HOME") {
            if !root.trim().is_empty() {
                return PathBuf::from(root).join("tasks");
            }
        }
        return TEST_TASKS_ROOT.clone();
    }

    #[cfg(not(test))]
    {
        crate::config::paths::tasks_dir()
    }
}

pub fn sanitize_task_list_id(input: &str) -> String {
    let sanitized: String = input
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    if sanitized.is_empty() {
        DEFAULT_TASK_LIST_ID.to_string()
    } else {
        sanitized
    }
}

pub fn task_list_dir(task_list_id: &str) -> PathBuf {
    task_lists_root().join(sanitize_task_list_id(task_list_id))
}

fn default_task_list_has_json_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.filter_map(std::result::Result::ok).any(|entry| {
        let path = entry.path();
        path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json")
    })
}

fn read_u64_file(path: &Path) -> u64 {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn migrate_legacy_flat_default_task_list(root: &Path, default_dir: &Path) {
    if !root.exists() || default_task_list_has_json_files(default_dir) {
        return;
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(
                path = %root.display(),
                error = %err,
                "failed to scan legacy flat task directory"
            );
            return;
        }
    };

    let mut legacy_files = Vec::new();
    let mut legacy_highwatermark = None;
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name == TASK_HIGHWATERMARK_FILE {
            legacy_highwatermark = Some(path);
        } else if file_name.ends_with(".json") || file_name.ends_with(".output.log") {
            legacy_files.push(path);
        }
    }

    if legacy_files.is_empty() && legacy_highwatermark.is_none() {
        return;
    }

    if let Err(err) = fs::create_dir_all(default_dir) {
        tracing::warn!(
            path = %default_dir.display(),
            error = %err,
            "failed to create default task-list directory for legacy migration"
        );
        return;
    }

    let mut copied = 0usize;
    for source in legacy_files {
        let Some(file_name) = source.file_name() else {
            continue;
        };
        let destination = default_dir.join(file_name);
        if destination.exists() {
            continue;
        }
        match fs::copy(&source, &destination) {
            Ok(_) => copied += 1,
            Err(err) => tracing::warn!(
                source = %source.display(),
                destination = %destination.display(),
                error = %err,
                "failed to copy legacy flat task file"
            ),
        }
    }

    if let Some(source) = legacy_highwatermark {
        let destination = default_dir.join(TASK_HIGHWATERMARK_FILE);
        let source_value = read_u64_file(&source);
        let destination_value = read_u64_file(&destination);
        if source_value > destination_value {
            if let Err(err) = write_text_atomic(&destination, &format!("{source_value}\n")) {
                tracing::warn!(
                    source = %source.display(),
                    destination = %destination.display(),
                    error = %err,
                    "failed to migrate legacy task high watermark"
                );
            }
        }
    }

    if copied > 0 {
        tracing::info!(
            legacy_dir = %root.display(),
            default_task_list_dir = %default_dir.display(),
            copied,
            "copied legacy flat tasks into default task list"
        );
    }
}

fn env_task_list_id(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn task_list_id_for_context(ctx: &ToolUseContext) -> String {
    if let Some(id) = env_task_list_id(CC_RUST_TASK_LIST_ID_ENV)
        .or_else(|| env_task_list_id(CLAUDE_CODE_TASK_LIST_ID_ENV))
    {
        return id;
    }

    if let Some(team_name) = crate::teams::context::try_get_team_name()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return team_name;
    }

    let app_state = (ctx.get_app_state)();
    if let Some(team_name) = app_state
        .team_context
        .as_ref()
        .map(|team| team.team_name.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return team_name;
    }

    if let Some(team_name) = env_task_list_id(CLAUDE_CODE_TEAM_NAME_ENV) {
        return team_name;
    }

    let session_id = ctx.session_id.trim();
    if session_id.is_empty() {
        DEFAULT_TASK_LIST_ID.to_string()
    } else {
        session_id.to_string()
    }
}

fn task_store_for_task_list_id(task_list_id: &str) -> TaskStore {
    let key = sanitize_task_list_id(task_list_id);
    let root = task_lists_root();
    let dir = root.join(&key);
    let registry_key = dir.to_string_lossy().to_string();
    let mut stores = GLOBAL_STORES.lock();
    stores
        .entry(registry_key)
        .or_insert_with(|| {
            if key == DEFAULT_TASK_LIST_ID {
                migrate_legacy_flat_default_task_list(&root, &dir);
            }
            TaskStore::with_dir(dir)
        })
        .clone()
}

fn store_for_context(ctx: &ToolUseContext) -> TaskStore {
    task_store_for_task_list_id(&task_list_id_for_context(ctx))
}

fn store() -> TaskStore {
    task_store_for_task_list_id(DEFAULT_TASK_LIST_ID)
}

fn todo_owner_key(ctx: &ToolUseContext) -> String {
    ctx.agent_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| ctx.session_id.clone())
}

fn parse_todo_items(input: &Value) -> std::result::Result<Vec<TodoItem>, String> {
    let Some(todos_value) = input.get("todos") else {
        return Err("todos is required".to_string());
    };
    let mut todos: Vec<TodoItem> = serde_json::from_value(todos_value.clone())
        .map_err(|err| format!("invalid todos array: {err}"))?;

    for (index, todo) in todos.iter_mut().enumerate() {
        todo.content = todo.content.trim().to_string();
        if todo.content.is_empty() {
            return Err(format!("todos[{index}].content is required"));
        }
        if !matches!(
            todo.status.as_str(),
            "pending" | "in_progress" | "completed"
        ) {
            return Err(format!("todos[{index}].status is invalid"));
        }
        todo.active_form = todo.active_form.as_ref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    }

    Ok(todos)
}

fn replace_todos_for_key(key: &str, todos: Vec<TodoItem>) -> TodoWriteOutcome {
    let all_done = todos.iter().all(|todo| todo.status == "completed");
    let verification_nudge_needed = all_done
        && todos.len() >= 3
        && !todos
            .iter()
            .any(|todo| todo.content.to_ascii_lowercase().contains("verif"));
    let stored = if all_done { Vec::new() } else { todos };

    TODO_STORE.lock().insert(key.to_string(), stored.clone());

    TodoWriteOutcome {
        todos: stored,
        cleared: all_done,
        verification_nudge_needed,
    }
}

#[cfg(test)]
fn todo_snapshot_for_key(key: &str) -> Vec<TodoItem> {
    TODO_STORE.lock().get(key).cloned().unwrap_or_default()
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

fn task_id_from_input(input: &Value) -> &str {
    input
        .get("task_id")
        .or_else(|| input.get("taskId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn optional_string_update(input: &Value, field: &str) -> Option<String> {
    input
        .get(field)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn task_update_owner(input: &Value, ctx: &ToolUseContext) -> String {
    input
        .get("owner")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            ctx.agent_id
                .as_deref()
                .map(str::trim)
                .filter(|owner| !owner.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| ctx.session_id.clone())
}

fn task_update_check_agent_busy(input: &Value) -> bool {
    input
        .get("check_agent_busy")
        .or_else(|| input.get("checkAgentBusy"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn task_update_fields_from_input(input: &Value) -> TaskUpdateFields {
    TaskUpdateFields {
        subject: optional_string_update(input, "subject"),
        description: optional_string_update(input, "description"),
        active_form: input
            .get("activeForm")
            .map(|value| normalize_optional_string(value.as_str().map(ToString::to_string))),
        owner: input
            .get("owner")
            .map(|value| normalize_optional_string(value.as_str().map(ToString::to_string))),
        metadata_patch: input
            .get("metadata")
            .filter(|value| value.is_object())
            .cloned(),
        status: None,
        add_blocks: string_array_field(input, "addBlocks"),
        add_blocked_by: string_array_field(input, "addBlockedBy"),
    }
}

fn task_updated_fields_from_input(input: &Value, status_value: Option<&str>) -> Vec<&'static str> {
    let mut fields = Vec::new();
    for (input_key, field_name) in [
        ("subject", "subject"),
        ("description", "description"),
        ("activeForm", "activeForm"),
        ("owner", "owner"),
        ("metadata", "metadata"),
        ("addBlocks", "blocks"),
        ("addBlockedBy", "blockedBy"),
    ] {
        if input.get(input_key).is_some() {
            fields.push(field_name);
        }
    }
    if status_value.is_some() {
        fields.push("status");
    }
    fields
}

/// Read-only handle to the global task store, exposed for command surfaces
/// (`/tasks`) that want to enumerate tool-driven tasks without running a
/// tool call. The store is cheap to clone: all interior state is behind `Arc`.
pub fn global_store() -> TaskStore {
    store()
}

// =============================================================================
// TodoWriteTool
// =============================================================================

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    async fn description(&self, _: &Value) -> String {
        "Replace the current session todo list with the provided todos array.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "Complete replacement todo list for the current session or agent",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "Todo item text"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Todo status"
                            },
                            "activeForm": {
                                "type": "string",
                                "description": "Optional in-progress wording for UI display"
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        match parse_todo_items(input) {
            Ok(_) => ValidationResult::Ok,
            Err(message) => ValidationResult::Error {
                message,
                error_code: 1,
            },
        }
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let todos = match parse_todo_items(&input) {
            Ok(todos) => todos,
            Err(message) => {
                return Ok(ToolResult {
                    data: json!({ "error": message }),
                    new_messages: vec![],
                    ..Default::default()
                });
            }
        };
        let key = todo_owner_key(ctx);
        let outcome = replace_todos_for_key(&key, todos);
        let count = outcome.todos.len();
        let mut data = json!({
            "todos": outcome.todos,
            "count": count,
            "cleared": outcome.cleared,
            "message": if outcome.cleared {
                "Todo list cleared because all items are completed"
            } else {
                "Todo list updated"
            },
        });

        if outcome.verification_nudge_needed {
            if let Some(map) = data.as_object_mut() {
                map.insert(
                    "verification_nudge".to_string(),
                    json!(
                        "You completed 3+ todo items without a verification step; consider adding or running verification before claiming completion."
                    ),
                );
            }
        }

        Ok(ToolResult {
            data,
            new_messages: vec![],
            ..Default::default()
        })
    }

    async fn prompt(&self) -> String {
        "Track progress by replacing the current todo list with a complete todos array. Use pending, in_progress, and completed statuses.".to_string()
    }
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
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown while the task is in progress"
                },
                "metadata": {
                    "type": "object",
                    "description": "Arbitrary metadata to attach to the task"
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
                "blocked_by": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Bun-compatible alias for depends_on"
                },
                "blockedBy": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Bun-compatible camelCase alias for depends_on"
                },
                "owner": {
                    "type": "string",
                    "description": "Optional owner that has claimed this task"
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
        let active_form = input
            .get("activeForm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let metadata = input.get("metadata").filter(|v| v.is_object()).cloned();
        let kind = input
            .get("kind")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let parent_id = input
            .get("parent_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let depends_on = dependency_ids_from_input(&input);
        let owner = input
            .get("owner")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
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
            || owner.is_some()
            || active_form.is_some()
            || metadata.is_some()
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

        let task_store = store_for_context(ctx);
        let entry = if has_options {
            task_store.create_with_options(
                subject,
                description,
                TaskCreateOptions {
                    kind,
                    parent_id,
                    depends_on,
                    owner,
                    active_form,
                    metadata,
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
            task_store.create(subject, description)
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
            "task": task_to_json_from_store(&task_store, &entry),
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
            "anyOf": [
                { "required": ["task_id"] },
                { "required": ["taskId"] }
            ]
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

        let task_store = store_for_context(ctx);
        match task_store.get(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({ "task": task_to_json_from_store(&task_store, &entry) }),
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
                "taskId": {
                    "type": "string",
                    "description": "Bun-compatible alias for task_id"
                },
                "subject": {
                    "type": "string",
                    "description": "New subject for the task"
                },
                "description": {
                    "type": "string",
                    "description": "New description for the task"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown while the task is in progress"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "failed", "cancelled", "recoverable", "interrupted", "deleted"],
                    "description": "New status for the task"
                },
                "addBlocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that this task blocks"
                },
                "addBlockedBy": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that block this task"
                },
                "owner": {
                    "type": "string",
                    "description": "Agent or session owner for this task"
                },
                "metadata": {
                    "type": "object",
                    "description": "Metadata keys to merge into the task; null values delete keys"
                },
                "check_agent_busy": {
                    "type": "boolean",
                    "description": "When claiming, fail if the same owner already has another unfinished task"
                },
                "checkAgentBusy": {
                    "type": "boolean",
                    "description": "Bun-compatible camelCase alias for check_agent_busy"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = task_id_from_input(&input);
        let status_value = input.get("status").and_then(|v| v.as_str());
        let status = match status_value {
            Some("deleted") | None => None,
            Some(status_str) => match TaskStatus::from_str(status_str) {
                Some(status) => Some(status),
                None => {
                    return Ok(ToolResult {
                        data: json!({ "error": format!("Invalid task status: {}", status_str) }),
                        new_messages: vec![],
                        ..Default::default()
                    });
                }
            },
        };

        let task_store = store_for_context(ctx);
        let existing = task_store.get(id);
        let Some(existing) = existing else {
            return Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
                ..Default::default()
            });
        };

        if status_value == Some("deleted") {
            let deleted = task_store.delete(id).is_some();
            return Ok(ToolResult {
                data: json!({
                    "success": deleted,
                    "task_id": id,
                    "updated_fields": if deleted { vec!["deleted"] } else { Vec::<&str>::new() },
                    "status_change": if deleted {
                        json!({ "from": existing.status.as_str(), "to": "deleted" })
                    } else {
                        Value::Null
                    },
                    "message": if deleted {
                        format!("Task '{}' deleted", existing.subject)
                    } else {
                        format!("Task not found: {}", id)
                    },
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        if status == Some(TaskStatus::InProgress) {
            let owner = task_update_owner(&input, ctx);
            let check_agent_busy = task_update_check_agent_busy(&input);
            let entry = match task_store.claim_task(id, &owner, check_agent_busy) {
                Ok(entry) => entry,
                Err(failure) => {
                    return Ok(ToolResult {
                        data: json!({
                            "error": format!("Task claim failed: {}", failure.reason.as_str()),
                            "claim": failure.to_json(),
                        }),
                        new_messages: vec![],
                        ..Default::default()
                    });
                }
            };
            let mut updates = task_update_fields_from_input(&input);
            updates.status = None;
            updates.owner = None;
            let entry = if updates.subject.is_some()
                || updates.description.is_some()
                || updates.active_form.is_some()
                || updates.metadata_patch.is_some()
                || !updates.add_blocks.is_empty()
                || !updates.add_blocked_by.is_empty()
            {
                task_store.update_fields(id, updates).unwrap_or(entry)
            } else {
                entry
            };
            return Ok(ToolResult {
                data: json!({
                    "task": task_to_json_from_store(&task_store, &entry),
                    "updated_fields": ["status", "owner"],
                    "status_change": { "from": existing.status.as_str(), "to": TaskStatus::InProgress.as_str() },
                    "message": format!("Task '{}' claimed by {}", entry.subject, owner)
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        let mut updates = task_update_fields_from_input(&input);
        updates.status = status;
        let updated_fields = task_updated_fields_from_input(&input, status_value);

        match task_store.update_fields(id, updates) {
            Some(entry) => {
                // Fire TaskCompleted hook when status changes to completed.
                if status == Some(TaskStatus::Completed) && existing.status != TaskStatus::Completed
                {
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
                        "task": task_to_json_from_store(&task_store, &entry),
                        "updated_fields": updated_fields,
                        "status_change": status.map(|new_status| json!({
                            "from": existing.status.as_str(),
                            "to": new_status.as_str()
                        })),
                        "message": if let Some(status) = status {
                            format!("Task '{}' updated to {}", entry.subject, status.as_str())
                        } else {
                            format!("Task '{}' updated", entry.subject)
                        }
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
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let task_store = store_for_context(ctx);
        let entries = task_store.list();
        let tasks: Vec<Value> = entries
            .iter()
            .map(|entry| task_to_json_from_store(&task_store, entry))
            .collect();

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
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");

        let task_store = store_for_context(ctx);
        match task_store.stop(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task": task_to_json_from_store(&task_store, &entry),
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
        "owner": entry.owner,
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
        "owner": entry.owner,
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

        let task_store = store_for_context(ctx);
        let initial = task_store.get(id);
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
                        task_store.clone(),
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
    use crate::types::app_state::AppState;
    use serde_json::json;
    use std::ffi::OsString;
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

    fn test_context_with_app_state(app_state: AppState) -> ToolUseContext {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".into(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: rx,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || app_state.clone()),
            set_app_state: Arc::new(|_| {}),
            session_id: "test-session".to_string(),
            langfuse_session_id: "test-session".to_string(),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            ask_user_callback: None,
            bg_agent_tx: None,
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        }
    }

    fn test_context() -> ToolUseContext {
        test_context_with_app_state(AppState::default())
    }

    fn dummy_parent() -> AssistantMessage {
        AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn task_list_id_prefers_explicit_env_over_team_context() {
        let _cc_rust = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, " explicit/list ");
        let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
        let _team_env = EnvGuard::set(CLAUDE_CODE_TEAM_NAME_ENV, "env-team");
        let mut app_state = AppState::default();
        app_state.team_context = Some(cc_types::teams::TeamContext {
            team_name: "state-team".to_string(),
            ..Default::default()
        });
        let ctx = test_context_with_app_state(app_state);

        assert_eq!(task_list_id_for_context(&ctx), "explicit/list");
        assert_eq!(sanitize_task_list_id("explicit/list"), "explicit-list");
    }

    #[test]
    #[serial_test::serial]
    fn task_list_id_uses_team_context_before_session_and_team_env() {
        let _cc_rust = EnvGuard::remove(CC_RUST_TASK_LIST_ID_ENV);
        let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
        let _team_env = EnvGuard::set(CLAUDE_CODE_TEAM_NAME_ENV, "env-team");
        let mut app_state = AppState::default();
        app_state.team_context = Some(cc_types::teams::TeamContext {
            team_name: "state-team".to_string(),
            ..Default::default()
        });
        let ctx = test_context_with_app_state(app_state);

        assert_eq!(task_list_id_for_context(&ctx), "state-team");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn task_list_id_uses_in_process_teammate_team_name() {
        let _cc_rust = EnvGuard::remove(CC_RUST_TASK_LIST_ID_ENV);
        let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
        let ctx = test_context();
        let identity = crate::teams::types::TeammateIdentity {
            agent_id: "worker@scope-team".to_string(),
            agent_name: "worker".to_string(),
            team_name: "scope-team".to_string(),
            color: None,
            plan_mode_required: false,
            parent_session_id: "leader-session".to_string(),
        };

        crate::teams::context::run_in_scope(identity, async {
            assert_eq!(task_list_id_for_context(&ctx), "scope-team");
        })
        .await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn task_tools_use_task_list_scoped_store() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path());
        let list_a = format!("phase1-a-{}", uuid::Uuid::new_v4());
        let list_b = format!("phase1-b-{}", uuid::Uuid::new_v4());
        let parent = dummy_parent();

        {
            let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_a);
            let ctx_a = test_context();
            TaskCreateTool
                .call(
                    json!({ "subject": "only list a", "description": "scoped" }),
                    &ctx_a,
                    &parent,
                    None,
                )
                .await
                .expect("create task in list a");

            let listed = TaskListTool
                .call(json!({}), &ctx_a, &parent, None)
                .await
                .expect("list a");
            let tasks = listed.data["tasks"].as_array().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0]["subject"], "only list a");
        }

        {
            let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_b);
            let ctx_b = test_context();
            let listed = TaskListTool
                .call(json!({}), &ctx_b, &parent, None)
                .await
                .expect("list b before create");
            assert_eq!(listed.data["count"], 0);

            TaskCreateTool
                .call(
                    json!({ "subject": "only list b", "description": "scoped" }),
                    &ctx_b,
                    &parent,
                    None,
                )
                .await
                .expect("create task in list b");
        }

        {
            let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_a);
            let ctx_a = test_context();
            let listed = TaskListTool
                .call(json!({}), &ctx_a, &parent, None)
                .await
                .expect("list a again");
            let tasks = listed.data["tasks"].as_array().unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0]["subject"], "only list a");
        }

        assert!(task_list_dir(&list_a).starts_with(home.path().join("tasks")));
        assert!(task_list_dir(&list_b).starts_with(home.path().join("tasks")));
    }

    #[test]
    #[serial_test::serial]
    fn default_task_list_store_copies_legacy_flat_tasks() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path());
        let legacy_root = home.path().join("tasks");
        fs::create_dir_all(&legacy_root).unwrap();
        fs::write(
            legacy_root.join("legacy-task.json"),
            serde_json::to_string_pretty(&json!({
                "id": "legacy-task",
                "subject": "legacy flat task",
                "description": "from pre-task-list storage",
                "status": "pending",
                "output": "legacy output",
                "created_at": 1,
                "updated_at": 2,
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(legacy_root.join(TASK_HIGHWATERMARK_FILE), "9\n").unwrap();

        let task_store = task_store_for_task_list_id(DEFAULT_TASK_LIST_ID);
        let migrated = task_store.get("legacy-task").unwrap();

        assert_eq!(migrated.subject, "legacy flat task");
        assert_eq!(migrated.output, "legacy output");
        assert!(legacy_root.join("legacy-task.json").exists());
        assert!(task_list_dir(DEFAULT_TASK_LIST_ID)
            .join("legacy-task.json")
            .exists());

        let next = task_store.create("next", "");
        assert_eq!(next.id, "10");
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
    fn test_task_store_allocates_incrementing_ids_and_preserves_highwater() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let first = store.create("first", "");
        let second = store.create("second", "");

        assert_eq!(first.id, "1");
        assert_eq!(second.id, "2");

        store.delete(&second.id).unwrap();
        let third = store.create("third", "");
        assert_eq!(third.id, "3");

        let restarted = TaskStore::with_dir(tmp.path());
        let fourth = restarted.create("fourth", "");
        assert_eq!(fourth.id, "4");
    }

    #[test]
    fn test_task_store_bootstraps_highwater_from_existing_numeric_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = TaskEntry {
            id: "7".to_string(),
            kind: TASK_KIND_TOOL.to_string(),
            subject: "legacy".to_string(),
            description: String::new(),
            status: TaskStatus::Completed,
            output: String::new(),
            output_summary: String::new(),
            output_bytes: 0,
            output_truncated: false,
            parent_id: None,
            depends_on: Vec::new(),
            owner: None,
            active_form: None,
            metadata: None,
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
            created_at: 1,
            updated_at: 1,
        };
        TaskRepository::new(tmp.path().to_path_buf(), DEFAULT_OUTPUT_LIMIT_BYTES)
            .persist_entry(&legacy)
            .unwrap();

        let store = TaskStore::with_dir(tmp.path());
        let next = store.create("next", "");
        assert_eq!(next.id, "8");
    }

    #[test]
    fn test_concurrent_task_creates_use_unique_incrementing_ids() {
        let (_tmp, store) = temp_store();
        let store = Arc::new(store);
        let mut threads = Vec::new();

        for index in 0..8 {
            let store = Arc::clone(&store);
            threads.push(thread::spawn(move || {
                store.create(&format!("task {index}"), "").id
            }));
        }

        let mut ids: Vec<u64> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap().parse::<u64>().unwrap())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8]);
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

    #[tokio::test]
    #[serial_test::serial]
    async fn task_list_and_task_stop_tools_cover_cancel_flow() {
        let subject = format!("phase0-stop-{}", uuid::Uuid::new_v4());
        let ctx = test_context();
        let task_store = store_for_context(&ctx);
        let task = task_store.create(&subject, "cancel through tool");
        let parent = dummy_parent();

        let listed = TaskListTool
            .call(json!({}), &ctx, &parent, None)
            .await
            .expect("list tasks");
        let tasks = listed.data["tasks"].as_array().expect("tasks array");
        assert!(tasks.iter().any(|entry| {
            entry["id"].as_str() == Some(task.id.as_str())
                && entry["subject"].as_str() == Some(subject.as_str())
                && entry["status"].as_str() == Some(TaskStatus::Pending.as_str())
        }));

        let stopped = TaskStopTool
            .call(json!({ "task_id": task.id.clone() }), &ctx, &parent, None)
            .await
            .expect("stop task");
        assert_eq!(stopped.data["task"]["id"].as_str(), Some(task.id.as_str()));
        assert_eq!(
            stopped.data["task"]["status"].as_str(),
            Some(TaskStatus::Cancelled.as_str())
        );
        assert!(stopped.data["message"].as_str().unwrap().contains(&subject));

        let missing = TaskStopTool
            .call(
                json!({ "task_id": format!("missing-{}", task.id) }),
                &ctx,
                &parent,
                None,
            )
            .await
            .expect("missing task response");
        assert!(missing.data["error"]
            .as_str()
            .unwrap()
            .contains("Task not found"));
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
        assert_eq!(store.blocked_tasks(&dep), vec![child.id.clone()]);

        store.update_status(&dep.id, TaskStatus::Completed);
        let restarted = TaskStore::with_dir(tmp.path());
        let child = restarted.get(&child.id).unwrap();
        assert_eq!(child.depends_on, vec![dep.id.clone()]);
        assert!(restarted.blocked_dependencies(&child).is_empty());
    }

    #[test]
    fn test_task_json_exposes_bun_dependency_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let dep = store.create("dep", "");
        let child = store.create_with_options(
            "child",
            "",
            TaskCreateOptions {
                depends_on: vec![dep.id.clone()],
                ..TaskCreateOptions::default()
            },
        );

        let dep_json = task_to_json_for_store(&store, &dep);
        assert_eq!(dep_json["blocks"], json!([child.id.clone()]));

        let child_json = task_to_json_for_store(&store, &child);
        assert_eq!(child_json["depends_on"], json!([dep.id.clone()]));
        assert_eq!(child_json["blocked_by"], json!([dep.id.clone()]));
        assert_eq!(child_json["blockedBy"], json!([dep.id.clone()]));
    }

    #[test]
    fn test_task_claim_respects_dependencies_and_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let dep = store.create("dep", "");
        let child = store.create_with_options(
            "child",
            "",
            TaskCreateOptions {
                depends_on: vec![dep.id.clone()],
                ..TaskCreateOptions::default()
            },
        );

        let blocked = store
            .claim_task(&child.id, "agent-a", false)
            .expect_err("unfinished dependency should block claim");
        assert_eq!(blocked.reason, TaskClaimFailureReason::Blocked);
        assert_eq!(blocked.blocked_by, vec![dep.id.clone()]);

        store.update_status(&dep.id, TaskStatus::Completed);
        let claimed = store.claim_task(&child.id, "agent-a", false).unwrap();
        assert_eq!(claimed.status, TaskStatus::InProgress);
        assert_eq!(claimed.owner.as_deref(), Some("agent-a"));

        let other_owner = store
            .claim_task(&child.id, "agent-b", false)
            .expect_err("different owner should not steal claim");
        assert_eq!(other_owner.reason, TaskClaimFailureReason::AlreadyClaimed);
        assert_eq!(other_owner.owner.as_deref(), Some("agent-a"));
    }

    #[test]
    fn test_task_claim_agent_busy_mode_allows_one_unfinished_task_per_owner() {
        let (_tmp, store) = temp_store();
        let first = store.create("first", "");
        let second = store.create("second", "");

        store.claim_task(&first.id, "agent-a", true).unwrap();
        let busy = store
            .claim_task(&second.id, "agent-a", true)
            .expect_err("agent already owns an unfinished task");

        assert_eq!(busy.reason, TaskClaimFailureReason::AgentBusy);
        assert_eq!(busy.busy_task_id.as_deref(), Some(first.id.as_str()));

        store.update_status(&first.id, TaskStatus::Completed);
        let claimed = store.claim_task(&second.id, "agent-a", true).unwrap();
        assert_eq!(claimed.owner.as_deref(), Some("agent-a"));
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
                owner: Some("agent-owner".to_string()),
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
        assert_eq!(by_agent.owner.as_deref(), Some("agent-owner"));

        let restarted = TaskStore::with_dir(tmp.path());
        let restored = restarted.get_by_agent_id("agent-1").unwrap();
        assert_eq!(restored.owner.as_deref(), Some("agent-owner"));
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
    fn test_todo_write_schema_matches_upstream_shape() {
        let schema = TodoWriteTool.input_json_schema();
        assert_eq!(schema["required"], json!(["todos"]));
        let item = &schema["properties"]["todos"]["items"];
        assert_eq!(item["required"], json!(["content", "status"]));
        assert_eq!(
            item["properties"]["status"]["enum"],
            json!(["pending", "in_progress", "completed"])
        );
        assert!(item["properties"].get("activeForm").is_some());
    }

    #[test]
    fn test_todo_write_replaces_session_state_and_clears_completed_list() {
        let key = format!("todo-test-{}", uuid::Uuid::new_v4());
        let todos = parse_todo_items(&json!({
            "todos": [
                { "content": "Plan work", "status": "completed" },
                { "content": "Implement work", "status": "in_progress", "activeForm": "Implementing work" }
            ]
        }))
        .unwrap();

        let outcome = replace_todos_for_key(&key, todos);
        assert!(!outcome.cleared);
        assert_eq!(outcome.todos.len(), 2);
        assert_eq!(todo_snapshot_for_key(&key).len(), 2);

        let completed = parse_todo_items(&json!({
            "todos": [
                { "content": "Plan work", "status": "completed" },
                { "content": "Implement work", "status": "completed" },
                { "content": "Document work", "status": "completed" }
            ]
        }))
        .unwrap();

        let outcome = replace_todos_for_key(&key, completed);
        assert!(outcome.cleared);
        assert!(outcome.todos.is_empty());
        assert!(outcome.verification_nudge_needed);
        assert!(todo_snapshot_for_key(&key).is_empty());
    }

    #[test]
    fn test_todo_write_keeps_completed_verification_list_quiet() {
        let key = format!("todo-test-{}", uuid::Uuid::new_v4());
        let todos = parse_todo_items(&json!({
            "todos": [
                { "content": "Implement work", "status": "completed" },
                { "content": "Verify behavior", "status": "completed" },
                { "content": "Update docs", "status": "completed" }
            ]
        }))
        .unwrap();

        let outcome = replace_todos_for_key(&key, todos);
        assert!(outcome.cleared);
        assert!(!outcome.verification_nudge_needed);
    }

    #[test]
    fn test_todo_write_rejects_invalid_items() {
        assert!(parse_todo_items(&json!({})).is_err());
        assert!(parse_todo_items(&json!({
            "todos": [{ "content": "", "status": "pending" }]
        }))
        .is_err());
        assert!(parse_todo_items(&json!({
            "todos": [{ "content": "Run", "status": "running" }]
        }))
        .is_err());
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
            "depends_on",
            "blocked_by",
            "blockedBy",
            "owner",
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
    fn test_task_update_schema_exposes_claim_controls() {
        let schema = TaskUpdateTool.input_json_schema();
        let props = &schema["properties"];
        assert!(props.get("owner").is_some());
        assert!(props.get("check_agent_busy").is_some());
        assert!(props.get("checkAgentBusy").is_some());
    }

    #[test]
    fn test_task_create_dependency_aliases_normalize() {
        let deps = dependency_ids_from_input(&json!({
            "depends_on": ["a", "b"],
            "blocked_by": ["b", "c"],
            "blockedBy": ["c", "d"]
        }));
        assert_eq!(deps, vec!["a", "b", "c", "d"]);
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

    #[test]
    fn phase0_gap_cross_store_claim_requires_task_list_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let store_a = TaskStore::with_dir(tmp.path());
        let store_b = TaskStore::with_dir(tmp.path());
        let task = store_a.create("claim race", "");

        let claimed_a = store_a
            .claim_task(&task.id, "agent-a", true)
            .expect("first claimant should win");
        assert_eq!(claimed_a.owner.as_deref(), Some("agent-a"));

        let claimed_b = store_b.claim_task(&task.id, "agent-b", true);
        assert!(
            claimed_b.is_err(),
            "second store must observe the persisted owner and fail the claim"
        );
    }

    #[test]
    fn task_list_lock_makes_agent_busy_check_cross_store_atomic() {
        let tmp = tempfile::tempdir().unwrap();
        let store_a = TaskStore::with_dir(tmp.path());
        let store_b = TaskStore::with_dir(tmp.path());
        let first = store_a.create("first", "");
        let second = store_a.create("second", "");

        store_a
            .claim_task(&first.id, "agent-a", true)
            .expect("first claim should win");
        let busy = store_b.claim_task(&second.id, "agent-a", true).unwrap_err();

        assert_eq!(busy.reason, TaskClaimFailureReason::AgentBusy);
        assert_eq!(busy.busy_task_id.as_deref(), Some(first.id.as_str()));
    }

    #[test]
    fn task_claim_reports_lock_unavailable_when_list_lock_is_held() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let task = store.create("locked", "");
        fs::write(tmp.path().join(TASK_LIST_LOCK_FILE), "").unwrap();

        let failure = store.claim_task(&task.id, "agent-a", true).unwrap_err();

        assert_eq!(failure.reason, TaskClaimFailureReason::LockUnavailable);
    }

    #[test]
    fn task_delete_removes_dependency_references_under_list_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStore::with_dir(tmp.path());
        let blocker = store.create("blocker", "");
        let blocked = store.create_with_options(
            "blocked",
            "",
            TaskCreateOptions {
                depends_on: vec![blocker.id.clone()],
                ..TaskCreateOptions::default()
            },
        );

        store.delete(&blocker.id).expect("delete blocker");
        let reloaded = TaskStore::with_dir(tmp.path());
        let cleaned = reloaded.get(&blocked.id).unwrap();

        assert!(cleaned.depends_on.is_empty());
    }

    #[test]
    fn task_list_refreshes_tasks_created_by_other_store() {
        let tmp = tempfile::tempdir().unwrap();
        let store_a = TaskStore::with_dir(tmp.path());
        let store_b = TaskStore::with_dir(tmp.path());
        let task = store_a.create("external", "");

        let listed = store_b.list();

        assert!(listed.iter().any(|entry| entry.id == task.id));
    }

    #[test]
    fn phase0_gap_task_v2_schema_requires_active_form_and_metadata() {
        let create_schema = TaskCreateTool.input_json_schema();
        let create_props = &create_schema["properties"];
        assert!(create_props.get("activeForm").is_some());
        assert!(create_props.get("metadata").is_some());

        let update_schema = TaskUpdateTool.input_json_schema();
        let update_props = &update_schema["properties"];
        for field in [
            "subject",
            "description",
            "activeForm",
            "addBlocks",
            "addBlockedBy",
            "metadata",
        ] {
            assert!(
                update_props.get(field).is_some(),
                "TaskUpdate schema should expose {field}"
            );
        }
        assert!(update_props["status"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("deleted")));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn task_create_and_update_persist_active_form_and_metadata() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path());
        let list_id = format!("phase3-schema-{}", uuid::Uuid::new_v4());
        let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_id);
        let ctx = test_context();
        let parent = dummy_parent();

        let created = TaskCreateTool
            .call(
                json!({
                    "subject": "original",
                    "description": "before",
                    "activeForm": "Running original",
                    "metadata": {
                        "keep": 1,
                        "drop": true
                    }
                }),
                &ctx,
                &parent,
                None,
            )
            .await
            .expect("create task");
        let id = created.data["task"]["id"].as_str().unwrap().to_string();

        let update = TaskUpdateTool
            .call(
                json!({
                    "task_id": id,
                    "subject": "renamed",
                    "description": "after",
                    "activeForm": "Running renamed",
                    "metadata": {
                        "drop": null,
                        "add": 2
                    }
                }),
                &ctx,
                &parent,
                None,
            )
            .await
            .expect("update task");
        assert_eq!(update.data["task"]["subject"], "renamed");
        assert_eq!(update.data["task"]["activeForm"], "Running renamed");

        let restarted = TaskStore::with_dir(task_list_dir(&list_id));
        let persisted = restarted.get(&id).unwrap();
        assert_eq!(persisted.subject, "renamed");
        assert_eq!(persisted.description, "after");
        assert_eq!(persisted.active_form.as_deref(), Some("Running renamed"));
        let metadata = persisted.metadata.unwrap();
        assert_eq!(metadata["keep"], 1);
        assert_eq!(metadata["add"], 2);
        assert!(metadata.get("drop").is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn task_update_adds_dependency_edges_and_deleted_cleans_them() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path());
        let list_id = format!("phase3-deps-{}", uuid::Uuid::new_v4());
        let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_id);
        let ctx = test_context();
        let parent = dummy_parent();
        let task_store = store_for_context(&ctx);
        let source = task_store.create("source", "");
        let blocked = task_store.create("blocked", "");
        let blocker = task_store.create("blocker", "");

        TaskUpdateTool
            .call(
                json!({
                    "task_id": source.id,
                    "addBlocks": [blocked.id],
                    "addBlockedBy": [blocker.id]
                }),
                &ctx,
                &parent,
                None,
            )
            .await
            .expect("update dependencies");

        let reloaded = TaskStore::with_dir(task_list_dir(&list_id));
        let source_after = reloaded.get(&source.id).unwrap();
        let blocked_after = reloaded.get(&blocked.id).unwrap();
        assert!(source_after.depends_on.iter().any(|id| id == &blocker.id));
        assert!(blocked_after.depends_on.iter().any(|id| id == &source.id));

        let deleted = TaskUpdateTool
            .call(
                json!({
                    "taskId": blocker.id,
                    "status": "deleted"
                }),
                &ctx,
                &parent,
                None,
            )
            .await
            .expect("delete blocker");
        assert_eq!(deleted.data["success"], true);

        let after_delete = TaskStore::with_dir(task_list_dir(&list_id));
        assert!(after_delete.get(&blocker.id).is_none());
        let source_after_delete = after_delete.get(&source.id).unwrap();
        assert!(!source_after_delete
            .depends_on
            .iter()
            .any(|id| id == &blocker.id));
    }

    #[test]
    #[ignore = "Phase 4: replace with concrete teammate-unassign assertions"]
    fn phase0_gap_teammate_unassign_is_not_wired() {
        panic!("Phase 4 should add teammate exit unassign tests once the reset hook exists");
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
        let blocked_tasks = store.blocked_tasks(entry);
        let blocked_by = entry.depends_on.clone();
        json!({
            "id": entry.id,
            "kind": entry.kind,
            "subject": entry.subject,
            "description": entry.description,
            "status": entry.status.as_str(),
            "created_at": entry.created_at,
            "updated_at": entry.updated_at,
            "parent_id": entry.parent_id,
            "depends_on": blocked_by.clone(),
            "blocked_by": blocked_by.clone(),
            "blockedBy": blocked_by,
            "blocks": blocked_tasks,
            "owner": entry.owner,
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
