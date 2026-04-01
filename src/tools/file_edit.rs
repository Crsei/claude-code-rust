use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult,
};

/// FileEditTool — Edit a file by replacing exact string matches
///
/// Corresponds to TypeScript: tools/FileEditTool
pub struct FileEditTool;

impl FileEditTool {
    pub fn new() -> Self {
        FileEditTool
    }

    fn parse_input(input: &Value) -> (String, String, String, bool) {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        (file_path, old_string, new_string, replace_all)
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    async fn description(&self, _input: &Value) -> String {
        "Performs exact string replacements in files.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences of old_string (default false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
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
        let old_string = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        if old_string.is_empty() {
            return ValidationResult::Error {
                message: "old_string is required and must not be empty".to_string(),
                error_code: 1,
            };
        }
        let new_string = input.get("new_string").and_then(|v| v.as_str());
        if new_string.is_none() {
            return ValidationResult::Error {
                message: "new_string is required".to_string(),
                error_code: 1,
            };
        }
        if old_string == new_string.unwrap_or("") {
            return ValidationResult::Error {
                message: "old_string and new_string must be different".to_string(),
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
        let (file_path, old_string, new_string, replace_all) = Self::parse_input(&input);

        if file_path.is_empty() || old_string.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "file_path and old_string are required" }),
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

        // Read current content
        let content = match tokio::fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    data: json!({ "error": format!("Failed to read file: {}", e) }),
                    new_messages: vec![],
                });
            }
        };

        // Count occurrences of old_string
        let occurrence_count = content.matches(&old_string).count();

        if occurrence_count == 0 {
            return Ok(ToolResult {
                data: json!({
                    "error": format!(
                        "old_string not found in {}. Make sure the string matches exactly, including whitespace and indentation.",
                        file_path
                    )
                }),
                new_messages: vec![],
            });
        }

        if occurrence_count > 1 && !replace_all {
            return Ok(ToolResult {
                data: json!({
                    "error": format!(
                        "old_string appears {} times in {}. Either provide a larger string with more surrounding context to make it unique, or set replace_all to true.",
                        occurrence_count, file_path
                    )
                }),
                new_messages: vec![],
            });
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(&old_string, &new_string)
        } else {
            // Replace only the first occurrence
            content.replacen(&old_string, &new_string, 1)
        };

        // Write back
        match tokio::fs::write(&file_path, &new_content).await {
            Ok(()) => {
                let replacements = if replace_all { occurrence_count } else { 1 };
                Ok(ToolResult {
                    data: json!({
                        "output": format!(
                            "Successfully replaced {} occurrence(s) in {}",
                            replacements, file_path
                        ),
                        "path": file_path,
                        "replacements": replacements,
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
        "Performs exact string replacements in files.\n\n\
Usage:\n\
- You must use your `Read` tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file. \n\
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: line number + tab. Everything after that is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.\n\
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n\
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.\n\
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.\n\
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance.".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Edit".to_string()
    }
}
