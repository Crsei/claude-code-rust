//! Tool trait implementation for AgentTool.

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::QuerySource;
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

use super::{
    build_child_config, collect_stream_result, resolve_model_alias, AgentInput, AgentTool,
    MAX_AGENT_DEPTH,
};

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
        let current_depth = ctx.query_tracking.as_ref().map(|t| t.depth).unwrap_or(0);

        if current_depth >= MAX_AGENT_DEPTH {
            bail!(
                "Agent recursion depth limit reached ({}/{}). \
                 Cannot spawn further subagents.",
                current_depth,
                MAX_AGENT_DEPTH
            );
        }

        let description = params.description.as_deref().unwrap_or("unnamed task");
        let subagent_type = params.subagent_type.as_deref().unwrap_or("general-purpose");

        // Resolve model for the subagent
        let parent_model = ctx.options.main_loop_model.clone();
        let agent_model = params
            .model
            .as_deref()
            .map(|m| resolve_model_alias(m, &parent_model))
            .unwrap_or_else(|| parent_model.clone());

        let agent_id = Uuid::new_v4().to_string();

        // Determine isolation mode before logging (borrow params.isolation)
        let use_worktree = params
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);

        info!(
            agent_id = %agent_id,
            description = %description,
            subagent_type = %subagent_type,
            model = %agent_model,
            depth = current_depth + 1,
            isolation = ?params.isolation,
            "spawning subagent"
        );

        // Load hook configs once (used by both background and synchronous paths)
        let start_configs = {
            let app_state = (ctx.get_app_state)();
            crate::tools::hooks::load_hook_configs(&app_state.hooks, "SubagentStart")
        };
        let stop_configs = {
            let app_state = (ctx.get_app_state)();
            crate::tools::hooks::load_hook_configs(&app_state.hooks, "SubagentStop")
        };

        // -- Background path
        if params.run_in_background {
            let Some(bg_tx) = ctx.bg_agent_tx.clone() else {
                warn!(
                    agent_id = %agent_id,
                    "run_in_background requested but no bg_agent_tx — running synchronously"
                );
                // Fall through to synchronous dispatch below
                return self.run_agent_dispatch(
                    use_worktree, &params, ctx, &agent_id, &agent_model,
                    &parent_model, current_depth, description, &start_configs, &stop_configs,
                ).await;
            };

            // Fire SubagentStart hook before spawn
            if !start_configs.is_empty() {
                let payload = json!({
                    "agent_id": &agent_id,
                    "prompt": &params.prompt,
                    "description": description,
                    "subagent_type": subagent_type,
                    "model": &agent_model,
                    "depth": current_depth + 1,
                    "background": true,
                });
                let _ = crate::tools::hooks::run_event_hooks("SubagentStart", &payload, &start_configs).await;
            }

            // Build child config now (before move into spawn)
            if use_worktree {
                warn!(agent_id = %agent_id, "background + worktree not yet combined — using normal cwd");
            }
            let child_cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());

            let child_config = build_child_config(
                child_cwd, ctx, &agent_id, &agent_model, &parent_model, current_depth,
            );

            // Capture owned values for the spawned task
            let spawn_agent_id = agent_id.clone();
            let spawn_description = description.to_string();
            let spawn_prompt = params.prompt.clone();
            let spawn_stop_configs = stop_configs.clone();

            tokio::spawn(async move {
                let started = std::time::Instant::now();

                let child_engine = QueryEngine::new(child_config);
                let stream = child_engine.submit_message(
                    &spawn_prompt,
                    QuerySource::Agent(spawn_agent_id.clone()),
                );
                let (result_text, had_error) = collect_stream_result(stream).await;

                // Fire SubagentStop hook
                if !spawn_stop_configs.is_empty() {
                    let payload = json!({
                        "agent_id": &spawn_agent_id,
                        "description": &spawn_description,
                        "is_error": had_error,
                        "background": true,
                    });
                    let _ = crate::tools::hooks::run_event_hooks(
                        "SubagentStop", &payload, &spawn_stop_configs,
                    ).await;
                }

                let _ = bg_tx.send(crate::tools::background_agents::CompletedBackgroundAgent {
                    agent_id: spawn_agent_id,
                    description: spawn_description,
                    result_text,
                    had_error,
                    duration: started.elapsed(),
                });
            });

            return Ok(ToolResult {
                data: json!(format!(
                    "Agent '{}' launched in background (id: {}). You will be notified when it completes.",
                    description, agent_id
                )),
                new_messages: vec![],
            });
        }

        // -- Synchronous path
        self.run_agent_dispatch(
            use_worktree, &params, ctx, &agent_id, &agent_model, &parent_model,
            current_depth, description, &start_configs, &stop_configs,
        ).await
    }

    async fn prompt(&self) -> String {
        "Launch a new agent to handle complex, multi-step tasks autonomously.\n\n\
The Agent tool launches specialized agents (subprocesses) that autonomously handle complex tasks. \
Each agent type has specific capabilities and tools available to it.\n\n\
Usage notes:\n\
- Always include a short description (3-5 words) summarizing what the agent will do\n\
- Launch multiple agents concurrently whenever possible, to maximize performance; \
to do that, use a single message with multiple tool uses\n\
- When the agent is done, it will return a single message back to you. \
The result returned by the agent is not visible to the user. \
To show the user the result, you should send a text message back to the user \
with a concise summary of the result.\n\
- Provide clear, detailed prompts so the agent can work autonomously \
and return exactly the information you need.\n\
- The agent's outputs should generally be trusted\n\
- Clearly tell the agent whether you expect it to write code or just to do research \
(search, file reads, web fetches, etc.), since it is not aware of the user's intent"
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // MAX_AGENT_DEPTH constant
    // -----------------------------------------------------------------------

    #[test]
    fn test_max_agent_depth_value() {
        assert_eq!(MAX_AGENT_DEPTH, 5);
    }

    // -----------------------------------------------------------------------
    // use_worktree flag — isolation field case-insensitivity
    // -----------------------------------------------------------------------

    #[test]
    fn test_use_worktree_lowercase() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "task",
            "isolation": "worktree"
        }))
        .unwrap();
        let use_worktree = input
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);
        assert!(use_worktree);
    }

    #[test]
    fn test_use_worktree_uppercase() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "task",
            "isolation": "WORKTREE"
        }))
        .unwrap();
        let use_worktree = input
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);
        assert!(use_worktree);
    }

    #[test]
    fn test_use_worktree_mixed_case() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "task",
            "isolation": "WorkTree"
        }))
        .unwrap();
        let use_worktree = input
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);
        assert!(use_worktree);
    }

    #[test]
    fn test_use_worktree_none_when_no_isolation() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "task"
        }))
        .unwrap();
        let use_worktree = input
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);
        assert!(!use_worktree);
    }

    #[test]
    fn test_use_worktree_false_for_unknown_mode() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "task",
            "isolation": "sandbox"
        }))
        .unwrap();
        let use_worktree = input
            .isolation
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("worktree"))
            .unwrap_or(false);
        assert!(!use_worktree);
    }

    // -----------------------------------------------------------------------
    // description / subagent_type defaults used in call()
    // -----------------------------------------------------------------------

    #[test]
    fn test_description_default_fallback() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something"
        }))
        .unwrap();
        let description = input.description.as_deref().unwrap_or("unnamed task");
        assert_eq!(description, "unnamed task");
    }

    #[test]
    fn test_description_provided() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something",
            "description": "search codebase"
        }))
        .unwrap();
        let description = input.description.as_deref().unwrap_or("unnamed task");
        assert_eq!(description, "search codebase");
    }

    #[test]
    fn test_subagent_type_default_fallback() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something"
        }))
        .unwrap();
        let subagent_type = input
            .subagent_type
            .as_deref()
            .unwrap_or("general-purpose");
        assert_eq!(subagent_type, "general-purpose");
    }

    #[test]
    fn test_subagent_type_provided() {
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "explore",
            "subagent_type": "Explore"
        }))
        .unwrap();
        let subagent_type = input
            .subagent_type
            .as_deref()
            .unwrap_or("general-purpose");
        assert_eq!(subagent_type, "Explore");
    }

    // -----------------------------------------------------------------------
    // Tool trait flags — is_read_only and is_destructive (defaults)
    // -----------------------------------------------------------------------

    #[test]
    fn test_agent_tool_not_read_only() {
        let tool = AgentTool;
        assert!(!tool.is_read_only(&json!({})));
    }

    #[test]
    fn test_agent_tool_not_destructive() {
        let tool = AgentTool;
        assert!(!tool.is_destructive(&json!({})));
    }

    // -----------------------------------------------------------------------
    // prompt() content — sanity check key phrases
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_prompt_contains_key_guidance() {
        let tool = AgentTool;
        let prompt = tool.prompt().await;
        assert!(prompt.contains("description"));
        assert!(prompt.contains("concurrently"));
        assert!(prompt.contains("autonomously"));
    }

    // -----------------------------------------------------------------------
    // schema — required fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_schema_required_fields() {
        let tool = AgentTool;
        let schema = tool.input_json_schema();
        let required = schema["required"].as_array().unwrap();
        let required_names: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_names.contains(&"prompt"));
        assert!(required_names.contains(&"description"));
    }

    #[test]
    fn test_schema_model_enum() {
        let tool = AgentTool;
        let schema = tool.input_json_schema();
        let model_enum = schema["properties"]["model"]["enum"].as_array().unwrap();
        let variants: Vec<&str> = model_enum
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(variants.contains(&"sonnet"));
        assert!(variants.contains(&"opus"));
        assert!(variants.contains(&"haiku"));
    }

    #[test]
    fn test_schema_isolation_enum() {
        let tool = AgentTool;
        let schema = tool.input_json_schema();
        let isolation_enum = schema["properties"]["isolation"]["enum"]
            .as_array()
            .unwrap();
        let variants: Vec<&str> = isolation_enum
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(variants.contains(&"worktree"));
    }

    #[test]
    fn test_schema_run_in_background_default_false() {
        let tool = AgentTool;
        let schema = tool.input_json_schema();
        let default_val = &schema["properties"]["run_in_background"]["default"];
        assert_eq!(default_val, &serde_json::Value::Bool(false));
    }

    // -----------------------------------------------------------------------
    // user_facing_name — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_facing_name_empty_string_description() {
        let tool = AgentTool;
        // Empty string is still a string — shows Agent()
        let input = json!({"description": ""});
        assert_eq!(tool.user_facing_name(Some(&input)), "Agent()");
    }

    #[test]
    fn test_user_facing_name_non_string_description_falls_back() {
        let tool = AgentTool;
        // description is a number, not a string — falls back to "Agent"
        let input = json!({"description": 42});
        assert_eq!(tool.user_facing_name(Some(&input)), "Agent");
    }
}
