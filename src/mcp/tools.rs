#![allow(unused)]
//! MCP tool integration — wraps MCP tools as local Tool trait objects
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;
use super::McpToolDef;

/// Wraps an MCP tool definition as a local Tool
pub struct McpToolWrapper {
    pub def: McpToolDef,
    pub server_name: String,
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str { &self.def.name }

    async fn description(&self, _input: &Value) -> String {
        self.def.description.clone()
    }

    fn input_json_schema(&self) -> Value {
        self.def.input_schema.clone()
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { false }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        // Would delegate to McpClient.call_tool()
        Ok(ToolResult {
            data: json!(format!("MCP tool {} not connected", self.def.name)),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        format!("MCP tool from server '{}'", self.server_name)
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        format!("mcp__{}__{}", self.server_name, self.def.name)
    }
}

/// Convert MCP tool definitions into Tool trait objects
pub fn mcp_tools_to_tools(defs: Vec<McpToolDef>) -> Vec<Arc<dyn Tool>> {
    defs.into_iter()
        .map(|def| {
            let server_name = def.server_name.clone();
            Arc::new(McpToolWrapper { def, server_name }) as Arc<dyn Tool>
        })
        .collect()
}
