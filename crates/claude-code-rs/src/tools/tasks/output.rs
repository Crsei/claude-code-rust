use super::*;

pub struct TaskOutputTool;
pub(super) use cc_tasks::{TaskOutputRetrievalStatus, TaskOutputWaitResult};

pub(super) fn parse_task_output_timeout_ms(input: &Value) -> Result<u64> {
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

pub(super) fn task_output_payload(
    entry: &TaskEntry,
    retrieval_status: TaskOutputRetrievalStatus,
) -> Value {
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

pub(super) async fn wait_for_task_output(
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
        cc_tools::task_specs::TASK_OUTPUT_NAME
    }

    async fn description(&self, _: &Value) -> String {
        "Get the retained output/log of a task.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        cc_tools::task_specs::task_output_schema()
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
