//! In-process teammate execution loop.
//!
//! Corresponds to TypeScript: `utils/swarm/inProcessRunner.ts`
//!
//! Runs a teammate's QueryEngine inside a `task_local!` scope,
//! processing messages from the mailbox and handling protocol messages.

#![allow(unused)]

use std::time::Duration;

use anyhow::Result;
use tokio::time;
use tracing::{debug, info, warn};

use super::context;
use super::in_process::InProcessBackend;
use super::mailbox;
use super::protocol::{self, ProtocolMessage};
use super::types::*;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::{QueryEngineConfig, QuerySource};

// ---------------------------------------------------------------------------
// Spawn entry point
// ---------------------------------------------------------------------------

/// Configuration for starting an in-process teammate runner.
pub struct InProcessRunnerConfig {
    pub identity: TeammateIdentity,
    pub task_id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub cwd: String,
    pub cancellation: tokio_util::sync::CancellationToken,
}

/// Spawn a teammate runner as a background tokio task.
///
/// Corresponds to TS: `startInProcessTeammate(config)`
///
/// The returned `JoinHandle` can be used for monitoring; the
/// `CancellationToken` controls the runner's lifecycle.
pub fn start_runner(config: InProcessRunnerConfig) -> tokio::task::JoinHandle<()> {
    let agent_id = config.identity.agent_id.clone();

    let handle = tokio::spawn(async move {
        if let Err(e) = run_teammate(config).await {
            warn!(agent_id = %agent_id, error = %e, "teammate runner exited with error");
        }
    });

    handle
}

// ---------------------------------------------------------------------------
// Main execution loop
// ---------------------------------------------------------------------------

/// Run the teammate execution loop within a task_local context.
///
/// Corresponds to TS: `runInProcessTeammate(config)`
///
/// Loop:
/// 1. Submit prompt to QueryEngine
/// 2. Poll mailbox for new messages
/// 3. Handle protocol messages (shutdown, permissions, plan approval)
/// 4. Mark idle when waiting
/// 5. Exit on cancellation or shutdown approval
async fn run_teammate(config: InProcessRunnerConfig) -> Result<()> {
    let identity = config.identity.clone();
    let task_id = config.task_id.clone();
    let agent_name = identity.agent_name.clone();
    let team_name = identity.team_name.clone();
    let cancellation = config.cancellation.clone();

    // Run everything inside the teammate context scope
    context::run_in_scope(identity.clone(), async move {
        info!(
            agent_id = %identity.agent_id,
            task_id = %task_id,
            "teammate runner started"
        );

        // Build a child QueryEngine
        let child_tools = crate::tools::registry::get_all_tools();
        let engine_config = QueryEngineConfig {
            cwd: config.cwd.clone(),
            tools: child_tools,
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: config.model.clone(),
            fallback_model: None,
            max_turns: Some(100),
            max_budget_usd: None,
            task_budget: None,
            verbose: false,
            initial_messages: None,
            commands: vec![],
            thinking_config: None,
            json_schema: None,
            replay_user_messages: false,
            include_partial_messages: false,
            persist_session: false,
        };

        let engine = QueryEngine::new(engine_config);

        // Submit the initial prompt
        {
            use crate::engine::sdk_types::SdkMessage;
            use futures::StreamExt;

            let stream = engine.submit_message(
                &config.prompt,
                QuerySource::Agent(identity.agent_id.clone()),
            );
            let mut stream = std::pin::pin!(stream);

            // Process stream while checking for cancellation and mailbox
            let mut poll_interval = time::interval(Duration::from_millis(
                super::constants::MAILBOX_POLL_INTERVAL_MS,
            ));

            loop {
                tokio::select! {
                    // Check cancellation
                    _ = cancellation.cancelled() => {
                        info!(agent_id = %identity.agent_id, "cancellation received");
                        break;
                    }

                    // Process stream messages
                    msg = stream.next() => {
                        match msg {
                            Some(SdkMessage::Result(_)) => {
                                // Query completed — mark idle
                                InProcessBackend::set_task_idle(&task_id, true);
                                debug!(agent_id = %identity.agent_id, "query completed, marking idle");

                                // Send idle notification to leader
                                let _ = send_idle_notification(
                                    &agent_name,
                                    &team_name,
                                    IdleReason::Available,
                                    None,
                                );
                                break;
                            }
                            Some(_) => {
                                // Still processing — ensure not marked idle
                                InProcessBackend::set_task_idle(&task_id, false);
                            }
                            None => {
                                // Stream ended
                                break;
                            }
                        }
                    }

                    // Periodic mailbox check
                    _ = poll_interval.tick() => {
                        if let Err(e) = process_mailbox(
                            &agent_name,
                            &team_name,
                            &identity.agent_id,
                            &task_id,
                        ) {
                            warn!(error = %e, "mailbox processing error");
                        }
                    }
                }
            }
        }

        // Cleanup
        InProcessBackend::update_task_status(&task_id, TaskStatus::Completed);
        info!(agent_id = %identity.agent_id, "teammate runner finished");
    })
    .await;

    Ok(())
}

// ---------------------------------------------------------------------------
// Mailbox processing
// ---------------------------------------------------------------------------

/// Process unread mailbox messages for this teammate.
fn process_mailbox(
    agent_name: &str,
    team_name: &str,
    agent_id: &str,
    task_id: &str,
) -> Result<()> {
    let messages = mailbox::read_unread_messages(agent_name, team_name)?;

    for (idx, msg) in messages.iter().enumerate() {
        // Try to parse as protocol message
        if let Some(proto) = protocol::try_parse_protocol_message(&msg.text) {
            handle_protocol_message(proto, agent_name, team_name, agent_id, task_id)?;
        } else {
            // Plain text message — log for now (would inject into conversation)
            debug!(
                from = %msg.from,
                "received plain text message from teammate"
            );
        }
    }

    // Mark all as read
    if !messages.is_empty() {
        mailbox::mark_all_as_read(agent_name, team_name)?;
    }

    Ok(())
}

/// Handle a structured protocol message.
fn handle_protocol_message(
    msg: ProtocolMessage,
    agent_name: &str,
    team_name: &str,
    agent_id: &str,
    task_id: &str,
) -> Result<()> {
    match msg {
        ProtocolMessage::ShutdownRequest {
            request_id, reason, ..
        } => {
            info!(
                agent_id,
                request_id = %request_id,
                reason = ?reason,
                "received shutdown request"
            );

            // Auto-approve shutdown for simplicity
            // (A full implementation would let the model decide)
            let now = chrono::Utc::now();
            let approval = serde_json::json!({
                "type": "shutdown_approved",
                "requestId": request_id,
                "from": agent_name,
                "timestamp": now.to_rfc3339(),
                "backendType": "in-process",
            });

            let response = TeammateMessage {
                from: agent_name.into(),
                text: approval.to_string(),
                timestamp: now.to_rfc3339(),
                read: false,
                color: None,
                summary: Some("Shutdown approved".into()),
            };

            // Write approval to leader's mailbox
            mailbox::write_to_mailbox(
                super::constants::TEAM_LEAD_NAME,
                response,
                team_name,
            )?;

            // Mark task as stopped
            InProcessBackend::update_task_status(task_id, TaskStatus::Stopped);
        }

        ProtocolMessage::PlanApprovalResponse {
            approved, feedback, ..
        } => {
            debug!(approved, feedback = ?feedback, "plan approval response received");
            // Would unblock the plan mode gate
        }

        ProtocolMessage::PermissionResponse {
            request_id,
            subtype,
            ..
        } => {
            debug!(request_id = %request_id, subtype = %subtype, "permission response received");
            // Would unblock the permission request
        }

        ProtocolMessage::ModeSetRequest { mode, .. } => {
            debug!(mode = %mode, "mode set request received");
            // Would update the permission mode
        }

        other => {
            debug!(?other, "unhandled protocol message");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Notification helpers
// ---------------------------------------------------------------------------

/// Send an idle notification to the team leader.
fn send_idle_notification(
    agent_name: &str,
    team_name: &str,
    reason: IdleReason,
    summary: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now();
    let notification = serde_json::json!({
        "type": "idle_notification",
        "from": agent_name,
        "timestamp": now.to_rfc3339(),
        "idleReason": reason,
        "summary": summary,
    });

    let message = TeammateMessage {
        from: agent_name.into(),
        text: notification.to_string(),
        timestamp: now.to_rfc3339(),
        read: false,
        color: None,
        summary: summary.map(|s| s.to_string()),
    };

    mailbox::write_to_mailbox(
        super::constants::TEAM_LEAD_NAME,
        message,
        team_name,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_creation() {
        let config = InProcessRunnerConfig {
            identity: TeammateIdentity {
                agent_id: "worker@team".into(),
                agent_name: "worker".into(),
                team_name: "team".into(),
                color: Some("red".into()),
                plan_mode_required: false,
                parent_session_id: "sess-1".into(),
            },
            task_id: "task-1".into(),
            prompt: "Do work".into(),
            model: None,
            cwd: "/tmp".into(),
            cancellation: tokio_util::sync::CancellationToken::new(),
        };
        assert_eq!(config.identity.agent_id, "worker@team");
        assert_eq!(config.prompt, "Do work");
    }

    #[test]
    fn test_idle_reason_serialize() {
        let json = serde_json::to_string(&IdleReason::Available).unwrap();
        assert_eq!(json, "\"available\"");
    }
}
