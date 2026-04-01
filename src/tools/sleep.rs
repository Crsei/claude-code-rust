//! SleepTool — pause execution for a specified duration.
//!
//! Simple tool that wraps `tokio::time::sleep`.
//! Useful for polling loops or waiting for external processes.
//!
//! Reference: TypeScript SleepTool

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// Maximum sleep duration in milliseconds (5 minutes)
const MAX_SLEEP_MS: u64 = 300_000;

pub struct SleepTool;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "Sleep"
    }

    async fn description(&self, _: &Value) -> String {
        "Pause execution for a specified duration in milliseconds.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "duration_ms": {
                    "type": "integer",
                    "description": "Duration to sleep in milliseconds (max 300000 = 5 minutes)",
                    "minimum": 0,
                    "maximum": 300000
                }
            },
            "required": ["duration_ms"]
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
        let duration_ms = input
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .min(MAX_SLEEP_MS);

        tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;

        Ok(ToolResult {
            data: json!({
                "message": format!("Slept for {} ms", duration_ms),
                "duration_ms": duration_ms
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Pause execution. Use sparingly — prefer checking status directly.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = SleepTool;
        assert_eq!(tool.name(), "Sleep");
        assert!(tool.is_concurrency_safe(&json!({})));
        assert!(tool.is_read_only(&json!({})));
    }
}
