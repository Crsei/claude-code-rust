#![allow(unused)]
//! Phase 12: WebSearchTool (network required)
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "WebSearch" }
    async fn description(&self, _: &Value) -> String { "Search the web.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": {
            "query": { "type": "string", "description": "Search query" },
            "max_results": { "type": "number", "default": 5 }
        }, "required": ["query"] })
    }
    fn is_concurrency_safe(&self, _: &Value) -> bool { true }
    fn is_read_only(&self, _: &Value) -> bool { true }

    fn is_enabled(&self) -> bool {
        cfg!(feature = "network")
    }

    async fn call(
        &self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        Ok(ToolResult {
            data: json!("WebSearch requires 'network' feature and API integration"),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String { "Search the web for information.".to_string() }
}
