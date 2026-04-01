//! TodoWrite tool — write structured TODO items to a project-local file.
//!
//! Reference: TypeScript `src/tools/TodoWriteTool/`

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    async fn description(&self, _: &Value) -> String {
        "Write or update a TODO list in a structured format.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "List of TODO items to write",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "Unique identifier for the TODO" },
                            "content": { "type": "string", "description": "The TODO content/description" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"],
                                "description": "Status of the TODO item"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["low", "medium", "high"],
                                "description": "Priority level"
                            }
                        },
                        "required": ["id", "content", "status"]
                    }
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to write the TODO file (defaults to .cc-rust/todos.json in cwd)"
                }
            },
            "required": ["todos"]
        })
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let todos = input
            .get("todos")
            .cloned()
            .unwrap_or_else(|| json!([]));

        let file_path = if let Some(fp) = input.get("file_path").and_then(|v| v.as_str()) {
            PathBuf::from(fp)
        } else {
            crate::utils::cwd::get_cwd().join(".cc-rust").join("todos.json")
        };

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Read existing todos if file exists, merge with new
        let mut existing: Vec<Value> = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path).unwrap_or_default();
            serde_json::from_str::<Value>(&content)
                .ok()
                .and_then(|v| v.get("todos").cloned())
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Update or insert todos
        if let Some(new_todos) = todos.as_array() {
            for new_todo in new_todos {
                let new_id = new_todo.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(pos) = existing.iter().position(|e| {
                    e.get("id").and_then(|v| v.as_str()) == Some(new_id)
                }) {
                    existing[pos] = new_todo.clone();
                } else {
                    existing.push(new_todo.clone());
                }
            }
        }

        let output = json!({
            "todos": existing,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });

        std::fs::write(&file_path, serde_json::to_string_pretty(&output)?)?;

        Ok(ToolResult {
            data: json!({
                "message": format!("Updated {} todos in {}", existing.len(), file_path.display()),
                "count": existing.len(),
                "file": file_path.to_string_lossy()
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Write structured TODO items to track project tasks.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = TodoWriteTool;
        assert_eq!(tool.name(), "TodoWrite");
        let schema = tool.input_json_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("todos").is_some());
    }
}
