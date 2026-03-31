#![allow(unused)]
//! Phase 12: WebFetchTool (network required)
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "WebFetch" }
    async fn description(&self, _: &Value) -> String { "Fetch content from a URL.".to_string() }
    fn input_json_schema(&self) -> Value {
        json!({ "type": "object", "properties": {
            "url": { "type": "string", "description": "URL to fetch" },
            "prompt": { "type": "string", "description": "Instructions for content extraction" }
        }, "required": ["url"] })
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
        let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");

        #[cfg(feature = "network")]
        {
            let resp = reqwest::get(url).await?;
            let status = resp.status().as_u16();
            let body = resp.text().await?;
            let truncated = if body.len() > 50000 { &body[..50000] } else { &body };
            Ok(ToolResult {
                data: json!({ "status": status, "content": truncated }),
                new_messages: vec![],
            })
        }

        #[cfg(not(feature = "network"))]
        {
            Ok(ToolResult {
                data: json!("WebFetch requires 'network' feature to be enabled"),
                new_messages: vec![],
            })
        }
    }

    async fn prompt(&self) -> String { "Fetch web content from URLs.".to_string() }
}
