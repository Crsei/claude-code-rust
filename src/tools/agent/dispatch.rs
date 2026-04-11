//! Agent dispatch: normal and worktree execution modes.

use anyhow::Result;
use serde_json::json;
use tracing::debug;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::QuerySource;
use crate::types::tool::*;

use super::{build_child_config, collect_stream_result, AgentInput, AgentTool};

impl AgentTool {
    /// Run the agent without worktree isolation (normal mode).
    pub(super) async fn run_agent_normal(
        &self,
        params: &AgentInput,
        ctx: &ToolUseContext,
        agent_id: &str,
        agent_model: &str,
        parent_model: &str,
        current_depth: usize,
    ) -> Result<ToolResult> {
        let child_cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let child_config = build_child_config(
            child_cwd,
            ctx,
            agent_id,
            agent_model,
            parent_model,
            current_depth,
        );

        let child_engine = QueryEngine::new(child_config);
        let stream =
            child_engine.submit_message(&params.prompt, QuerySource::Agent(agent_id.to_string()));

        let (result_text, had_error) = collect_stream_result(stream).await;

        debug!(
            agent_id = %agent_id,
            result_len = result_text.len(),
            error = had_error,
            "subagent completed"
        );

        Ok(ToolResult {
            data: json!(result_text),
            new_messages: vec![],
        })
    }

    /// Consolidated dispatch: fires SubagentStart hook, runs the agent (worktree
    /// or normal), fires SubagentStop hook.  Used by both the synchronous path
    /// and as a fallback when `bg_agent_tx` is unavailable.
    pub(super) async fn run_agent_dispatch(
        &self,
        use_worktree: bool,
        params: &AgentInput,
        ctx: &ToolUseContext,
        agent_id: &str,
        agent_model: &str,
        parent_model: &str,
        current_depth: usize,
        description: &str,
        start_configs: &[crate::tools::hooks::HookEventConfig],
        stop_configs: &[crate::tools::hooks::HookEventConfig],
    ) -> Result<ToolResult> {
        // Fire SubagentStart hook
        if !start_configs.is_empty() {
            let payload = json!({
                "agent_id": agent_id,
                "prompt": &params.prompt,
                "description": description,
                "subagent_type": params.subagent_type.as_deref().unwrap_or("general-purpose"),
                "model": agent_model,
                "depth": current_depth + 1,
            });
            let _ = crate::tools::hooks::run_event_hooks("SubagentStart", &payload, start_configs).await;
        }

        let result = if use_worktree {
            self.run_in_worktree(params, ctx, agent_id, agent_model, parent_model, current_depth).await
        } else {
            self.run_agent_normal(params, ctx, agent_id, agent_model, parent_model, current_depth).await
        };

        // Fire SubagentStop hook
        if !stop_configs.is_empty() {
            let is_error = result.as_ref().is_err();
            let payload = json!({
                "agent_id": agent_id,
                "description": description,
                "is_error": is_error,
            });
            let _ = crate::tools::hooks::run_event_hooks("SubagentStop", &payload, stop_configs).await;
        }

        result
    }
}
