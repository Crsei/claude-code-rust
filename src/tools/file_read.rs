use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    InterruptBehavior, Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult,
};

/// FileReadTool — Read files from the filesystem
///
/// Corresponds to TypeScript: tools/FileReadTool
pub struct FileReadTool;

impl FileReadTool {
    pub fn new() -> Self {
        FileReadTool
    }

    fn parse_input(input: &Value) -> (String, Option<usize>, Option<usize>) {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        (file_path, offset, limit)
    }

    /// Detect if content is likely binary by checking for null bytes
    fn is_binary(content: &[u8]) -> bool {
        let check_len = content.len().min(8192);
        content[..check_len].contains(&0)
    }

    /// Format file content with line numbers (like cat -n)
    fn format_with_line_numbers(
        content: &str,
        offset: usize,
        limit: usize,
    ) -> (String, usize) {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // offset is 1-based line number (0 means start from beginning)
        let start = if offset > 0 { offset - 1 } else { 0 };
        let end = (start + limit).min(total_lines);

        if start >= total_lines {
            return (String::new(), total_lines);
        }

        let mut result = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            let line_num = start + i + 1;
            result.push_str(&format!("{}\t{}\n", line_num, line));
        }

        (result, total_lines)
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    async fn description(&self, _input: &Value) -> String {
        "Reads a file from the local filesystem.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "The line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read"
                }
            },
            "required": ["file_path"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
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
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let (file_path, offset, limit) = Self::parse_input(&input);

        if file_path.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "file_path is required" }),
                new_messages: vec![],
            });
        }

        let path = Path::new(&file_path);

        if !path.exists() {
            return Ok(ToolResult {
                data: json!({ "error": format!("File not found: {}", file_path) }),
                new_messages: vec![],
            });
        }

        if path.is_dir() {
            return Ok(ToolResult {
                data: json!({ "error": format!("Path is a directory, not a file: {}. Use ls via Bash tool to list directory contents.", file_path) }),
                new_messages: vec![],
            });
        }

        // Read file bytes first for binary detection
        let bytes = match tokio::fs::read(&file_path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolResult {
                    data: json!({ "error": format!("Failed to read file: {}", e) }),
                    new_messages: vec![],
                });
            }
        };

        if Self::is_binary(&bytes) {
            return Ok(ToolResult {
                data: json!({ "error": "File appears to be binary. Cannot display binary file contents." }),
                new_messages: vec![],
            });
        }

        let content = String::from_utf8_lossy(&bytes).to_string();

        // Default limit is 2000 lines
        let effective_limit = limit.unwrap_or(2000);
        let effective_offset = offset.unwrap_or(0);

        let (formatted, total_lines) =
            Self::format_with_line_numbers(&content, effective_offset, effective_limit);

        if formatted.is_empty() && total_lines > 0 {
            return Ok(ToolResult {
                data: json!({
                    "output": format!("File has {} lines, but offset {} is beyond the end.", total_lines, effective_offset),
                    "total_lines": total_lines,
                }),
                new_messages: vec![],
            });
        }

        if formatted.is_empty() {
            return Ok(ToolResult {
                data: json!({
                    "output": "(empty file)",
                    "total_lines": 0,
                }),
                new_messages: vec![],
            });
        }

        // Truncate if too large
        let max_chars = self.max_result_size_chars();
        let output = if formatted.len() > max_chars {
            let mut truncated = formatted;
            truncated.truncate(max_chars);
            truncated.push_str("\n... (output truncated)");
            truncated
        } else {
            formatted
        };

        Ok(ToolResult {
            data: json!({
                "output": output,
                "total_lines": total_lines,
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        r#"Reads a file from the local filesystem. Returns file content with line numbers (like cat -n). By default reads up to 2000 lines. Use offset and limit for large files."#.to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Read".to_string()
    }
}
