use super::*;

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
            task_store.try_create_with_options(
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
                },
            )
        } else {
            task_store.try_create(subject, description)
        }?;

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
            let deleted = task_store.try_delete(id)?.is_some();
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
                task_store.try_update_fields(id, updates)?.unwrap_or(entry)
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

        match task_store.try_update_fields(id, updates)? {
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
        match task_store.try_stop(id)? {
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
