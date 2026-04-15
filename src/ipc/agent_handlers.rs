//! Command handlers for agent and team IPC commands.

#![allow(dead_code)]

use tracing::debug;

use crate::ipc::agent_events::*;
use crate::ipc::agent_tree::AGENT_TREE;
use crate::ipc::protocol::{send_to_frontend, BackendMessage};

pub async fn handle_agent_command(cmd: AgentCommand) {
    match cmd {
        AgentCommand::AbortAgent { agent_id } => {
            debug!(agent_id = %agent_id, "IPC: abort agent requested");
            AGENT_TREE
                .lock()
                .update_state(&agent_id, "aborted", None, None, false);
            let _ = send_to_frontend(&BackendMessage::AgentEvent {
                event: AgentEvent::Aborted {
                    agent_id: agent_id.clone(),
                },
            });
            let roots = AGENT_TREE.lock().build_snapshot();
            let _ = send_to_frontend(&BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { roots },
            });
        }
        AgentCommand::QueryActiveAgents => {
            let roots = AGENT_TREE.lock().build_snapshot();
            let _ = send_to_frontend(&BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { roots },
            });
        }
        AgentCommand::QueryAgentOutput { agent_id } => {
            debug!(agent_id = %agent_id, "IPC: query agent output");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!("Agent output replay not yet implemented for {}", agent_id),
                level: "info".into(),
            });
        }
    }
}

pub async fn handle_team_command(cmd: TeamCommand) {
    match cmd {
        TeamCommand::InjectMessage {
            team_name,
            to,
            text,
        } => {
            debug!(team = %team_name, to = %to, "IPC: inject team message");
            // Team mailbox — attempt delivery via write_to_mailbox
            let result = std::panic::catch_unwind(|| {
                let msg = crate::teams::types::TeammateMessage {
                    from: "__frontend__".to_string(),
                    text: text.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    read: false,
                    color: None,
                    summary: None,
                };
                crate::teams::mailbox::write_to_mailbox(&to, msg, &team_name)
            });
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let _ = send_to_frontend(&BackendMessage::SystemInfo {
                        text: format!("Failed to inject message: {}", e),
                        level: "error".into(),
                    });
                }
                Err(_) => {
                    let _ = send_to_frontend(&BackendMessage::SystemInfo {
                        text: "Agent Teams feature is not available.".into(),
                        level: "warning".into(),
                    });
                }
            }
        }
        TeamCommand::QueryTeamStatus { team_name } => {
            debug!(team = %team_name, "IPC: query team status");
            let _ = send_to_frontend(&BackendMessage::SystemInfo {
                text: format!("Team status query not yet implemented for {}", team_name),
                level: "info".into(),
            });
        }
    }
}
