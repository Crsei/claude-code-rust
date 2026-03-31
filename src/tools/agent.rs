#![allow(unused)]
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// AgentTool — spawns subagent instances to handle complex tasks
pub struct AgentTool;

#[derive(Deserialize)]
struct AgentInput {
    prompt: String,
    description: Option<String>,
    #[serde(default)]
    subagent_type: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    run_in_background: bool,
    #[serde(default)]
    isolation: Option<String>,
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str { "Agent" }

    async fn description(&self, _input: &Value) -> String {
        "Launch a new agent to handle complex, multi-step tasks autonomously.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Task for the agent" },
                "description": { "type": "string", "description": "Short 3-5 word description" },
                "subagent_type": { "type": "string" },
                "model": { "type": "string", "enum": ["sonnet", "opus", "haiku"] },
                "run_in_background": { "type": "boolean", "default": false },
                "isolation": { "type": "string", "enum": ["worktree"] }
            },
            "required": ["prompt", "description"]
        })
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: AgentInput = serde_json::from_value(input)?;

        // Subagent execution would create a new QueryEngine with scoped context
        // For now, return a placeholder indicating the agent was "launched"
        let result = format!(
            "Agent launched: {}\nSubagent type: {}\nBackground: {}",
            params.description.unwrap_or_else(|| "unnamed".to_string()),
            params.subagent_type.unwrap_or_else(|| "general-purpose".to_string()),
            params.run_in_background
        );

        Ok(ToolResult {
            data: json!(result),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use Agent to delegate complex tasks to specialized subagents.".to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(desc) = input.and_then(|v| v.get("description")).and_then(|v| v.as_str()) {
            format!("Agent({})", desc)
        } else {
            "Agent".to_string()
        }
    }

    fn max_result_size_chars(&self) -> usize { 200_000 }
}
