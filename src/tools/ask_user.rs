#![allow(unused)]
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct AskUserQuestionTool;

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str { "AskUserQuestion" }

    async fn description(&self, _input: &Value) -> String {
        "Ask the user a question and wait for their response.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "The question to ask" }
            },
            "required": ["question"]
        })
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let question = input.get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("(no question provided)");

        if ctx.options.is_non_interactive_session {
            return Ok(ToolResult {
                data: json!("Cannot ask user in non-interactive session. Proceed with your best judgment."),
                new_messages: vec![],
            });
        }

        // In interactive mode, the UI layer handles the actual prompting.
        // This tool just signals that a question needs to be asked.
        Ok(ToolResult {
            data: json!(format!("Question asked: {}", question)),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use AskUserQuestion when you need clarification from the user.".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "AskUserQuestion".to_string()
    }
}
