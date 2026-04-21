//! Agent tool — spawns a sub-QueryEngine to handle complex tasks.
//!
//! Corresponds to TypeScript: tools/AgentTool/
//!
//! The Agent tool creates a child QueryEngine with its own conversation context,
//! runs the provided prompt through it, and returns the result to the parent.
//! This enables delegation of complex, multi-step tasks to specialized subagents.

mod dispatch;
pub mod fork;
mod tool_impl;
mod worktree;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::types::config::{AgentContext, QueryEngineConfig};
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
// Helper: convert SdkMessage → AgentEvent for IPC forwarding
// ---------------------------------------------------------------------------

/// Convert an SdkMessage to an AgentEvent for IPC forwarding.
/// Returns None for messages that don't map to agent events.
pub(crate) fn sdk_to_agent_event(
    sdk_msg: &crate::engine::sdk_types::SdkMessage,
    agent_id: &str,
) -> Option<cc_types::agent_events::AgentEvent> {
    use crate::engine::sdk_types::SdkMessage;
    use cc_types::agent_events::AgentEvent;
    use crate::types::message::{ContentBlock, StreamEvent, ToolResultContent};

    match sdk_msg {
        SdkMessage::StreamEvent(evt) => match &evt.event {
            StreamEvent::ContentBlockDelta { delta, .. } => {
                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                    Some(AgentEvent::StreamDelta {
                        agent_id: agent_id.to_string(),
                        text: text.to_string(),
                    })
                } else if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                    Some(AgentEvent::ThinkingDelta {
                        agent_id: agent_id.to_string(),
                        thinking: thinking.to_string(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        },
        SdkMessage::Assistant(a) => {
            for block in &a.message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    return Some(AgentEvent::ToolUse {
                        agent_id: agent_id.to_string(),
                        tool_use_id: id.clone(),
                        tool_name: name.clone(),
                        input: input.clone(),
                    });
                }
            }
            None
        }
        SdkMessage::UserReplay(replay) => {
            if let Some(blocks) = &replay.content_blocks {
                for block in blocks {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = block
                    {
                        let output = match content {
                            ToolResultContent::Text(t) => t.clone(),
                            ToolResultContent::Blocks(_) => "[complex output]".to_string(),
                        };
                        return Some(AgentEvent::ToolResult {
                            agent_id: agent_id.to_string(),
                            tool_use_id: tool_use_id.clone(),
                            output,
                            is_error: *is_error,
                        });
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper: build a child QueryEngineConfig
// ---------------------------------------------------------------------------

fn build_child_config(
    cwd: String,
    ctx: &ToolUseContext,
    agent_id: &str,
    child_agent_type: Option<&str>,
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
            langfuse_session_id: ctx.langfuse_session_id.clone(),
            agent_type: child_agent_type.map(|value| value.to_string()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Helper: consume a child engine stream and collect text result
// ---------------------------------------------------------------------------

/// Consume a child engine stream, collecting text output.
///
/// When `ipc` is provided (sender + agent_id), intermediate streaming events
/// are forwarded through the agent IPC channel via [`sdk_to_agent_event`].
async fn collect_stream_result(
    stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = crate::engine::sdk_types::SdkMessage> + Send>,
    >,
    ipc: Option<(&cc_types::agent_channel::AgentSender, &str)>,
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

        // Forward intermediate events to IPC when a sender is available
        if let Some((tx, agent_id)) = ipc {
            if let Some(agent_event) = sdk_to_agent_event(&msg, agent_id) {
                let _ = tx.send(cc_types::agent_channel::AgentIpcEvent::Agent(agent_event));
            }
        }
    }

    if result_text.is_empty() {
        result_text = "(Agent completed with no text output)".to_string();
    }

    (result_text, had_error)
}
