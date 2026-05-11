use super::*;

fn task_error_result(error: TaskError) -> ToolResult {
    ToolResult {
        data: json!({
            "error": error.message,
            "task_error": error.to_json(),
        }),
        new_messages: vec![],
        ..Default::default()
    }
}

fn default_update_owner(input: &Value, ctx: &ToolUseContext) -> String {
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
        let request = parse_task_create(&input);
        let task_store = store_for_context(ctx);
        let entry = if request.has_options {
            task_store.try_create_with_options(
                &request.subject,
                &request.description,
                request.options,
            )
        } else {
            task_store.try_create(&request.subject, &request.description)
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
        let id = match parse_task_id(&input) {
            Ok(id) => id,
            Err(error) => return Ok(task_error_result(error)),
        };

        let task_store = store_for_context(ctx);
        match task_store.get(&id) {
            Some(entry) => Ok(ToolResult {
                data: json!({ "task": task_to_json_from_store(&task_store, &entry) }),
                new_messages: vec![],
                ..Default::default()
            }),
            None => Ok(task_error_result(TaskError::not_found(id))),
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
        let request = match parse_task_update(&input, default_update_owner(&input, ctx)) {
            Ok(request) => request,
            Err(error) => return Ok(task_error_result(error)),
        };

        let task_store = store_for_context(ctx);
        let existing = task_store.get(&request.id);
        let Some(existing) = existing else {
            return Ok(task_error_result(TaskError::not_found(request.id)));
        };

        if request.action == TaskUpdateAction::Delete {
            let deleted = task_store.try_delete(&request.id)?.is_some();
            return Ok(ToolResult {
                data: json!({
                    "success": deleted,
                    "task_id": request.id,
                    "updated_fields": if deleted { vec!["deleted"] } else { Vec::<&str>::new() },
                    "status_change": if deleted {
                        json!({ "from": existing.status.as_str(), "to": "deleted" })
                    } else {
                        Value::Null
                    },
                    "message": if deleted {
                        format!("Task '{}' deleted", existing.subject)
                    } else {
                        format!("Task not found: {}", request.id)
                    },
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        if request.action == TaskUpdateAction::Claim {
            let entry = match task_store.claim_task(
                &request.id,
                &request.owner,
                request.check_agent_busy,
            ) {
                Ok(entry) => entry,
                Err(failure) => {
                    let error = TaskError::claim_failed(failure.reason.as_str());
                    return Ok(ToolResult {
                        data: json!({
                            "error": error.message,
                            "task_error": error.to_json(),
                            "claim": failure.to_json(),
                        }),
                        new_messages: vec![],
                        ..Default::default()
                    });
                }
            };
            let mut updates = request.fields.clone();
            updates.status = None;
            updates.owner = None;
            let entry = if updates.subject.is_some()
                || updates.description.is_some()
                || updates.active_form.is_some()
                || updates.metadata_patch.is_some()
                || !updates.add_blocks.is_empty()
                || !updates.add_blocked_by.is_empty()
            {
                task_store
                    .try_update_fields(&request.id, updates)?
                    .unwrap_or(entry)
            } else {
                entry
            };
            return Ok(ToolResult {
                data: json!({
                    "task": task_to_json_from_store(&task_store, &entry),
                    "updated_fields": ["status", "owner"],
                    "status_change": { "from": existing.status.as_str(), "to": TaskStatus::InProgress.as_str() },
                    "message": format!("Task '{}' claimed by {}", entry.subject, request.owner)
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        let updates = request.fields.clone();
        let updated_fields = request.updated_fields;

        match task_store.try_update_fields(&request.id, updates)? {
            Some(entry) => {
                // Fire TaskCompleted hook when status changes to completed.
                if request.status == Some(TaskStatus::Completed)
                    && existing.status != TaskStatus::Completed
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
                        "status_change": request.status.map(|new_status| json!({
                            "from": existing.status.as_str(),
                            "to": new_status.as_str()
                        })),
                        "message": if let Some(status) = request.status {
                            format!("Task '{}' updated to {}", entry.subject, status.as_str())
                        } else {
                            format!("Task '{}' updated", entry.subject)
                        }
                    }),
                    new_messages: vec![],
                    ..Default::default()
                })
            }
            None => Ok(task_error_result(TaskError::not_found(request.id))),
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
        let id = match parse_task_id(&input) {
            Ok(id) => id,
            Err(error) => return Ok(task_error_result(error)),
        };

        let task_store = store_for_context(ctx);
        match task_store.try_stop(&id)? {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task": task_to_json_from_store(&task_store, &entry),
                    "message": format!("Task '{}' cancelled", entry.subject)
                }),
                new_messages: vec![],
                ..Default::default()
            }),
            None => Ok(task_error_result(TaskError::not_found(id))),
        }
    }

    async fn prompt(&self) -> String {
        "Cancel a running task.".to_string()
    }
}
