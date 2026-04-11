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
