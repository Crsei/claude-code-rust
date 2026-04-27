//! SleepTool -- signals the proactive tick loop to pause for N seconds.
//!
//! This is a KAIROS tool. When the model calls Sleep, it signals the daemon
//! tick loop to set a `sleep_until` marker and stop ticking for the requested
//! duration. The tool itself does NOT actually block -- it only returns a
//! JSON result describing the requested sleep.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::features::{self, Feature};
use crate::types::message::AssistantMessage;
use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult};

/// SleepTool -- signal the proactive tick loop to pause.
pub struct SleepTool;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "Sleep"
    }

    async fn description(&self, _input: &Value) -> String {
        "Pause the proactive tick loop for a specified duration.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "duration_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "description": "Number of seconds to pause the proactive tick loop (1-3600)"
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for pausing (e.g. \"waiting for CI to finish\")"
                }
            },
            "required": ["duration_seconds"]
        })
    }

    fn is_enabled(&self) -> bool {
        features::enabled(Feature::Proactive) || features::enabled(Feature::Kairos)
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let duration = input.get("duration_seconds").and_then(|v| v.as_i64());

        match duration {
            None => ValidationResult::Error {
                message: "\"duration_seconds\" is required".to_string(),
                error_code: 1,
            },
            Some(d) if !(1..=3600).contains(&d) => ValidationResult::Error {
                message: format!("\"duration_seconds\" must be between 1 and 3600, got {}", d),
                error_code: 1,
            },
            Some(_) => ValidationResult::Ok,
        }
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let duration_seconds = input
            .get("duration_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(ToolResult {
            data: json!({
                "status": "sleeping",
                "duration_seconds": duration_seconds,
                "reason": reason,
            }),
            new_messages: vec![],
            ..Default::default()
        })
    }

    async fn prompt(&self) -> String {
        "Use Sleep to pause the proactive tick loop for a specified number of seconds.\n\n\
         When you determine that no further action is needed for a period of time \
         (e.g. waiting for a CI build, a deployment, or an external event), call \
         Sleep with the estimated wait duration. The daemon will stop ticking \
         until the sleep period expires.\n\n\
         Parameters:\n\
         - duration_seconds (required): 1-3600 seconds.\n\
         - reason (optional): A brief explanation of why you are sleeping.\n\n\
         This tool does not block execution -- it signals intent to the tick loop."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "Sleep".to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sleep_tool_name() {
        let tool = SleepTool;
        assert_eq!(tool.name(), "Sleep");
    }

    #[test]
    fn test_sleep_tool_schema() {
        let tool = SleepTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("duration_seconds"));
        assert!(props.contains_key("reason"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("duration_seconds")));
    }

    #[test]
    fn test_sleep_tool_is_read_only() {
        let tool = SleepTool;
        assert!(tool.is_read_only(&json!({})));
    }

    #[test]
    fn test_sleep_tool_validates_missing_duration() {
        let tool = SleepTool;
        let ctx = make_test_ctx();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.validate_input(&json!({}), &ctx));
        match result {
            ValidationResult::Error { message, .. } => {
                assert!(
                    message.contains("required"),
                    "expected 'required' in error: {}",
                    message
                );
            }
            ValidationResult::Ok => panic!("expected error for missing duration_seconds"),
        }
    }

    #[test]
    fn test_sleep_tool_validates_too_high() {
        let tool = SleepTool;
        let ctx = make_test_ctx();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.validate_input(&json!({"duration_seconds": 7200}), &ctx));
        match result {
            ValidationResult::Error { message, .. } => {
                assert!(
                    message.contains("3600"),
                    "expected '3600' in error: {}",
                    message
                );
            }
            ValidationResult::Ok => panic!("expected error for duration_seconds > 3600"),
        }
    }

    #[test]
    fn test_sleep_tool_validates_too_low() {
        let tool = SleepTool;
        let ctx = make_test_ctx();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.validate_input(&json!({"duration_seconds": 0}), &ctx));
        match result {
            ValidationResult::Error { .. } => {}
            ValidationResult::Ok => panic!("expected error for duration_seconds < 1"),
        }
    }

    #[test]
    fn test_sleep_tool_validates_valid_input() {
        let tool = SleepTool;
        let ctx = make_test_ctx();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.validate_input(&json!({"duration_seconds": 60}), &ctx));
        assert!(
            matches!(result, ValidationResult::Ok),
            "expected Ok for valid input"
        );
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_test_ctx() -> ToolUseContext {
        use crate::types::app_state::AppState;
        use crate::types::tool::{FileStateCache, ToolUseOptions};
        use std::sync::Arc;

        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".to_string(),
                verbose: false,
                is_non_interactive_session: true,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(AppState::default),
            set_app_state: Arc::new(|_| {}),
            session_id: "test-session".to_string(),
            langfuse_session_id: "test-session".to_string(),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            ask_user_callback: None,
            bg_agent_tx: None,
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        }
    }
}
