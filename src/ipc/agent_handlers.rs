//! Command handlers for agent and team IPC commands.
//!
//! Each handler returns a `Vec<BackendMessage>` that the caller sends via the
//! [`FrontendSink`].  Handlers never write to stdout directly.

use tracing::debug;

use crate::ipc::agent_events::*;
use crate::ipc::agent_tree::AGENT_TREE;
use crate::ipc::protocol::BackendMessage;

pub fn handle_agent_command(cmd: AgentCommand) -> Vec<BackendMessage> {
    match cmd {
        AgentCommand::AbortAgent { agent_id } => {
            debug!(agent_id = %agent_id, "IPC: abort agent requested");
            AGENT_TREE
                .lock()
                .update_state(&agent_id, "aborted", None, None, false);

            let aborted = BackendMessage::AgentEvent {
                event: AgentEvent::Aborted {
                    agent_id: agent_id.clone(),
                },
            };
            let roots = AGENT_TREE.lock().build_snapshot();
            let snapshot = BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { roots },
            };
            vec![aborted, snapshot]
        }
        AgentCommand::QueryActiveAgents => {
            let roots = AGENT_TREE.lock().build_snapshot();
            vec![BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { roots },
            }]
        }
        AgentCommand::QueryAgentOutput { agent_id } => {
            debug!(agent_id = %agent_id, "IPC: query agent output");
            vec![BackendMessage::SystemInfo {
                text: format!("Agent output replay not yet implemented for {}", agent_id),
                level: "info".into(),
            }]
        }
    }
}

pub fn handle_team_command(cmd: TeamCommand) -> Vec<BackendMessage> {
    match cmd {
        TeamCommand::InjectMessage {
            team_name,
            to,
            text,
        } => {
            debug!(team = %team_name, to = %to, "IPC: inject team message");
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
                Ok(Ok(())) => vec![],
                Ok(Err(e)) => {
                    vec![BackendMessage::SystemInfo {
                        text: format!("Failed to inject message: {}", e),
                        level: "error".into(),
                    }]
                }
                Err(_) => {
                    vec![BackendMessage::SystemInfo {
                        text: "Agent Teams feature is not available.".into(),
                        level: "warning".into(),
                    }]
                }
            }
        }
        TeamCommand::QueryTeamStatus { team_name } => {
            debug!(team = %team_name, "IPC: query team status");
            vec![BackendMessage::SystemInfo {
                text: format!("Team status query not yet implemented for {}", team_name),
                level: "info".into(),
            }]
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_agent_query_active_returns_tree_snapshot() {
        let msgs = handle_agent_command(AgentCommand::QueryActiveAgents);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            &msgs[0],
            BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { .. }
            }
        ));
    }

    #[test]
    fn handle_agent_abort_returns_two_messages() {
        let msgs = handle_agent_command(AgentCommand::AbortAgent {
            agent_id: "nonexistent".into(),
        });
        assert_eq!(msgs.len(), 2);
        assert!(matches!(
            &msgs[0],
            BackendMessage::AgentEvent {
                event: AgentEvent::Aborted { .. }
            }
        ));
        assert!(matches!(
            &msgs[1],
            BackendMessage::AgentEvent {
                event: AgentEvent::TreeSnapshot { .. }
            }
        ));
    }

    #[test]
    fn handle_team_query_status_returns_info() {
        let msgs = handle_team_command(TeamCommand::QueryTeamStatus {
            team_name: "test".into(),
        });
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SystemInfo { .. }));
    }
}
