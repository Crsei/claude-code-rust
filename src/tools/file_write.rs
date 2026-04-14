use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult};

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
        input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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
        ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let (file_path, content) = Self::parse_input(&input);

        if file_path.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "file_path is required" }),
                new_messages: vec![],
                ..Default::default()
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
                        ..Default::default()
                    });
                }
            }
        }

        // Write the file
        match tokio::fs::write(&file_path, &content).await {
            Ok(()) => {
                let line_count = content.lines().count();
                let byte_count = content.len();

                // Fire FileChanged hook
                {
                    let app_state = (ctx.get_app_state)();
                    let configs =
                        crate::tools::hooks::load_hook_configs(&app_state.hooks, "FileChanged");
                    if !configs.is_empty() {
                        let payload = json!({
                            "file_path": &file_path,
                            "operation": "write",
                            "byte_count": byte_count,
                            "line_count": line_count,
                        });
                        let _ =
                            crate::tools::hooks::run_event_hooks("FileChanged", &payload, &configs)
                                .await;
                    }
                }

                Ok(ToolResult {
                    data: json!({
                        "output": format!(
                            "Successfully wrote {} bytes ({} lines) to {}",
                            byte_count, line_count, file_path
                        ),
                        "path": file_path,
                    }),
                    new_messages: vec![],
                    ..Default::default()
                })
            }
            Err(e) => Ok(ToolResult {
                data: json!({ "error": format!("Failed to write file: {}", e) }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Writes a file to the local filesystem.\n\n\
Usage:\n\
- This tool will overwrite the existing file if there is one at the provided path.\n\
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.\n\
- Prefer the Edit tool for modifying existing files \u{2014} it only sends the diff. Only use this tool to create new files or for complete rewrites.\n\
- NEVER create documentation files (*.md) or README files unless explicitly requested by the User.\n\
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Write".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_name() {
        assert_eq!(FileWriteTool::new().name(), "Write");
    }

    #[test]
    fn test_schema_has_required_fields() {
        let schema = FileWriteTool::new().input_json_schema();
        let props = schema.get("properties").unwrap();
        assert!(props.get("file_path").is_some());
        assert!(props.get("content").is_some());
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("file_path")));
        assert!(required.contains(&json!("content")));
    }

    #[test]
    fn test_parse_input_full() {
        let input = json!({"file_path": "/tmp/test.txt", "content": "hello"});
        let (path, content) = FileWriteTool::parse_input(&input);
        assert_eq!(path, "/tmp/test.txt");
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_parse_input_missing() {
        let input = json!({});
        let (path, content) = FileWriteTool::parse_input(&input);
        assert_eq!(path, "");
        assert_eq!(content, "");
    }

    #[test]
    fn test_is_destructive() {
        let tool = FileWriteTool::new();
        assert!(tool.is_destructive(&json!({})));
        assert!(!tool.is_read_only(&json!({})));
        assert!(!tool.is_concurrency_safe(&json!({})));
    }

    #[test]
    fn test_get_path() {
        let tool = FileWriteTool::new();
        assert_eq!(
            tool.get_path(&json!({"file_path": "/a/b.rs"})),
            Some("/a/b.rs".to_string())
        );
        assert_eq!(tool.get_path(&json!({})), None);
    }

    #[tokio::test]
    async fn test_write_and_read_back() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("output.txt");
        let content = "line1\nline2\nline3";

        // Test the actual write via tokio::fs (same as what call() uses)
        tokio::fs::write(&file_path, content).await.unwrap();
        let read_back = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_back, content);
        assert_eq!(content.lines().count(), 3);
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("c").join("test.txt");

        // Simulate what call() does
        if let Some(parent) = nested.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        tokio::fs::write(&nested, "hello").await.unwrap();
        assert!(nested.exists());
    }
}
