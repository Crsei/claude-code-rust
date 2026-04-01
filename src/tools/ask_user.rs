//! AskUser tool — prompts the user for input via terminal interaction.
//!
//! Corresponds to TypeScript: tools/AskUserQuestionTool/
//!
//! In interactive mode, reads a line from stdin. In non-interactive mode,
//! returns a message indicating the user cannot be prompted and the model
//! should proceed with its best judgment.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::debug;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// AskUserQuestion tool — asks the user a question and waits for response.
pub struct AskUserQuestionTool;

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "AskUserQuestion"
    }

    async fn description(&self, _input: &Value) -> String {
        "Ask the user a question and wait for their response.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                }
            },
            "required": ["question"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false // User interaction is inherently serial
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("(no question provided)");

        // Non-interactive sessions cannot prompt the user
        if ctx.options.is_non_interactive_session {
            debug!(question = question, "AskUser: non-interactive session, skipping prompt");
            return Ok(ToolResult {
                data: json!(
                    "Cannot ask user in non-interactive session. \
                     Proceed with your best judgment."
                ),
                new_messages: vec![],
            });
        }

        // Interactive mode: print the question and read from stdin.
        // We use eprintln for the question (so it goes to stderr / visible output)
        // and read the answer from stdin.
        eprintln!("\n{}", question);
        eprint!("> ");

        let answer = read_user_line().await;

        debug!(
            question = question,
            answer_len = answer.len(),
            "AskUser: received user response"
        );

        let response = if answer.trim().is_empty() {
            "(User provided no response)".to_string()
        } else {
            answer
        };

        Ok(ToolResult {
            data: json!(response),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use AskUserQuestion when you need clarification or input from the user.".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "AskUserQuestion".to_string()
    }
}

/// Read a single line from stdin asynchronously.
///
/// Falls back to an empty string if reading fails (e.g., stdin is closed).
async fn read_user_line() -> String {
    // Use tokio::task::spawn_blocking to avoid blocking the async runtime
    // since std::io::stdin().read_line() is a blocking operation.
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(_) => line.trim_end_matches('\n').trim_end_matches('\r').to_string(),
            Err(e) => {
                debug!("AskUser: failed to read stdin: {}", e);
                String::new()
            }
        }
    })
    .await
    .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_user_tool_name() {
        let tool = AskUserQuestionTool;
        assert_eq!(tool.name(), "AskUserQuestion");
    }

    #[test]
    fn test_ask_user_schema() {
        let tool = AskUserQuestionTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("question"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("question")));
    }

    #[test]
    fn test_ask_user_not_concurrency_safe() {
        let tool = AskUserQuestionTool;
        assert!(!tool.is_concurrency_safe(&json!({})));
    }

    #[test]
    fn test_ask_user_is_read_only() {
        let tool = AskUserQuestionTool;
        assert!(tool.is_read_only(&json!({})));
    }

    #[test]
    fn test_ask_user_user_facing_name() {
        let tool = AskUserQuestionTool;
        assert_eq!(tool.user_facing_name(None), "AskUserQuestion");
    }
}
