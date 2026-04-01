use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult,
};

/// GlobTool — Find files matching glob patterns
///
/// Corresponds to TypeScript: tools/GlobTool
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        GlobTool
    }

    fn parse_input(input: &Value) -> (String, Option<String>) {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (pattern, path)
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    async fn description(&self, _input: &Value) -> String {
        "Fast file pattern matching tool that works with any codebase size.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. Defaults to the current working directory."
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn get_path(&self, input: &Value) -> Option<String> {
        input.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        if pattern.is_empty() {
            return ValidationResult::Error {
                message: "pattern is required".to_string(),
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
        let (pattern, path) = Self::parse_input(&input);

        if pattern.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "pattern is required" }),
                new_messages: vec![],
            });
        }

        // Determine base directory
        let base_dir = match &path {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };

        if !base_dir.exists() {
            return Ok(ToolResult {
                data: json!({ "error": format!("Directory not found: {}", base_dir.display()) }),
                new_messages: vec![],
            });
        }

        // Build the full glob pattern
        let full_pattern = if pattern.starts_with('/') || pattern.contains(':') {
            // Absolute pattern
            pattern.clone()
        } else {
            // Relative to base_dir
            let base = base_dir.to_string_lossy();
            // Normalize path separators for glob crate
            let base = base.replace('\\', "/");
            if base.ends_with('/') {
                format!("{}{}", base, pattern)
            } else {
                format!("{}/{}", base, pattern)
            }
        };

        // Use the glob crate to find matching files
        // Run in blocking task since glob is synchronous
        let matches = tokio::task::spawn_blocking(move || {
            let mut results: Vec<String> = Vec::new();
            match glob::glob(&full_pattern) {
                Ok(paths) => {
                    for entry in paths {
                        match entry {
                            Ok(path) => {
                                results.push(path.to_string_lossy().to_string());
                            }
                            Err(_) => continue,
                        }
                    }
                }
                Err(e) => {
                    return Err(format!("Invalid glob pattern: {}", e));
                }
            }
            Ok(results)
        })
        .await?;

        match matches {
            Ok(mut files) => {
                // Sort by path for consistent output
                files.sort();

                let count = files.len();
                let output = if files.is_empty() {
                    "No matching files found.".to_string()
                } else {
                    files.join("\n")
                };

                // Truncate if too large
                let max_chars = self.max_result_size_chars();
                let output = if output.len() > max_chars {
                    let mut truncated = output;
                    truncated.truncate(max_chars);
                    truncated.push_str("\n... (output truncated)");
                    truncated
                } else {
                    output
                };

                Ok(ToolResult {
                    data: json!({
                        "output": output,
                        "count": count,
                    }),
                    new_messages: vec![],
                })
            }
            Err(e) => Ok(ToolResult {
                data: json!({ "error": e }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        "- Fast file pattern matching tool that works with any codebase size\n\
- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n\
- Returns matching file paths sorted by modification time\n\
- Use this tool when you need to find files by name patterns\n\
- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Glob".to_string()
    }
}
