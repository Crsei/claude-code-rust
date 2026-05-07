use super::*;

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
pub(super) struct TaskUpdateFields {
    pub(super) subject: Option<String>,
    pub(super) description: Option<String>,
    pub(super) active_form: Option<Option<String>>,
    pub(super) owner: Option<Option<String>>,
    pub(super) metadata_patch: Option<Value>,
    pub(super) status: Option<TaskStatus>,
    pub(super) add_blocks: Vec<String>,
    pub(super) add_blocked_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnassignedTaskSummary {
    pub id: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnassignTeammateTasksResult {
    pub unassigned_tasks: Vec<UnassignedTaskSummary>,
    pub notification_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeammateTaskExitReason {
    Terminated,
    Shutdown,
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

    pub(super) fn should_interrupt_on_startup(self) -> bool {
        matches!(
            self,
            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Recoverable
        )
    }

    pub(super) fn is_success(self) -> bool {
        matches!(self, TaskStatus::Completed)
    }

    pub(super) fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Completed
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Interrupted
                | TaskStatus::Stopped
        )
    }

    pub(super) fn is_active_for_output_wait(self) -> bool {
        matches!(
            self,
            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Recoverable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskClaimFailureReason {
    TaskNotFound,
    AlreadyClaimed,
    AlreadyResolved,
    Blocked,
    AgentBusy,
    OwnerRequired,
    LockUnavailable,
}

impl TaskClaimFailureReason {
    pub(super) fn as_str(self) -> &'static str {
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
pub(super) struct TaskClaimFailure {
    pub(super) reason: TaskClaimFailureReason,
    pub(super) owner: Option<String>,
    pub(super) blocked_by: Vec<String>,
    pub(super) busy_task_id: Option<String>,
}

impl TaskClaimFailure {
    pub(super) fn new(reason: TaskClaimFailureReason) -> Self {
        Self {
            reason,
            owner: None,
            blocked_by: Vec::new(),
            busy_task_id: None,
        }
    }

    pub(super) fn with_owner(mut self, owner: String) -> Self {
        self.owner = Some(owner);
        self
    }

    pub(super) fn with_blocked_by(mut self, blocked_by: Vec<String>) -> Self {
        self.blocked_by = blocked_by;
        self
    }

    pub(super) fn with_busy_task_id(mut self, busy_task_id: String) -> Self {
        self.busy_task_id = Some(busy_task_id);
        self
    }

    pub(super) fn to_json(&self) -> Value {
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
pub(super) struct TodoItem {
    pub(super) content: String,
    pub(super) status: String,
    #[serde(
        rename = "activeForm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) active_form: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct TodoWriteOutcome {
    pub(super) todos: Vec<TodoItem>,
    pub(super) cleared: bool,
    pub(super) verification_nudge_needed: bool,
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

    pub fn try_create(&self, subject: &str, description: &str) -> Result<TaskEntry> {
        self.try_create_with_options(subject, description, TaskCreateOptions::default())
    }

    #[cfg(test)]
    pub fn create(&self, subject: &str, description: &str) -> TaskEntry {
        self.try_create(subject, description)
            .expect("failed to create task")
    }

    #[cfg(test)]
    pub fn create_with_options(
        &self,
        subject: &str,
        description: &str,
        options: TaskCreateOptions,
    ) -> TaskEntry {
        self.try_create_with_options(subject, description, options)
            .expect("failed to create task")
    }

    pub fn try_create_with_options(
        &self,
        subject: &str,
        description: &str,
        options: TaskCreateOptions,
    ) -> Result<TaskEntry> {
        let _guard = self.acquire_task_list_lock("create task")?;
        let mut tasks = self.load_repository_tasks_strict()?;
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
        self.persist_entry_strict(&entry)?;
        self.replace_tasks(tasks);
        Ok(entry)
    }

    pub fn get(&self, id: &str) -> Option<TaskEntry> {
        self.refresh_from_repository();
        self.refresh_remote_review_timeout(id)
    }

    #[cfg(test)]
    pub fn update_status(&self, id: &str, status: TaskStatus) -> Option<TaskEntry> {
        self.try_update_status(id, status).ok().flatten()
    }

    pub fn try_update_status(&self, id: &str, status: TaskStatus) -> Result<Option<TaskEntry>> {
        let _guard = self.acquire_task_list_lock("update task status")?;
        let mut tasks = self.load_repository_tasks_strict()?;
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = normalize_new_status(status);
            entry.updated_at = chrono::Utc::now().timestamp();
            if entry.status == TaskStatus::Cancelled && entry.cancel_requested_at.is_none() {
                entry.cancel_requested_at = Some(entry.updated_at);
            }
            let cloned = entry.clone();
            self.persist_entry_strict(&cloned)?;
            self.replace_tasks(tasks);
            Ok(Some(cloned))
        } else {
            self.replace_tasks(tasks);
            Ok(None)
        }
    }

    pub(super) fn try_update_fields(
        &self,
        id: &str,
        updates: TaskUpdateFields,
    ) -> Result<Option<TaskEntry>> {
        let _guard = self.acquire_task_list_lock("update task fields")?;
        let mut tasks = self.load_repository_tasks_strict()?;
        let now = chrono::Utc::now().timestamp();
        let mut changed_entries = Vec::new();
        let updated = {
            let Some(entry) = tasks.get_mut(id) else {
                self.replace_tasks(tasks);
                return Ok(None);
            };
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
            self.persist_entry_strict(&entry)?;
        }
        self.replace_tasks(tasks);
        Ok(Some(updated))
    }

    pub(super) fn claim_task(
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

    pub fn try_delete(&self, id: &str) -> Result<Option<TaskEntry>> {
        let _guard = self.acquire_task_list_lock("delete task")?;
        let mut tasks = self.load_repository_tasks_strict()?;
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
            self.repository.delete(id)?;
            for entry in changed {
                self.persist_entry_strict(&entry)?;
            }
        }
        self.replace_tasks(tasks);
        Ok(removed)
    }

    #[cfg(test)]
    pub fn delete(&self, id: &str) -> Option<TaskEntry> {
        self.try_delete(id).ok().flatten()
    }

    pub(super) fn unassign_teammate_tasks(
        &self,
        teammate_id: &str,
        teammate_name: &str,
    ) -> Vec<UnassignedTaskSummary> {
        self.try_unassign_teammate_tasks(teammate_id, teammate_name)
            .unwrap_or_default()
    }

    pub(super) fn try_unassign_teammate_tasks(
        &self,
        teammate_id: &str,
        teammate_name: &str,
    ) -> Result<Vec<UnassignedTaskSummary>> {
        let _guard = self.acquire_task_list_lock("unassign teammate tasks")?;
        let mut tasks = self.load_repository_tasks_strict()?;
        let now = chrono::Utc::now().timestamp();
        let mut changed = Vec::new();
        for entry in tasks.values_mut() {
            let owner_matches = entry.owner.as_deref() == Some(teammate_id)
                || entry.owner.as_deref() == Some(teammate_name);
            if owner_matches && !entry.status.is_terminal() {
                entry.owner = None;
                entry.status = TaskStatus::Pending;
                entry.updated_at = now;
                changed.push(entry.clone());
            }
        }

        for entry in &changed {
            self.persist_entry_strict(entry)?;
        }
        self.replace_tasks(tasks);
        Ok(changed
            .into_iter()
            .map(|entry| UnassignedTaskSummary {
                id: entry.id,
                subject: entry.subject,
            })
            .collect())
    }

    #[cfg(test)]
    pub fn stop(&self, id: &str) -> Option<TaskEntry> {
        self.try_stop(id).ok().flatten()
    }

    pub fn try_stop(&self, id: &str) -> Result<Option<TaskEntry>> {
        if let Some(handle) = self.runtime_handles.lock().get(id).cloned() {
            handle.cancel();
        }

        let now = chrono::Utc::now().timestamp();
        let _guard = self.acquire_task_list_lock("stop task")?;
        let mut tasks = self.load_repository_tasks_strict()?;
        if let Some(entry) = tasks.get_mut(id) {
            entry.cancel_requested_at = Some(now);
            entry.status = TaskStatus::Cancelled;
            entry.updated_at = now;
            let cloned = entry.clone();
            self.persist_entry_strict(&cloned)?;
            self.replace_tasks(tasks);
            Ok(Some(cloned))
        } else {
            self.replace_tasks(tasks);
            Ok(None)
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

    pub(super) fn acquire_task_list_lock(&self, operation: &str) -> Result<TaskListLock> {
        TaskListLock::acquire(self.repository.dir.clone()).with_context(|| {
            format!(
                "failed to acquire task-list lock for {operation} in {}",
                self.repository.dir.display()
            )
        })
    }

    pub(super) fn load_repository_tasks(&self) -> HashMap<String, TaskEntry> {
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

    pub(super) fn load_repository_tasks_strict(&self) -> Result<HashMap<String, TaskEntry>> {
        self.repository
            .load_for_live_refresh()
            .context("failed to refresh persisted tasks")
    }

    pub(super) fn refresh_from_repository(&self) {
        let tasks = self.load_repository_tasks();
        self.replace_tasks(tasks);
    }

    pub(super) fn replace_tasks(&self, tasks: HashMap<String, TaskEntry>) {
        *self.tasks.lock() = tasks;
    }

    pub(super) fn persist_entry(&self, entry: &TaskEntry) {
        if let Err(err) = self.repository.persist_entry(entry) {
            tracing::warn!(
                task_id = %entry.id,
                error = %err,
                "failed to persist task"
            );
        }
    }

    pub(super) fn persist_entry_strict(&self, entry: &TaskEntry) -> Result<()> {
        self.repository
            .persist_entry(entry)
            .with_context(|| format!("failed to persist task {}", entry.id))
    }

    pub(super) fn refresh_remote_review_timeouts(&self) {
        let ids: Vec<String> = self.tasks.lock().keys().cloned().collect();
        for id in ids {
            self.refresh_remote_review_timeout(&id);
        }
    }

    pub(super) fn refresh_remote_review_timeout(&self, id: &str) -> Option<TaskEntry> {
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
