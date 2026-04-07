//! Task management tools — create, get, update, list, stop, and output.
//!
//! Tasks are tracked in a shared in-memory `TaskStore` backed by
//! `Arc<Mutex<HashMap>>`. Each tool gets a clone of the store so all
//! tools operate on the same underlying data.
//!
//! Reference: TypeScript `src/tools/TaskCreateTool/`, `TaskGetTool/`, etc.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// =============================================================================
// TaskStore — shared state
// =============================================================================

/// In-memory task store (shared across all task tools).
#[derive(Debug, Clone, Default)]
pub struct TaskStore {
    tasks: Arc<Mutex<HashMap<String, TaskEntry>>>,
}

/// A single task entry.
#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub output: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Task lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Stopped,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Stopped => "stopped",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "in_progress" => Some(TaskStatus::InProgress),
            "completed" => Some(TaskStatus::Completed),
            "stopped" => Some(TaskStatus::Stopped),
            _ => None,
        }
    }
}

impl TaskStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, subject: &str, description: &str) -> TaskEntry {
        let now = chrono::Utc::now().timestamp();
        let id = uuid::Uuid::new_v4().to_string();
        let entry = TaskEntry {
            id: id.clone(),
            subject: subject.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            output: String::new(),
            created_at: now,
            updated_at: now,
        };
        self.tasks.lock().expect("task store lock poisoned").insert(id, entry.clone());
        entry
    }

    pub fn get(&self, id: &str) -> Option<TaskEntry> {
        self.tasks.lock().expect("task store lock poisoned").get(id).cloned()
    }

    pub fn update_status(&self, id: &str, status: TaskStatus) -> Option<TaskEntry> {
        let mut tasks = self.tasks.lock().expect("task store lock poisoned");
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = status;
            entry.updated_at = chrono::Utc::now().timestamp();
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Append output text to a task's log (used by background agent execution).
    #[allow(dead_code)] // Will be used when background agent execution is implemented
    pub fn append_output(&self, id: &str, output: &str) -> Option<TaskEntry> {
        let mut tasks = self.tasks.lock().expect("task store lock poisoned");
        if let Some(entry) = tasks.get_mut(id) {
            if !entry.output.is_empty() {
                entry.output.push('\n');
            }
            entry.output.push_str(output);
            entry.updated_at = chrono::Utc::now().timestamp();
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn list(&self) -> Vec<TaskEntry> {
        let tasks = self.tasks.lock().expect("task store lock poisoned");
        let mut entries: Vec<TaskEntry> = tasks.values().cloned().collect();
        entries.sort_by_key(|e| e.created_at);
        entries
    }

    pub fn stop(&self, id: &str) -> Option<TaskEntry> {
        self.update_status(id, TaskStatus::Stopped)
    }
}

fn task_to_json(entry: &TaskEntry) -> Value {
    json!({
        "id": entry.id,
        "subject": entry.subject,
        "description": entry.description,
        "status": entry.status.as_str(),
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
    })
}

// =============================================================================
// Global task store (lazy singleton)
// =============================================================================

static GLOBAL_STORE: std::sync::LazyLock<TaskStore> =
    std::sync::LazyLock::new(TaskStore::new);

fn store() -> &'static TaskStore {
    &GLOBAL_STORE
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
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
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

        let entry = store().create(subject, description);

        Ok(ToolResult {
            data: json!({
                "task": task_to_json(&entry),
                "message": format!("Created task: {}", entry.subject)
            }),
            new_messages: vec![],
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
        let id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match store().get(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({ "task": task_to_json(&entry) }),
                new_messages: vec![],
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
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
                    "enum": ["in_progress", "completed"],
                    "description": "New status for the task"
                }
            },
            "required": ["task_id", "status"]
        })
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let status_str = input
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("in_progress");

        let status = TaskStatus::from_str(status_str).unwrap_or(TaskStatus::InProgress);

        match store().update_status(id, status) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task": task_to_json(&entry),
                    "message": format!("Task '{}' updated to {}", entry.subject, status.as_str())
                }),
                new_messages: vec![],
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
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
        "Stop a running task.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to stop"
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
        let id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match store().stop(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task": task_to_json(&entry),
                    "message": format!("Task '{}' stopped", entry.subject)
                }),
                new_messages: vec![],
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Stop a running task.".to_string()
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
        "Get the output/log of a task.".to_string()
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
        let id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match store().get(id) {
            Some(entry) => Ok(ToolResult {
                data: json!({
                    "task_id": entry.id,
                    "subject": entry.subject,
                    "output": if entry.output.is_empty() {
                        "(no output yet)".to_string()
                    } else {
                        entry.output
                    }
                }),
                new_messages: vec![],
            }),
            None => Ok(ToolResult {
                data: json!({ "error": format!("Task not found: {}", id) }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Get the output or logs from a task.".to_string()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_store_create_and_get() {
        let store = TaskStore::new();
        let task = store.create("Test task", "Do the thing");
        assert_eq!(task.subject, "Test task");
        assert_eq!(task.description, "Do the thing");
        assert_eq!(task.status, TaskStatus::Pending);

        let fetched = store.get(&task.id).unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[test]
    fn test_task_store_update_status() {
        let store = TaskStore::new();
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
        let store = TaskStore::new();
        store.create("Task A", "First");
        store.create("Task B", "Second");
        let list = store.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_task_store_stop() {
        let store = TaskStore::new();
        let task = store.create("Stop me", "...");
        let stopped = store.stop(&task.id).unwrap();
        assert_eq!(stopped.status, TaskStatus::Stopped);
    }

    #[test]
    fn test_task_store_append_output() {
        let store = TaskStore::new();
        let task = store.create("Output task", "...");
        store.append_output(&task.id, "line 1");
        store.append_output(&task.id, "line 2");
        let entry = store.get(&task.id).unwrap();
        assert_eq!(entry.output, "line 1\nline 2");
    }

    #[test]
    fn test_task_store_not_found() {
        let store = TaskStore::new();
        assert!(store.get("nonexistent").is_none());
        assert!(store.update_status("nonexistent", TaskStatus::Completed).is_none());
        assert!(store.stop("nonexistent").is_none());
    }

    #[test]
    fn test_task_status_roundtrip() {
        for status in [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Completed,
            TaskStatus::Stopped,
        ] {
            let s = status.as_str();
            assert_eq!(TaskStatus::from_str(s), Some(status));
        }
        assert_eq!(TaskStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_task_to_json() {
        let store = TaskStore::new();
        let task = store.create("JSON test", "desc");
        let json = task_to_json(&task);
        assert_eq!(json["subject"], "JSON test");
        assert_eq!(json["description"], "desc");
        assert_eq!(json["status"], "pending");
    }
}
