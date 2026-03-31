#![allow(unused)]
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// SkillTool — invoke user-defined skills (slash command wrappers)
pub struct SkillTool;

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str { "Skill" }

    async fn description(&self, _: &Value) -> String {
        "Execute a skill within the conversation.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill": { "type": "string", "description": "The skill name" },
                "args": { "type": "string", "description": "Optional arguments" }
            },
            "required": ["skill"]
        })
    }

    async fn call(
        &self, input: Value, _ctx: &ToolUseContext, _p: &AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let skill = input.get("skill").and_then(|v| v.as_str()).unwrap_or("");
        let args = input.get("args").and_then(|v| v.as_str()).unwrap_or("");

        // Skill execution would load the skill definition, expand the prompt,
        // and inject it into the conversation.
        Ok(ToolResult {
            data: json!(format!("Skill '{}' invoked with args: {}", skill, args)),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Invoke skills by name (e.g., /commit, /review-pr).".to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(s) = input.and_then(|v| v.get("skill")).and_then(|v| v.as_str()) {
            format!("Skill({})", s)
        } else {
            "Skill".to_string()
        }
    }
}
