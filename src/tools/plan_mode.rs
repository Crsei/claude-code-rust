#![allow(unused)]
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// EnterPlanMode — switch to read-only planning mode
pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str { "EnterPlanMode" }
    async fn description(&self, _: &Value) -> String { "Enter plan mode (read-only).".to_string() }
    fn input_json_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }

    async fn call(
        &self, _input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        Ok(ToolResult {
            data: json!("Entered plan mode. Only read-only operations are allowed."),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String { "Enter read-only planning mode.".to_string() }
}

/// ExitPlanMode — return to normal mode
pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str { "ExitPlanMode" }
    async fn description(&self, _: &Value) -> String { "Exit plan mode.".to_string() }
    fn input_json_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }

    async fn call(
        &self, _input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        Ok(ToolResult {
            data: json!("Exited plan mode. Normal operations restored."),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String { "Exit plan mode.".to_string() }
}
