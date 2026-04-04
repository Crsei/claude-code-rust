//! PowerShell tool -- execute PowerShell commands.
//!
//! Similar to BashTool but invokes PowerShell instead of sh/bash.
//! On Windows, uses `powershell.exe -NoProfile -NonInteractive -Command`.
//! On non-Windows, uses `pwsh -NoProfile -NonInteractive -Command`.

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::{
    InterruptBehavior, PermissionResult, Tool, ToolProgress, ToolResult, ToolUseContext,
    ValidationResult,
};

use super::bash::truncate_output;

/// PowerShellTool -- execute PowerShell commands.
pub struct PowerShellTool;

impl PowerShellTool {
    fn parse_input(input: &Value) -> (String, u64) {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(120_000);
        (command, timeout_ms)
    }

    /// Return the PowerShell executable name for the current platform.
    fn powershell_executable() -> &'static str {
        if cfg!(target_os = "windows") {
            "powershell.exe"
        } else {
            "pwsh"
        }
    }
}

#[async_trait]
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "PowerShell"
    }

    async fn description(&self, _input: &Value) -> String {
        "Executes a PowerShell command and returns its output.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The PowerShell command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (default 120000, max 600000)"
                }
            },
            "required": ["command"]
        })
    }

    fn is_enabled(&self) -> bool {
        // On Windows, PowerShell is always available.
        // On other platforms, check for pwsh.
        if cfg!(target_os = "windows") {
            true
        } else {
            // Best-effort check: see if pwsh is on PATH
            std::process::Command::new("pwsh")
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
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

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
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
        let (command, timeout_ms) = Self::parse_input(&input);

        if command.is_empty() {
            return Ok(ToolResult {
                data: json!({ "error": "Command must not be empty" }),
                new_messages: vec![],
            });
        }

        let exe = Self::powershell_executable();
        let mut cmd = tokio::process::Command::new(exe);
        cmd.arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(&command);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Cap timeout at 600_000 ms (10 min)
        let capped_ms = timeout_ms.min(600_000);
        let timeout_duration = Duration::from_millis(capped_ms);

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

                let max_chars = self.max_result_size_chars();
                combined = truncate_output(&combined, max_chars);

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
                data: json!({ "error": format!("Failed to execute PowerShell command: {}", e) }),
                new_messages: vec![],
            }),
            Err(_) => Ok(ToolResult {
                data: json!({ "error": format!("PowerShell command timed out after {}ms", capped_ms) }),
                new_messages: vec![],
            }),
        }
    }

    async fn prompt(&self) -> String {
        "Executes a PowerShell command and returns its output.\n\n\
Use this tool when you need to run PowerShell-specific commands or cmdlets.\n\
On Windows, uses powershell.exe; on other platforms, uses pwsh (PowerShell Core).\n\n\
- The command is passed via `-Command` so you can use full PowerShell syntax.\n\
- Default timeout is 120 seconds (120000 ms). Maximum is 600 seconds.\n\
- stdout and stderr are captured separately.\n\
- For simple shell commands, prefer the Bash tool instead."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "PowerShell".to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_powershell_tool_name() {
        let tool = PowerShellTool;
        assert_eq!(tool.name(), "PowerShell");
    }

    #[test]
    fn test_powershell_schema() {
        let tool = PowerShellTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("command"));
        assert!(props.contains_key("timeout"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("command")));
    }

    #[test]
    fn test_powershell_parse_input_defaults() {
        let input = json!({ "command": "Get-Date" });
        let (cmd, timeout) = PowerShellTool::parse_input(&input);
        assert_eq!(cmd, "Get-Date");
        assert_eq!(timeout, 120_000);
    }

    #[test]
    fn test_powershell_parse_input_custom_timeout() {
        let input = json!({ "command": "dir", "timeout": 5000 });
        let (cmd, timeout) = PowerShellTool::parse_input(&input);
        assert_eq!(cmd, "dir");
        assert_eq!(timeout, 5000);
    }

    #[test]
    fn test_powershell_is_destructive() {
        let tool = PowerShellTool;
        assert!(tool.is_destructive(&json!({})));
    }

    #[test]
    fn test_powershell_not_read_only() {
        let tool = PowerShellTool;
        assert!(!tool.is_read_only(&json!({})));
    }

    #[test]
    fn test_powershell_executable_name() {
        let exe = PowerShellTool::powershell_executable();
        if cfg!(target_os = "windows") {
            assert_eq!(exe, "powershell.exe");
        } else {
            assert_eq!(exe, "pwsh");
        }
    }
}
