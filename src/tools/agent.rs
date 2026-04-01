//! Agent tool — spawns a sub-QueryEngine to handle complex tasks.
//!
//! Corresponds to TypeScript: tools/AgentTool/
//!
//! The Agent tool creates a child QueryEngine with its own conversation context,
//! runs the provided prompt through it, and returns the result to the parent.
//! This enables delegation of complex, multi-step tasks to specialized subagents.

#![allow(unused)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::engine::lifecycle::QueryEngine;
use crate::types::app_state::AppState;
use crate::types::config::{QueryEngineConfig, QuerySource};
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// AgentTool — spawns subagent instances to handle complex tasks.
pub struct AgentTool;

#[derive(Deserialize)]
struct AgentInput {
    /// The task/prompt for the subagent to execute.
    prompt: String,
    /// Short (3-5 word) description of the task.
    description: Option<String>,
    /// Type of specialized subagent (e.g., "general-purpose", "Explore", "Plan").
    #[serde(default)]
    subagent_type: Option<String>,
    /// Optional model override for the subagent.
    #[serde(default)]
    model: Option<String>,
    /// Whether to run the agent in the background.
    #[serde(default)]
    run_in_background: bool,
    /// Isolation mode ("worktree" for git worktree isolation).
    #[serde(default)]
    isolation: Option<String>,
}

/// Maximum depth for nested agent spawning to prevent infinite recursion.
const MAX_AGENT_DEPTH: usize = 5;

/// Resolve a model alias ("sonnet", "opus", "haiku") to a full model ID.
fn resolve_model_alias(alias: &str, fallback: &str) -> String {
    match alias {
        "sonnet" => "claude-sonnet-4-20250514".to_string(),
        "opus" => "claude-opus-4-20250514".to_string(),
        "haiku" => "claude-haiku-4-5-20251001".to_string(),
        other => other.to_string(),
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }

    async fn description(&self, _input: &Value) -> String {
        "Launch a new agent to handle complex, multi-step tasks autonomously.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform"
                },
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized agent to use"
                },
                "model": {
                    "type": "string",
                    "enum": ["sonnet", "opus", "haiku"],
                    "description": "Optional model override for this agent"
                },
                "run_in_background": {
                    "type": "boolean",
                    "default": false,
                    "description": "Set to true to run this agent in the background"
                },
                "isolation": {
                    "type": "string",
                    "enum": ["worktree"],
                    "description": "Isolation mode for the agent"
                }
            },
            "required": ["prompt", "description"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: AgentInput = serde_json::from_value(input)?;

        // Check recursion depth
        let current_depth = ctx
            .query_tracking
            .as_ref()
            .map(|t| t.depth)
            .unwrap_or(0);

        if current_depth >= MAX_AGENT_DEPTH {
            bail!(
                "Agent recursion depth limit reached ({}/{}). \
                 Cannot spawn further subagents.",
                current_depth,
                MAX_AGENT_DEPTH
            );
        }

        let description = params
            .description
            .unwrap_or_else(|| "unnamed task".to_string());
        let subagent_type = params
            .subagent_type
            .unwrap_or_else(|| "general-purpose".to_string());

        // Resolve model for the subagent
        let parent_model = ctx.options.main_loop_model.clone();
        let agent_model = params
            .model
            .map(|m| resolve_model_alias(&m, &parent_model))
            .unwrap_or_else(|| parent_model.clone());

        let agent_id = Uuid::new_v4().to_string();

        info!(
            agent_id = %agent_id,
            description = %description,
            subagent_type = %subagent_type,
            model = %agent_model,
            depth = current_depth + 1,
            "spawning subagent"
        );

        // Get tools and cwd for the subagent
        let child_tools = crate::tools::registry::get_all_tools();
        let child_cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        // Build a QueryEngineConfig for the child agent
        let child_config = QueryEngineConfig {
            cwd: child_cwd,
            tools: child_tools,
            custom_system_prompt: ctx.options.custom_system_prompt.clone(),
            append_system_prompt: ctx.options.append_system_prompt.clone(),
            user_specified_model: Some(agent_model.clone()),
            fallback_model: Some(parent_model),
            max_turns: Some(30), // subagents get a smaller turn budget
            max_budget_usd: ctx.options.max_budget_usd,
            task_budget: None,
            verbose: ctx.options.verbose,
            initial_messages: None,
            commands: vec![],
            thinking_config: None,
            json_schema: None,
            replay_user_messages: false,
            include_partial_messages: false,
            persist_session: false,
        };

        // Create and run the child QueryEngine
        let child_engine = QueryEngine::new(child_config);
        let stream = child_engine.submit_message(&params.prompt, QuerySource::Agent(agent_id.clone()));

        // Collect the stream results
        use crate::engine::sdk_types::SdkMessage;
        use futures::StreamExt;

        let mut stream = std::pin::pin!(stream);
        let mut result_text = String::new();
        let mut had_error = false;

        while let Some(msg) = stream.next().await {
            match msg {
                SdkMessage::Assistant(ref assistant_msg) => {
                    // Extract text from the inner message's content blocks
                    for block in &assistant_msg.message.content {
                        if let crate::types::message::ContentBlock::Text { text } = block {
                            if !result_text.is_empty() {
                                result_text.push('\n');
                            }
                            result_text.push_str(&text);
                        }
                    }
                }
                SdkMessage::Result(ref sdk_result) => {
                    if sdk_result.is_error {
                        had_error = true;
                        if !sdk_result.result.is_empty() {
                            result_text = sdk_result.result.clone();
                        }
                    } else if result_text.is_empty() && !sdk_result.result.is_empty() {
                        result_text = sdk_result.result.clone();
                    }
                }
                _ => {}
            }
        }

        if result_text.is_empty() {
            result_text = "(Agent completed with no text output)".to_string();
        }

        debug!(
            agent_id = %agent_id,
            description = %description,
            result_len = result_text.len(),
            error = had_error,
            "subagent completed"
        );

        Ok(ToolResult {
            data: json!(result_text),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use Agent to delegate complex, multi-step tasks to specialized subagents. \
         Each agent runs autonomously with its own context."
            .to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(desc) = input
            .and_then(|v| v.get("description"))
            .and_then(|v| v.as_str())
        {
            format!("Agent({})", desc)
        } else {
            "Agent".to_string()
        }
    }

    fn max_result_size_chars(&self) -> usize {
        200_000
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_alias() {
        assert!(resolve_model_alias("sonnet", "fallback").contains("sonnet"));
        assert!(resolve_model_alias("opus", "fallback").contains("opus"));
        assert!(resolve_model_alias("haiku", "fallback").contains("haiku"));
        assert_eq!(resolve_model_alias("custom-model", "fallback"), "custom-model");
    }

    #[test]
    fn test_agent_tool_schema() {
        let tool = AgentTool;
        let schema = tool.input_json_schema();
        assert!(schema.get("properties").is_some());
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("prompt"));
        assert!(props.contains_key("description"));
        assert!(props.contains_key("subagent_type"));
        assert!(props.contains_key("model"));
        assert!(props.contains_key("run_in_background"));
        assert!(props.contains_key("isolation"));
    }

    #[test]
    fn test_agent_tool_name() {
        let tool = AgentTool;
        assert_eq!(tool.name(), "Agent");
    }

    #[test]
    fn test_agent_user_facing_name() {
        let tool = AgentTool;
        assert_eq!(tool.user_facing_name(None), "Agent");

        let input = json!({"description": "search codebase"});
        assert_eq!(
            tool.user_facing_name(Some(&input)),
            "Agent(search codebase)"
        );
    }

    #[test]
    fn test_agent_concurrency_safe() {
        let tool = AgentTool;
        assert!(tool.is_concurrency_safe(&json!({})));
    }
}
