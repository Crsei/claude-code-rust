#![allow(unused)]
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct ToolSearchTool;

#[derive(Deserialize)]
struct ToolSearchInput {
    query: String,
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize { 5 }

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str { "ToolSearch" }

    async fn description(&self, _input: &Value) -> String {
        "Search for available tools by keyword.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query for tools" },
                "max_results": { "type": "number", "default": 5 }
            },
            "required": ["query"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }
    fn is_read_only(&self, _input: &Value) -> bool { true }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: ToolSearchInput = serde_json::from_value(input)?;
        let query_lower = params.query.to_lowercase();

        // Search through available tools by name and description
        let mut matches: Vec<Value> = Vec::new();

        // In a full implementation, this would search ctx.options.tools
        // For now, return a placeholder
        let result = json!({
            "message": format!("Tool search for '{}' - found {} results", params.query, matches.len()),
            "tools": matches
        });

        Ok(ToolResult {
            data: result,
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use ToolSearch to find available tools by keyword.".to_string()
    }
}
