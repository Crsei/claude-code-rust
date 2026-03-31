#![allow(unused)]
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// In-memory task store (shared across tools)
#[derive(Debug, Clone, Default)]
pub struct TaskStore {
    tasks: Arc<Mutex<HashMap<String, TaskEntry>>>,
}

#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub id: String,
    pub description: String,
    pub status: String,  // "pending", "in_progress", "completed"
    pub created_at: i64,
    pub updated_at: i64,
}

impl TaskStore {
    pub fn new() -> Self { Self::default() }

    pub fn create(&self, id: &str, description: &str) -> TaskEntry {
        let now = chrono::Utc::now().timestamp();
        let entry = TaskEntry {
            id: id.to_string(),
            description: description.to_string(),
            status: "pending".to_string(),
            created_at: now,
            updated_at: now,
        };
        self.tasks.lock().unwrap().insert(id.to_string(), entry.clone());
        entry
    }

    pub fn get(&self, id: &str) -> Option<TaskEntry> {
        self.tasks.lock().unwrap().get(id).cloned()
    }

    pub fn update(&self, id: &str, status: &str) -> Option<TaskEntry> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = status.to_string();
            entry.updated_at = chrono::Utc::now().timestamp();
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn list(&self) -> Vec<TaskEntry> {
        self.tasks.lock().unwrap().values().cloned().collect()
    }

    pub fn stop(&self, id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(entry) = tasks.get_mut(id) {
            entry.status = "stopped".to_string();
            entry.updated_at = chrono::Utc::now().timestamp();
            true
        } else {
            false
        }
    }
}

// ── TaskCreateTool ──

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str { "TaskCreate" }
    async fn description(&self, _: &Value) -> String { "Create a new task.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": {
            "description": { "type": "string" }
        }, "required": ["description"] })
    }
    async fn call(&self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let id = uuid::Uuid::new_v4().to_string();
        Ok(ToolResult { data: json!({ "task_id": id, "description": desc, "status": "pending" }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "Create tasks to track progress.".to_string() }
}

// ── TaskGetTool ──

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str { "TaskGet" }
    async fn description(&self, _: &Value) -> String { "Get task details by ID.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": { "task_id": { "type": "string" } }, "required": ["task_id"] })
    }
    fn is_concurrency_safe(&self, _: &Value) -> bool { true }
    fn is_read_only(&self, _: &Value) -> bool { true }
    async fn call(&self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        Ok(ToolResult { data: json!({ "task_id": id, "status": "unknown" }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "Get task status.".to_string() }
}

// ── TaskUpdateTool ──

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str { "TaskUpdate" }
    async fn description(&self, _: &Value) -> String { "Update task status.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": {
            "task_id": { "type": "string" },
            "status": { "type": "string", "enum": ["in_progress", "completed"] }
        }, "required": ["task_id", "status"] })
    }
    async fn call(&self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let status = input.get("status").and_then(|v| v.as_str()).unwrap_or("in_progress");
        Ok(ToolResult { data: json!({ "task_id": id, "status": status }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "Update task status.".to_string() }
}

// ── TaskListTool ──

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str { "TaskList" }
    async fn description(&self, _: &Value) -> String { "List all tasks.".to_string() }
    fn input_json_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }
    fn is_concurrency_safe(&self, _: &Value) -> bool { true }
    fn is_read_only(&self, _: &Value) -> bool { true }
    async fn call(&self, _input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        Ok(ToolResult { data: json!({ "tasks": [] }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "List all tasks.".to_string() }
}

// ── TaskStopTool ──

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str { "TaskStop" }
    async fn description(&self, _: &Value) -> String { "Stop a running task.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": { "task_id": { "type": "string" } }, "required": ["task_id"] })
    }
    async fn call(&self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        Ok(ToolResult { data: json!({ "task_id": id, "stopped": true }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "Stop a task.".to_string() }
}

// ── TaskOutputTool ──

pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str { "TaskOutput" }
    async fn description(&self, _: &Value) -> String { "Get task output.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": { "task_id": { "type": "string" } }, "required": ["task_id"] })
    }
    fn is_concurrency_safe(&self, _: &Value) -> bool { true }
    fn is_read_only(&self, _: &Value) -> bool { true }
    async fn call(&self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage, _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
        let id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        Ok(ToolResult { data: json!({ "task_id": id, "output": "" }), new_messages: vec![] })
    }
    async fn prompt(&self) -> String { "Get task output.".to_string() }
}
