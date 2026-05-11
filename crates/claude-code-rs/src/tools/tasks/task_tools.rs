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
        cc_tools::task_specs::TODO_WRITE_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Replace the current session todo list with the provided todos array.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::todo_write_schema()
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

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        cc_tools::task_specs::TASK_CREATE_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Create a new task to track work progress.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_create_schema()
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

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        cc_tools::task_specs::TASK_GET_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Get details of a task by ID.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_get_schema()
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

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        cc_tools::task_specs::TASK_UPDATE_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Update a task's status.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_update_schema()
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

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        cc_tools::task_specs::TASK_LIST_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "List all tasks and their statuses.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_list_schema()
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

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        cc_tools::task_specs::TASK_STOP_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Cancel a running task.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_stop_schema()
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
