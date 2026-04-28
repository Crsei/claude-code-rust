//! Background agent supervisor.
//!
//! Keeps background `Agent` runs under one lifecycle owner instead of letting
//! each tool call detach an untracked task. The supervisor owns registration,
//! cancellation, worktree pre-spawn setup, shutdown cleanup, and task output
//! tracking.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use parking_lot::Mutex;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::engine::lifecycle::QueryEngine;
use crate::tools::tasks::{global_store, TaskCreateOptions, TaskEntry, TaskStatus};
use crate::types::config::{QueryEngineConfig, QuerySource};
use crate::types::tool::*;
use crate::utils::bash::validate_working_directory;

use super::{
    build_child_config, count_worktree_changes, find_git_root, get_head_sha, sdk_to_agent_event,
    AgentInput, AgentTool,
};

const SHUTDOWN_WAIT_PER_AGENT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) struct BackgroundLaunch {
    pub(super) task_id: String,
}

#[derive(Clone)]
struct WorktreeRuntime {
    git_root: PathBuf,
    worktree_path: PathBuf,
    branch_name: String,
    original_head: Option<String>,
}

struct PreparedRuntime {
    child_cwd: String,
    worktree: Option<WorktreeRuntime>,
    startup_warning: Option<String>,
}

struct BackgroundJob {
    agent_id: String,
    task_id: String,
    cancellation_token: CancellationToken,
    handle: Option<tokio::task::JoinHandle<()>>,
    worktree: Option<WorktreeRuntime>,
}

#[derive(Default)]
struct SupervisorState {
    active: HashMap<String, BackgroundJob>,
}

#[derive(Default)]
struct BackgroundSupervisor {
    state: Mutex<SupervisorState>,
}

static BACKGROUND_SUPERVISOR: std::sync::LazyLock<BackgroundSupervisor> =
    std::sync::LazyLock::new(BackgroundSupervisor::default);

#[allow(clippy::too_many_arguments)]
pub(super) async fn spawn_background_agent(
    params: AgentInput,
    ctx: &ToolUseContext,
    agent_id: String,
    description: String,
    subagent_type: String,
    agent_model: String,
    parent_model: String,
    current_depth: usize,
    use_worktree: bool,
    bg_tx: cc_types::agent_channel::AgentSender,
    start_configs: Vec<cc_types::hooks::HookEventConfig>,
    stop_configs: Vec<cc_types::hooks::HookEventConfig>,
) -> Result<BackgroundLaunch> {
    if !start_configs.is_empty() {
        let payload = json!({
            "agent_id": &agent_id,
            "prompt": &params.prompt,
            "description": &description,
            "subagent_type": &subagent_type,
            "model": &agent_model,
            "depth": current_depth + 1,
            "background": true,
        });
        let _ = ctx
            .hook_runner
            .run_event_hooks("SubagentStart", &payload, &start_configs)
            .await;
    }

    let prepared = prepare_runtime(
        use_worktree,
        &agent_id,
        &description,
        &agent_model,
        current_depth,
        ctx.agent_id.as_deref(),
    )
    .await?;
    validate_working_directory(&prepared.child_cwd)?;

    let child_config = build_child_config(
        prepared.child_cwd.clone(),
        ctx,
        &agent_id,
        params.subagent_type.as_deref(),
        &agent_model,
        &parent_model,
        current_depth,
    );

    let task_store = global_store();
    let task_entry = task_store.create_with_options(
        &description,
        &params.prompt,
        TaskCreateOptions {
            kind: Some("local_agent".to_string()),
            parent_id: None,
            depends_on: Vec::new(),
            agent_id: Some(agent_id.clone()),
            supervisor_id: Some(agent_id.clone()),
            isolation: if use_worktree {
                Some("worktree".to_string())
            } else {
                None
            },
            worktree_path: prepared
                .worktree
                .as_ref()
                .map(|wt| wt.worktree_path.display().to_string()),
            worktree_branch: prepared.worktree.as_ref().map(|wt| wt.branch_name.clone()),
        },
    );
    let task_id = task_entry.id.clone();
    task_store.update_status(&task_id, TaskStatus::InProgress);

    let cancellation_token = CancellationToken::new();
    task_store.register_runtime_handle(&task_id, cancellation_token.clone());

    register_agent_tree(
        &agent_id,
        ctx.agent_id.clone(),
        &description,
        params.subagent_type.clone(),
        &agent_model,
        current_depth,
        ctx.query_tracking
            .as_ref()
            .map(|t| t.chain_id.clone())
            .unwrap_or_default(),
        &bg_tx,
    );

    BACKGROUND_SUPERVISOR.register(BackgroundJob {
        agent_id: agent_id.clone(),
        task_id: task_id.clone(),
        cancellation_token: cancellation_token.clone(),
        handle: None,
        worktree: prepared.worktree.clone(),
    });

    let runtime = AgentRuntime {
        child_config,
        prompt: params.prompt,
        agent_id: agent_id.clone(),
        task_id: task_id.clone(),
        description: description.clone(),
        parent_agent_id: ctx.agent_id.clone(),
        agent_model: agent_model.clone(),
        depth: current_depth + 1,
        bg_tx: bg_tx.clone(),
        task_store: task_store.clone(),
        cancellation_token,
        startup_warning: prepared.startup_warning,
        worktree: prepared.worktree,
        stop_configs,
        hook_runner: ctx.hook_runner.clone(),
        command_dispatcher: ctx.command_dispatcher.clone(),
        permission_callback: ctx.permission_callback.clone(),
        ask_user_callback: ctx.ask_user_callback.clone(),
    };

    let handle = tokio::spawn(async move {
        runtime.run().await;
    });
    BACKGROUND_SUPERVISOR.attach_handle(&agent_id, handle);

    Ok(BackgroundLaunch { task_id })
}

pub(crate) fn cancel_agent(agent_id: &str) -> Option<String> {
    let task_id = BACKGROUND_SUPERVISOR.cancel_agent(agent_id);
    if let Some(task_id) = &task_id {
        let _ = global_store().stop(task_id);
    } else if let Some(task) = global_store().get_by_agent_id(agent_id) {
        let _ = global_store().stop(&task.id);
        return Some(task.id);
    }
    task_id
}

pub(crate) fn output_for_agent(agent_id: &str) -> Option<TaskEntry> {
    global_store().get_by_agent_id(agent_id)
}

pub(crate) async fn shutdown_all(reason: &str) -> usize {
    let jobs = BACKGROUND_SUPERVISOR.take_active_jobs();
    let count = jobs.len();

    for mut job in jobs {
        job.cancellation_token.cancel();
        let _ = global_store().append_output(
            &job.task_id,
            &format!("[Supervisor: cancelled during shutdown: {}]", reason),
        );
        let _ = global_store().stop(&job.task_id);

        if let Some(handle) = job.handle.take() {
            let abort_handle = handle.abort_handle();
            match tokio::time::timeout(SHUTDOWN_WAIT_PER_AGENT, handle).await {
                Ok(join_result) => {
                    if let Err(err) = join_result {
                        warn!(
                            agent_id = %job.agent_id,
                            error = %err,
                            "background agent task failed while shutting down"
                        );
                    }
                }
                Err(_) => {
                    warn!(
                        agent_id = %job.agent_id,
                        task_id = %job.task_id,
                        "background agent did not stop before shutdown timeout"
                    );
                    abort_handle.abort();
                    if let Some(worktree) = job.worktree.take() {
                        finalize_or_keep_worktree_after_forced_shutdown(
                            &job.agent_id,
                            &job.task_id,
                            worktree,
                        )
                        .await;
                    }
                }
            }
        }
    }

    count
}

struct AgentRuntime {
    child_config: QueryEngineConfig,
    prompt: String,
    agent_id: String,
    task_id: String,
    description: String,
    parent_agent_id: Option<String>,
    agent_model: String,
    depth: usize,
    bg_tx: cc_types::agent_channel::AgentSender,
    task_store: crate::tools::tasks::TaskStore,
    cancellation_token: CancellationToken,
    startup_warning: Option<String>,
    worktree: Option<WorktreeRuntime>,
    stop_configs: Vec<cc_types::hooks::HookEventConfig>,
    hook_runner: Arc<dyn cc_types::hooks::HookRunner>,
    command_dispatcher: Arc<dyn cc_types::commands::CommandDispatcher>,
    permission_callback: Option<PermissionCallback>,
    ask_user_callback: Option<AskUserCallback>,
}

impl AgentRuntime {
    async fn run(self) {
        let started = std::time::Instant::now();
        info!(
            agent_id = %self.agent_id,
            task_id = %self.task_id,
            description = %self.description,
            "background agent started"
        );

        let mut child_engine = QueryEngine::new(self.child_config);
        child_engine.set_hook_runner(self.hook_runner.clone());
        child_engine.set_command_dispatcher(self.command_dispatcher.clone());
        if let Some(callback) = self.permission_callback.clone() {
            child_engine.set_permission_callback(callback);
        }
        if let Some(callback) = self.ask_user_callback.clone() {
            child_engine.set_ask_user_callback(callback);
        }
        child_engine.set_bg_agent_tx(self.bg_tx.clone());

        let stream =
            child_engine.submit_message(&self.prompt, QuerySource::Agent(self.agent_id.clone()));
        let mut stream = std::pin::pin!(stream);
        let mut result_text = String::new();
        let mut had_error = false;
        let mut was_cancelled = false;

        loop {
            let msg = tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    child_engine.abort();
                    was_cancelled = true;
                    had_error = true;
                    break;
                }
                msg = stream.next() => msg,
            };

            let Some(msg) = msg else {
                break;
            };

            match &msg {
                crate::engine::sdk_types::SdkMessage::Assistant(assistant_msg) => {
                    for block in &assistant_msg.message.content {
                        if let crate::types::message::ContentBlock::Text { text } = block {
                            if !result_text.is_empty() {
                                result_text.push('\n');
                            }
                            result_text.push_str(text);
                        }
                    }
                }
                crate::engine::sdk_types::SdkMessage::Result(sdk_result) => {
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

            if let Some(agent_event) = sdk_to_agent_event(&msg, &self.agent_id) {
                let _ = self
                    .bg_tx
                    .send(cc_types::agent_channel::AgentIpcEvent::Agent(agent_event));
            }
        }

        if result_text.is_empty() {
            result_text = if was_cancelled {
                "(Agent cancelled before producing text output)".to_string()
            } else {
                "(Agent completed with no text output)".to_string()
            };
        }

        if let Some(warning) = &self.startup_warning {
            result_text = format!("[WARNING: {}]\n\n{}", warning, result_text);
        }

        if let Some(worktree) = &self.worktree {
            append_worktree_outcome(
                &mut result_text,
                &self.agent_id,
                &self.task_id,
                worktree,
                self.parent_agent_id.as_deref(),
                &self.description,
                &self.agent_model,
                self.depth,
            )
            .await;
        }

        let duration_ms = started.elapsed().as_millis() as u64;
        let final_status = if was_cancelled {
            TaskStatus::Cancelled
        } else if had_error {
            TaskStatus::Failed
        } else {
            TaskStatus::Completed
        };

        self.task_store.append_output(&self.task_id, &result_text);
        self.task_store.update_status(&self.task_id, final_status);
        self.task_store.unregister_runtime_handle(&self.task_id);
        BACKGROUND_SUPERVISOR.complete(&self.agent_id);

        let _ = crate::dashboard::emit_subagent_event(
            "background_complete",
            &self.agent_id,
            self.parent_agent_id.as_deref(),
            Some(&self.description),
            Some(&self.agent_model),
            self.depth,
            true,
            Some(json!({
                "task_id": self.task_id,
                "duration_ms": duration_ms,
                "result_len": result_text.len(),
                "had_error": had_error,
                "cancelled": was_cancelled,
            })),
        );

        if !self.stop_configs.is_empty() {
            let payload = json!({
                "agent_id": &self.agent_id,
                "description": &self.description,
                "is_error": had_error,
                "background": true,
            });
            let _ = self
                .hook_runner
                .run_event_hooks("SubagentStop", &payload, &self.stop_configs)
                .await;
        }

        let result_preview = preview(&result_text);
        crate::ipc::agent_tree::AGENT_TREE.lock().update_state(
            &self.agent_id,
            if was_cancelled {
                "cancelled"
            } else if had_error {
                "error"
            } else {
                "completed"
            },
            Some(result_preview.clone()),
            Some(duration_ms),
            had_error,
        );

        let _ = self
            .bg_tx
            .send(cc_types::agent_channel::AgentIpcEvent::Agent(
                cc_types::agent_events::AgentEvent::Completed {
                    agent_id: self.agent_id.clone(),
                    result_preview,
                    had_error,
                    duration_ms,
                    output_tokens: None,
                },
            ));

        let roots = crate::ipc::agent_tree::AGENT_TREE.lock().build_snapshot();
        let _ = self
            .bg_tx
            .send(cc_types::agent_channel::AgentIpcEvent::Agent(
                cc_types::agent_events::AgentEvent::TreeSnapshot { roots },
            ));
    }
}

impl BackgroundSupervisor {
    fn register(&self, job: BackgroundJob) {
        self.state.lock().active.insert(job.agent_id.clone(), job);
    }

    fn attach_handle(&self, agent_id: &str, handle: tokio::task::JoinHandle<()>) {
        if let Some(job) = self.state.lock().active.get_mut(agent_id) {
            job.handle = Some(handle);
        }
    }

    fn cancel_agent(&self, agent_id: &str) -> Option<String> {
        let state = self.state.lock();
        let job = state.active.get(agent_id)?;
        job.cancellation_token.cancel();
        Some(job.task_id.clone())
    }

    fn complete(&self, agent_id: &str) {
        self.state.lock().active.remove(agent_id);
    }

    fn take_active_jobs(&self) -> Vec<BackgroundJob> {
        let mut state = self.state.lock();
        state.active.drain().map(|(_, job)| job).collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn register_agent_tree(
    agent_id: &str,
    parent_agent_id: Option<String>,
    description: &str,
    agent_type: Option<String>,
    agent_model: &str,
    current_depth: usize,
    chain_id: String,
    bg_tx: &cc_types::agent_channel::AgentSender,
) {
    let node = cc_types::agent_types::AgentNode {
        agent_id: agent_id.to_string(),
        parent_agent_id: parent_agent_id.clone(),
        description: description.to_string(),
        agent_type: agent_type.clone(),
        model: Some(agent_model.to_string()),
        state: "running".into(),
        is_background: true,
        depth: current_depth + 1,
        chain_id: chain_id.clone(),
        spawned_at: chrono::Utc::now().timestamp(),
        completed_at: None,
        duration_ms: None,
        result_preview: None,
        had_error: false,
        children: vec![],
    };
    crate::ipc::agent_tree::AGENT_TREE.lock().register(node);

    let _ = bg_tx.send(cc_types::agent_channel::AgentIpcEvent::Agent(
        cc_types::agent_events::AgentEvent::Spawned {
            agent_id: agent_id.to_string(),
            parent_agent_id,
            description: description.to_string(),
            agent_type,
            model: Some(agent_model.to_string()),
            is_background: true,
            depth: current_depth + 1,
            chain_id,
        },
    ));

    let roots = crate::ipc::agent_tree::AGENT_TREE.lock().build_snapshot();
    let _ = bg_tx.send(cc_types::agent_channel::AgentIpcEvent::Agent(
        cc_types::agent_events::AgentEvent::TreeSnapshot { roots },
    ));
}

async fn prepare_runtime(
    use_worktree: bool,
    agent_id: &str,
    description: &str,
    agent_model: &str,
    current_depth: usize,
    parent_agent_id: Option<&str>,
) -> Result<PreparedRuntime> {
    if !use_worktree {
        return Ok(PreparedRuntime {
            child_cwd: current_dir_string(),
            worktree: None,
            startup_warning: None,
        });
    }

    match prepare_worktree_runtime(
        agent_id,
        description,
        agent_model,
        current_depth,
        parent_agent_id,
    )
    .await
    {
        Ok(runtime) => Ok(runtime),
        Err(err) => {
            warn!(
                agent_id = %agent_id,
                error = %err,
                "background worktree isolation failed; falling back to normal cwd"
            );
            let warning = format!("worktree isolation skipped: {}", err);
            let _ = crate::dashboard::emit_subagent_event(
                "warning",
                agent_id,
                parent_agent_id,
                Some(description),
                Some(agent_model),
                current_depth + 1,
                true,
                Some(json!({ "message": warning })),
            );
            Ok(PreparedRuntime {
                child_cwd: current_dir_string(),
                worktree: None,
                startup_warning: Some(warning),
            })
        }
    }
}

async fn prepare_worktree_runtime(
    agent_id: &str,
    description: &str,
    agent_model: &str,
    current_depth: usize,
    parent_agent_id: Option<&str>,
) -> Result<PreparedRuntime> {
    let cwd = std::env::current_dir()?;
    let git_root = find_git_root(&cwd).await?;
    let original_head = get_head_sha(&git_root).await;
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let branch_name = format!("agent-worktree-{}", short_id);
    let worktree_path = std::env::temp_dir().join(format!("agent-worktree-{}", short_id));

    info!(
        agent_id = %agent_id,
        worktree_path = %worktree_path.display(),
        branch = %branch_name,
        "creating background agent worktree"
    );

    let output = tokio::process::Command::new("git")
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
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let _ = crate::dashboard::emit_subagent_event(
        "worktree_created",
        agent_id,
        parent_agent_id,
        Some(description),
        Some(agent_model),
        current_depth + 1,
        true,
        Some(json!({
            "worktree_path": worktree_path.display().to_string(),
            "branch": branch_name,
        })),
    );

    Ok(PreparedRuntime {
        child_cwd: worktree_path.to_string_lossy().to_string(),
        worktree: Some(WorktreeRuntime {
            git_root,
            worktree_path,
            branch_name,
            original_head,
        }),
        startup_warning: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn append_worktree_outcome(
    result_text: &mut String,
    agent_id: &str,
    task_id: &str,
    worktree: &WorktreeRuntime,
    parent_agent_id: Option<&str>,
    description: &str,
    agent_model: &str,
    depth: usize,
) {
    let changes =
        count_worktree_changes(&worktree.worktree_path, worktree.original_head.as_deref()).await;
    let has_changes = match changes {
        Some((files, commits)) => files > 0 || commits > 0,
        None => true,
    };

    if has_changes {
        let (files, commits) = changes.unwrap_or((0, 0));
        let _ = crate::dashboard::emit_subagent_event(
            "worktree_kept",
            agent_id,
            parent_agent_id,
            Some(description),
            Some(agent_model),
            depth,
            true,
            Some(json!({
                "task_id": task_id,
                "files": files,
                "commits": commits,
                "worktree_path": worktree.worktree_path.display().to_string(),
                "branch": worktree.branch_name,
            })),
        );
        result_text.push_str(&format!(
            "\n\n[Worktree isolation: changes detected ({} file(s), {} commit(s)). Worktree kept at: {} on branch: {}]",
            files,
            commits,
            worktree.worktree_path.display(),
            worktree.branch_name
        ));
    } else {
        let _ = crate::dashboard::emit_subagent_event(
            "worktree_cleaned",
            agent_id,
            parent_agent_id,
            Some(description),
            Some(agent_model),
            depth,
            true,
            Some(json!({
                "task_id": task_id,
                "worktree_path": worktree.worktree_path.display().to_string(),
                "branch": worktree.branch_name,
            })),
        );
        AgentTool::cleanup_worktree(
            &worktree.git_root,
            &worktree.worktree_path,
            &worktree.branch_name,
            agent_id,
        )
        .await;
        result_text.push_str("\n\n[Worktree isolation: no changes detected; worktree cleaned up]");
    }
}

async fn finalize_or_keep_worktree_after_forced_shutdown(
    agent_id: &str,
    task_id: &str,
    worktree: WorktreeRuntime,
) {
    let changes =
        count_worktree_changes(&worktree.worktree_path, worktree.original_head.as_deref()).await;
    let has_changes = match changes {
        Some((files, commits)) => files > 0 || commits > 0,
        None => true,
    };

    if has_changes {
        let suffix = format!(
            "[Supervisor: shutdown kept worktree at {} on branch {} because changes may exist]",
            worktree.worktree_path.display(),
            worktree.branch_name
        );
        let _ = global_store().append_output(task_id, &suffix);
    } else {
        AgentTool::cleanup_worktree(
            &worktree.git_root,
            &worktree.worktree_path,
            &worktree.branch_name,
            agent_id,
        )
        .await;
        let _ = global_store().append_output(
            task_id,
            "[Supervisor: shutdown cleaned an unchanged worktree]",
        );
    }
}

fn current_dir_string() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

fn preview(result_text: &str) -> String {
    if result_text.len() > 200 {
        let end = result_text.floor_char_boundary(200);
        format!("{}...", &result_text[..end])
    } else {
        result_text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_respects_char_boundary() {
        let text = format!("{}{}", "a".repeat(199), "é".repeat(10));
        let preview = preview(&text);
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 203);
    }

    #[test]
    fn cancel_missing_agent_returns_none() {
        assert!(BACKGROUND_SUPERVISOR
            .cancel_agent("missing-agent")
            .is_none());
    }

    #[test]
    fn register_and_cancel_agent_returns_task_id() {
        let token = CancellationToken::new();
        let agent_id = format!("agent-{}", uuid::Uuid::new_v4());
        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        BACKGROUND_SUPERVISOR.register(BackgroundJob {
            agent_id: agent_id.clone(),
            task_id: task_id.clone(),
            cancellation_token: token.clone(),
            handle: None,
            worktree: None,
        });

        let cancelled_task_id = BACKGROUND_SUPERVISOR.cancel_agent(&agent_id);
        BACKGROUND_SUPERVISOR.complete(&agent_id);

        assert_eq!(cancelled_task_id.as_deref(), Some(task_id.as_str()));
        assert!(token.is_cancelled());
    }
}
