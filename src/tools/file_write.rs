use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult,
};

/// FileWriteTool — Write content to a file
///
/// Corresponds to TypeScript: tools/FileWriteTool
pub struct FileWriteTool;

impl FileWriteTool {
    pub fn new() -> Self {
        FileWriteTool
    }

    fn parse_input(input: &Value) -> (String, String) {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (file_path, content)
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    async fn description(&self, _input: &Value) -> String {
        "Writes a file to the local filesystem.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn is_destructive(&self, _input: &Value) -> bool {
        true
    }

    fn get_path(&self, input: &Value) -> Option<String> {
        input.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        if file_path.is_empty() {
            return ValidationResult::Error {
                message: "file_path is required".to_string(),
                error_code: 1,
            };
        }
        if !input.get("content").and_then(|v| v.as_str()).is_some() {
            return ValidationResult::Error {
                message: "content is required".to_string(),
                error_code: 1,
            };
        }
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let (file_path, content) = Self::parse_input(&input);

        if file_path.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "file_path is required" }),
                new_messages: vec![],
            });
        }

        let path = Path::new(&file_path);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Ok(ToolResult {
                        data: json!({ "error": format!("Failed to create directories: {}", e) }),
                        new_messages: vec![],
                    });
                }
            }
        }

        // Write the file
        match tokio::fs::write(&file_path, &content).await {
            Ok(()) => {
                let line_count = content.lines().count();
                let byte_count = content.len();
                Ok(ToolResult {
                    data: json!({
                        "output": format!(
                            "Successfully wrote {} bytes ({} lines) to {}",
                            byte_count, line_count, file_path
                        ),
                        "path": file_path,
                    }),
                    new_messages: vec![],
                })
            }
            Err(e) => Ok(ToolResult {
                data: json!({ "error": format!("Failed to write file: {}", e) }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        r#"Writes a file to the local filesystem. Will overwrite the existing file if there is one. Creates parent directories if needed."#.to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Write".to_string()
    }
}
