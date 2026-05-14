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
#[cfg(test)]
use cc_tasks::dependency_ids_from_input;
#[cfg(test)]
use cc_tasks::TaskCreateOptions;
#[cfg(test)]
use cc_tasks::REMOTE_REVIEW_TIMEOUT_MS;
use cc_tasks::{
    normalize_dependencies, normalize_new_status, normalize_optional_string, parse_task_create,
    parse_task_id, parse_task_update, recover_task_after_restart, remote_review_timed_out,
    TaskEntry, TaskError, TaskStatus, TaskUpdateAction, TeammateTaskExitReason,
    UnassignTeammateTasksResult, DEFAULT_TASK_LIST_ID, REMOTE_TASK_TYPE_AUTOFIX_PR,
    REMOTE_TASK_TYPE_BACKGROUND_PR, REMOTE_TASK_TYPE_REMOTE_AGENT, REMOTE_TASK_TYPE_ULTRAPLAN,
    REMOTE_TASK_TYPE_ULTRAREVIEW, TASK_KIND_DREAM, TASK_KIND_IN_PROCESS_TEAMMATE,
    TASK_KIND_LOCAL_AGENT, TASK_KIND_LOCAL_BASH, TASK_KIND_LOCAL_WORKFLOW, TASK_KIND_MONITOR_MCP,
    TASK_KIND_REMOTE_AGENT, TASK_KIND_TOOL,
};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;

use cc_engine::types::tool::*;
use cc_types::message::AssistantMessage;

mod json;
mod lists;
mod output;
mod repository;
mod store;
mod task_tools;
mod todo;

#[cfg(test)]
mod tests;

use json::task_to_json_from_store;
use lists::task_list_dir;
pub use lists::unassign_teammate_tasks;
#[cfg(test)]
use lists::{sanitize_task_list_id, task_list_id_for_context};
use lists::{store, store_for_context, TaskListLock};
pub use output::TaskOutputTool;
use repository::TaskRepository;
pub use store::TaskStore;
pub use task_tools::{
    TaskCreateTool, TaskGetTool, TaskListTool, TaskStopTool, TaskUpdateTool, TodoWriteTool,
};
use todo::{parse_todo_items, replace_todos_for_key, todo_owner_key};

#[cfg(test)]
use cc_tasks::{PersistedTaskFile, TaskClaimFailureReason};
#[cfg(test)]
use lists::task_store_for_task_list_id;
#[cfg(test)]
use output::{
    parse_task_output_timeout_ms, task_output_payload, wait_for_task_output,
    TaskOutputRetrievalStatus, TaskOutputWaitResult,
};
#[cfg(test)]
use todo::todo_snapshot_for_key;

const TASK_SCHEMA_VERSION: u32 = 5;
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const OUTPUT_SUMMARY_MAX_CHARS: usize = 2_000;
const DEFAULT_TASK_OUTPUT_TIMEOUT_MS: u64 = cc_tools::task_specs::TASK_OUTPUT_DEFAULT_TIMEOUT_MS;
const MAX_TASK_OUTPUT_TIMEOUT_MS: u64 = cc_tools::task_specs::TASK_OUTPUT_MAX_TIMEOUT_MS;
const TASK_OUTPUT_POLL_INTERVAL_MS: u64 = 100;
const TASK_HIGHWATERMARK_FILE: &str = ".highwatermark";
const TASK_HIGHWATERMARK_LOCK_FILE: &str = ".highwatermark.lock";
const TASK_HIGHWATERMARK_LOCK_RETRIES: usize = 30;
const TASK_LIST_LOCK_FILE: &str = ".lock";
#[cfg(not(test))]
const TASK_LIST_LOCK_RETRIES: usize = 30;
#[cfg(test)]
const TASK_LIST_LOCK_RETRIES: usize = 3;
const CC_RUST_TASK_LIST_ID_ENV: &str = "CC_RUST_TASK_LIST_ID";
const CLAUDE_CODE_TASK_LIST_ID_ENV: &str = "CLAUDE_CODE_TASK_LIST_ID";
const CLAUDE_CODE_TEAM_NAME_ENV: &str = "CLAUDE_CODE_TEAM_NAME";

// TaskStore shared state lives in tasks/store.rs.

// Durable repository lives in tasks/repository.rs.

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

// Task-list registry, locking, and migration live in tasks/lists.rs.

fn plan_workflow_cwd() -> PathBuf {
    let cwd = cc_bootstrap::state::original_cwd();
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
) -> Result<Option<cc_types::plan_workflow::PlanWorkflowRecord>> {
    let cwd = plan_workflow_cwd();
    let existing = match cc_commands::plan_workflow::load(&cwd) {
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
    let slot: Arc<Mutex<Option<Option<cc_types::plan_workflow::PlanWorkflowRecord>>>> =
        Arc::new(Mutex::new(None));
    let slot_for_update = Arc::clone(&slot);

    (ctx.set_app_state)(Box::new(move |mut state| {
        let linked = cc_commands::plan_workflow::maybe_link_implementation_task_state(
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
        cc_commands::plan_workflow::persist(&persist_cwd, record)?;
    }
    Ok(record)
}

/// Read-only handle to the global task store, exposed for command surfaces
/// (`/tasks`) that want to enumerate tool-driven tasks without running a
/// tool call. The store is cheap to clone: all interior state is behind `Arc`.
pub fn global_store() -> TaskStore {
    store()
}

// Task tool implementations live in tasks/task_tools.rs.

// TaskOutputTool lives in tasks/output.rs.

// Tests live in tasks/tests.rs.

/// Task-domain tool adapters.
///
/// Runtime behavior still lives in this root module while the workspace split
/// moves pure specs and task-domain types into leaf crates.
pub fn tools() -> Tools {
    vec![
        Arc::new(TodoWriteTool),
        Arc::new(TaskCreateTool),
        Arc::new(TaskGetTool),
        Arc::new(TaskUpdateTool),
        Arc::new(TaskListTool),
        Arc::new(TaskStopTool),
        Arc::new(TaskOutputTool),
    ]
}
