// src/tools/system_status.rs

//! SystemStatus tool - lets the Agent query subsystem status.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use cc_ipc::system_status_tool::{self, ChromeStatus};

use crate::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, ValidationResult};
use cc_types::message::AssistantMessage;

pub struct SystemStatusTool;

#[async_trait]
impl Tool for SystemStatusTool {
    fn name(&self) -> &str {
        "SystemStatus"
    }

    async fn description(&self, _input: &Value) -> String {
        system_status_tool::system_status_description()
    }

    fn input_json_schema(&self) -> Value {
        system_status_tool::system_status_schema()
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let subsystem = input["subsystem"].as_str().unwrap_or("all");
        let output = format_status_output(subsystem);

        Ok(ToolResult {
            data: json!({ "status": output }),
            model_content: None,
            display_preview: Some(output.clone()),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        system_status_tool::system_status_prompt()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "SystemStatus".to_string()
    }
}

/// Format a human-readable status output for the given subsystem.
fn format_status_output(subsystem: &str) -> String {
    let snap = cc_browser::state::snapshot();
    system_status_tool::format_status_output(
        subsystem,
        Some(ChromeStatus {
            connection_label: snap.connection.label().to_string(),
            extension_detected: snap.extension_installed,
            last_error: snap.last_error,
        }),
    )
}
fn _count_nodes(node: &cc_types::agent_types::AgentNode) -> usize {
    1 + node.children.iter().map(_count_nodes).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_status_tool_name() {
        let tool = SystemStatusTool;
        assert_eq!(tool.name(), "SystemStatus");
    }

    #[test]
    fn system_status_tool_is_read_only() {
        let tool = SystemStatusTool;
        assert!(tool.is_read_only(&json!({})));
    }

    #[test]
    fn system_status_tool_schema_has_subsystem_property() {
        let tool = SystemStatusTool;
        let schema = tool.input_json_schema();
        assert!(schema["properties"]["subsystem"].is_object());
    }

    #[test]
    fn format_status_output_all_returns_all_sections() {
        let output = format_status_output("all");
        assert!(output.contains("## LSP Servers"));
        assert!(output.contains("## MCP Servers"));
        assert!(output.contains("## Plugins"));
        assert!(output.contains("## Skills"));
        assert!(output.contains("## IDE Integrations"));
        assert!(output.contains("## Chrome Integration"));
    }

    #[test]
    fn format_status_output_lsp_only() {
        let output = format_status_output("lsp");
        assert!(output.contains("## LSP Servers"));
        assert!(!output.contains("## MCP Servers"));
    }
}
