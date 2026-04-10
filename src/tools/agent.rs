//! Agent tool — spawns a sub-QueryEngine to handle complex tasks.
//!
//! Corresponds to TypeScript: tools/AgentTool/
//!
//! The Agent tool creates a child QueryEngine with its own conversation context,
//! runs the provided prompt through it, and returns the result to the parent.
//! This enables delegation of complex, multi-step tasks to specialized subagents.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::{AgentContext, QueryEngineConfig, QuerySource};
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
fn resolve_model_alias(alias: &str, _fallback: &str) -> String {
    match alias {
        "sonnet" => "claude-sonnet-4-20250514".to_string(),
        "opus" => "claude-opus-4-20250514".to_string(),
        "haiku" => "claude-haiku-4-5-20251001".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Worktree isolation helpers
// ---------------------------------------------------------------------------

/// Find the git root directory from a working directory.
async fn find_git_root(cwd: &Path) -> Result<PathBuf> {
    let output = tokio::process::Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "--show-toplevel"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Not a git repository (or git not found): {}", stderr.trim());
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Get the HEAD commit SHA at a given path.
async fn get_head_sha(cwd: &Path) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Count uncommitted changes and new commits in a worktree relative to a
/// baseline commit.  Returns `(changed_files, new_commits)`.
async fn count_worktree_changes(
    worktree_path: &Path,
    original_head: Option<&str>,
) -> Option<(usize, usize)> {
    let status = tokio::process::Command::new("git")
        .args([
            "-C",
            &worktree_path.to_string_lossy(),
            "status",
            "--porcelain",
        ])
        .output()
        .await
        .ok()?;

    if !status.status.success() {
        return None;
    }

    let changed_files = String::from_utf8_lossy(&status.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();

    let Some(orig_head) = original_head else {
        return Some((changed_files, 0));
    };

    let rev_list = tokio::process::Command::new("git")
        .args([
            "-C",
            &worktree_path.to_string_lossy(),
            "rev-list",
            "--count",
            &format!("{}..HEAD", orig_head),
        ])
        .output()
        .await
        .ok()?;

    if !rev_list.status.success() {
        return None;
    }

    let commits = String::from_utf8_lossy(&rev_list.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0);

    Some((changed_files, commits))
}

// ---------------------------------------------------------------------------
// Helper: build a child QueryEngineConfig
// ---------------------------------------------------------------------------

fn build_child_config(
    cwd: String,
    ctx: &ToolUseContext,
    agent_id: &str,
    agent_model: &str,
    parent_model: &str,
    current_depth: usize,
) -> QueryEngineConfig {
    let child_tools = crate::tools::registry::get_all_tools();
    let chain_id = ctx
        .query_tracking
        .as_ref()
        .map(|t| t.chain_id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    QueryEngineConfig {
        cwd,
        tools: child_tools,
        custom_system_prompt: ctx.options.custom_system_prompt.clone(),
        append_system_prompt: ctx.options.append_system_prompt.clone(),
        user_specified_model: Some(agent_model.to_string()),
        fallback_model: Some(parent_model.to_string()),
        max_turns: Some(30),
        max_budget_usd: ctx.options.max_budget_usd,
        task_budget: None,
        verbose: ctx.options.verbose,
        initial_messages: None,
        commands: vec![],
        thinking_config: None,
        json_schema: None,
        replay_user_messages: false,
        persist_session: false,
        resolved_model: None,
        auto_save_session: false,
        agent_context: Some(AgentContext {
            agent_id: agent_id.to_string(),
            query_tracking: QueryChainTracking {
                chain_id,
                depth: current_depth + 1,
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// Helper: consume a child engine stream and collect text result
// ---------------------------------------------------------------------------

async fn collect_stream_result(
    stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = crate::engine::sdk_types::SdkMessage> + Send>,
    >,
) -> (String, bool) {
    use crate::engine::sdk_types::SdkMessage;
    use futures::StreamExt;

    let mut stream = stream;
    let mut result_text = String::new();
    let mut had_error = false;

    while let Some(msg) = stream.next().await {
        match msg {
            SdkMessage::Assistant(ref assistant_msg) => {
                for block in &assistant_msg.message.content {
                    if let crate::types::message::ContentBlock::Text { text } = block {
                        if !result_text.is_empty() {
                            result_text.push('\n');
                        }
                        result_text.push_str(text);
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

    (result_text, had_error)
}

// ---------------------------------------------------------------------------
// AgentTool impl methods
// ---------------------------------------------------------------------------

impl AgentTool {
    /// Run the agent inside an isolated git worktree.
    ///
    /// Creates a temporary worktree + branch, points the child QueryEngine's
    /// cwd at it, runs the agent, and then cleans up if no changes were made.
    async fn run_in_worktree(
        &self,
        params: &AgentInput,
        ctx: &ToolUseContext,
        agent_id: &str,
        agent_model: &str,
        parent_model: &str,
        current_depth: usize,
    ) -> Result<ToolResult> {
        let cwd = std::env::current_dir()?;

        // ── 1. Find git root ─────────────────────────────────────────────
        let git_root = match find_git_root(&cwd).await {
            Ok(root) => root,
            Err(e) => {
                warn!(
                    agent_id = %agent_id,
                    error = %e,
                    "worktree isolation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — {}]\n\n{}",
                                e, s
                            ));
                        }
                        r
                    });
            }
        };

        let original_head = get_head_sha(&git_root).await;

        // ── 2. Create branch + worktree ──────────────────────────────────
        let short_id = &Uuid::new_v4().to_string()[..8];
        let branch_name = format!("agent-worktree-{}", short_id);
        let worktree_path = std::env::temp_dir().join(format!("agent-worktree-{}", short_id));

        info!(
            agent_id = %agent_id,
            worktree_path = %worktree_path.display(),
            branch = %branch_name,
            "creating agent worktree"
        );

        let wt_output = tokio::process::Command::new("git")
            .args([
                "-C",
                &git_root.to_string_lossy(),
                "worktree",
                "add",
                "-B",
                &branch_name,
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .await;

        match wt_output {
            Ok(ref o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                warn!(
                    agent_id = %agent_id,
                    error = %stderr,
                    "worktree creation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — git worktree add failed: {}]\n\n{}",
                                stderr.trim(), s
                            ));
                        }
                        r
                    });
            }
            Err(e) => {
                warn!(
                    agent_id = %agent_id,
                    error = %e,
                    "worktree creation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — {}]\n\n{}",
                                e, s
                            ));
                        }
                        r
                    });
            }
            Ok(_) => {
                debug!(
                    agent_id = %agent_id,
                    worktree_path = %worktree_path.display(),
                    "worktree created successfully"
                );
            }
        }

        // ── 3. Run the agent with cwd = worktree ─────────────────────────
        let child_config = build_child_config(
            worktree_path.to_string_lossy().to_string(),
            ctx,
            agent_id,
            agent_model,
            parent_model,
            current_depth,
        );

        let child_engine = QueryEngine::new(child_config);
        let stream =
            child_engine.submit_message(&params.prompt, QuerySource::Agent(agent_id.to_string()));

        let (mut result_text, had_error) = collect_stream_result(stream).await;

        // ── 4. Check for changes ─────────────────────────────────────────
        let changes = count_worktree_changes(&worktree_path, original_head.as_deref()).await;

        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true, // fail-closed: assume changes if we can't tell
        };

        if has_changes {
            // Keep the worktree — include location info in result
            let (files, commits) = changes.unwrap_or((0, 0));
            info!(
                agent_id = %agent_id,
                worktree_path = %worktree_path.display(),
                branch = %branch_name,
                changed_files = files,
                new_commits = commits,
                "agent worktree has changes — keeping"
            );

            result_text.push_str(&format!(
                "\n\n[Worktree isolation: changes detected ({} file(s), {} commit(s)). \
                 Worktree kept at: {} on branch: {}]",
                files,
                commits,
                worktree_path.display(),
                branch_name,
            ));
        } else {
            // No changes — clean up worktree + branch
            info!(
                agent_id = %agent_id,
                worktree_path = %worktree_path.display(),
                branch = %branch_name,
                "agent worktree has no changes — cleaning up"
            );

            let remove_result = tokio::process::Command::new("git")
                .args([
                    "-C",
                    &git_root.to_string_lossy(),
                    "worktree",
                    "remove",
                    "--force",
                    &worktree_path.to_string_lossy(),
                ])
                .output()
                .await;

            match remove_result {
                Ok(o) if o.status.success() => {
                    debug!(agent_id = %agent_id, "agent worktree directory removed");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!(
                        agent_id = %agent_id,
                        "worktree remove warning: {}", stderr
                    );
                }
                Err(e) => {
                    warn!(agent_id = %agent_id, "worktree remove failed: {}", e);
                }
            }

            // Brief pause to let git release locks before branch delete
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let branch_result = tokio::process::Command::new("git")
                .args([
                    "-C",
                    &git_root.to_string_lossy(),
                    "branch",
                    "-D",
                    &branch_name,
                ])
                .output()
                .await;

            match branch_result {
                Ok(o) if o.status.success() => {
                    debug!(agent_id = %agent_id, "agent worktree branch deleted");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!(
                        agent_id = %agent_id,
                        "branch delete warning: {}", stderr
                    );
                }
                Err(e) => {
                    warn!(agent_id = %agent_id, "branch delete failed: {}", e);
                }
            }

            result_text
                .push_str("\n\n[Worktree isolation: no changes detected — worktree cleaned up]");
        }

        debug!(
            agent_id = %agent_id,
            result_len = result_text.len(),
            error = had_error,
            "subagent (worktree) completed"
        );

        Ok(ToolResult {
            data: json!(result_text),
            new_messages: vec![],
        })
    }

    /// Run the agent without worktree isolation (normal mode).
    async fn run_agent_normal(
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
}

// ---------------------------------------------------------------------------
// Tool trait implementation
// ---------------------------------------------------------------------------

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

        // Background mode is stubbed — run synchronously with a warning
        if params.run_in_background {
            warn!(
                agent_id = %agent_id,
                "run_in_background requested but not yet implemented — running synchronously"
            );
        }

        info!(
            agent_id = %agent_id,
            description = %description,
            subagent_type = %subagent_type,
            model = %agent_model,
            depth = current_depth + 1,
            isolation = ?params.isolation,
            "spawning subagent"
        );

        // Fire SubagentStart hook
        {
            let app_state = (ctx.get_app_state)();
            let start_configs = crate::tools::hooks::load_hook_configs(&app_state.hooks, "SubagentStart");
            if !start_configs.is_empty() {
                let payload = json!({
                    "agent_id": &agent_id,
                    "prompt": &params.prompt,
                    "description": description,
                    "subagent_type": subagent_type,
                    "model": &agent_model,
                    "depth": current_depth + 1,
                });
                let _ = crate::tools::hooks::run_event_hooks("SubagentStart", &payload, &start_configs).await;
            }
        }

        // Dispatch based on isolation mode
        let result = if use_worktree {
            self.run_in_worktree(
                &params,
                ctx,
                &agent_id,
                &agent_model,
                &parent_model,
                current_depth,
            )
            .await
        } else {
            self.run_agent_normal(
                &params,
                ctx,
                &agent_id,
                &agent_model,
                &parent_model,
                current_depth,
            )
            .await
        };

        // Fire SubagentStop hook
        {
            let app_state = (ctx.get_app_state)();
            let stop_configs = crate::tools::hooks::load_hook_configs(&app_state.hooks, "SubagentStop");
            if !stop_configs.is_empty() {
                let is_error = result.as_ref().is_err();
                let payload = json!({
                    "agent_id": &agent_id,
                    "description": description,
                    "is_error": is_error,
                });
                let _ = crate::tools::hooks::run_event_hooks("SubagentStop", &payload, &stop_configs).await;
            }
        }

        result
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
        assert_eq!(
            resolve_model_alias("custom-model", "fallback"),
            "custom-model"
        );
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

    #[test]
    fn test_agent_isolation_field() {
        // No isolation field — should be None
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something",
            "description": "test task"
        }))
        .unwrap();
        assert!(input.isolation.is_none());

        // Explicit worktree isolation
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something",
            "description": "test task",
            "isolation": "worktree"
        }))
        .unwrap();
        assert_eq!(input.isolation.as_deref(), Some("worktree"));

        // Null isolation — should be None
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "do something",
            "description": "test task",
            "isolation": null
        }))
        .unwrap();
        assert!(input.isolation.is_none());
    }

    #[test]
    fn test_agent_input_deserialization() {
        // Minimal required fields
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "find all TODO comments"
        }))
        .unwrap();
        assert_eq!(input.prompt, "find all TODO comments");
        assert!(input.description.is_none());
        assert!(input.model.is_none());
        assert!(!input.run_in_background);
        assert!(input.isolation.is_none());

        // Full fields
        let input: AgentInput = serde_json::from_value(json!({
            "prompt": "search for bugs",
            "description": "bug search",
            "subagent_type": "Explore",
            "model": "haiku",
            "run_in_background": true,
            "isolation": "worktree"
        }))
        .unwrap();
        assert_eq!(input.prompt, "search for bugs");
        assert_eq!(input.description.as_deref(), Some("bug search"));
        assert_eq!(input.subagent_type.as_deref(), Some("Explore"));
        assert_eq!(input.model.as_deref(), Some("haiku"));
        assert!(input.run_in_background);
        assert_eq!(input.isolation.as_deref(), Some("worktree"));
    }

    #[test]
    fn test_max_result_size() {
        let tool = AgentTool;
        assert_eq!(tool.max_result_size_chars(), 200_000);
    }
}
