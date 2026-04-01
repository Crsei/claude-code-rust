//! SnipTool — manually trim conversation history to free context.
//!
//! Allows the model or user to explicitly discard older messages from
//! the conversation, keeping only the most recent N turns or messages
//! after a given point.
//!
//! Reference: TypeScript SnipTool (manual history trimming)

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct SnipTool;

#[async_trait]
impl Tool for SnipTool {
    fn name(&self) -> &str {
        "Snip"
    }

    async fn description(&self, _: &Value) -> String {
        "Trim conversation history to free context window space.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "keep_last_n": {
                    "type": "integer",
                    "description": "Number of most recent message pairs (user+assistant) to keep. Default: 4",
                    "minimum": 1,
                    "maximum": 100
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for snipping (logged for debugging)"
                }
            }
        })
    }

    fn is_read_only(&self, _: &Value) -> bool {
        false
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let keep_last_n = input
            .get("keep_last_n")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize;

        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("manual snip");

        let total_messages = ctx.messages.len();

        // Each "turn" is roughly 2 messages (user + assistant)
        let keep_count = keep_last_n * 2;
        let to_remove = total_messages.saturating_sub(keep_count);

        if to_remove == 0 {
            return Ok(ToolResult {
                data: json!({
                    "message": "No messages to snip — conversation is already short enough.",
                    "total_messages": total_messages,
                    "kept": total_messages
                }),
                new_messages: vec![],
            });
        }

        // Signal to the query engine that messages should be trimmed.
        // The actual trimming is done by the engine's compaction pipeline
        // when it processes this tool result. We return metadata indicating
        // how many messages to remove.
        Ok(ToolResult {
            data: json!({
                "action": "snip",
                "removed_count": to_remove,
                "kept_count": total_messages - to_remove,
                "total_before": total_messages,
                "reason": reason,
                "message": format!(
                    "Snipped {} messages, keeping last {} turns ({} messages). Reason: {}",
                    to_remove, keep_last_n, total_messages - to_remove, reason
                )
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Manually trim conversation history when approaching context limits.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = SnipTool;
        assert_eq!(tool.name(), "Snip");
        let schema = tool.input_json_schema();
        assert!(schema.get("properties").is_some());
    }
}
