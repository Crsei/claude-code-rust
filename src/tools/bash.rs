use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    InterruptBehavior, PermissionResult, Tool, ToolProgress, ToolResult, ToolUseContext,
    ValidationResult,
};

/// BashTool — Execute shell commands
///
/// Corresponds to TypeScript: tools/BashTool
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        BashTool
    }

    fn parse_input(input: &Value) -> (String, u64, Option<String>) {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timeout = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(120);
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (command, timeout, description)
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    async fn description(&self, _input: &Value) -> String {
        "Executes a given bash command and returns its output.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)"
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does"
                }
            },
            "required": ["command"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn get_path(&self, _input: &Value) -> Option<String> {
        None
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if command.is_empty() {
            return ValidationResult::Error {
                message: "Command must not be empty".to_string(),
                error_code: 1,
            };
        }
        ValidationResult::Ok
    }

    async fn check_permissions(
        &self,
        input: &Value,
        _ctx: &ToolUseContext,
    ) -> PermissionResult {
        // In a full implementation, this would check dangerous command patterns,
        // permission mode, etc. For now, allow all.
        PermissionResult::Allow {
            updated_input: input.clone(),
        }
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let (command, timeout_secs, _description) = Self::parse_input(&input);

        if command.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "Command must not be empty" }),
                new_messages: vec![],
            });
        }

        // Use sh on Unix, cmd on Windows (but we use bash syntax per project convention)
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("bash");
            c.arg("-c").arg(&command);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(&command);
            c
        };

        // Capture stdout and stderr
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = Duration::from_secs(timeout_secs);

        let result = tokio::time::timeout(timeout_duration, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&stderr);
                }

                // Truncate if too large
                let max_chars = self.max_result_size_chars();
                if combined.len() > max_chars {
                    combined.truncate(max_chars);
                    combined.push_str("\n... (output truncated)");
                }

                Ok(ToolResult {
                    data: json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code,
                        "output": combined,
                    }),
                    new_messages: vec![],
                })
            }
            Ok(Err(e)) => Ok(ToolResult {
                data: json!({ "error": format!("Failed to execute command: {}", e) }),
                new_messages: vec![],
            }),
            Err(_) => Ok(ToolResult {
                data: json!({ "error": format!("Command timed out after {}s", timeout_secs) }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        r#"Executes a given bash command and returns its output. The working directory persists between commands. Use Unix shell syntax."#.to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Bash".to_string()
    }
}
