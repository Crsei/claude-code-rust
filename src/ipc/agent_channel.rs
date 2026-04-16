//! Agent IPC channel — the dedicated mpsc channel for agent + team events.

#![allow(dead_code)] // Types are pre-defined for upcoming agent IPC extension tasks

use super::agent_events::{AgentEvent, TeamEvent};

/// All events that flow through the agent channel.
#[derive(Debug)]
pub enum AgentIpcEvent {
    Agent(AgentEvent),
    Team(TeamEvent),
}

/// Sender half — injected into agent tool and team modules.
pub type AgentSender = tokio::sync::mpsc::UnboundedSender<AgentIpcEvent>;

/// Receiver half — consumed by the headless event loop.
pub type AgentReceiver = tokio::sync::mpsc::UnboundedReceiver<AgentIpcEvent>;

/// Create a new agent channel pair.
pub fn agent_channel() -> (AgentSender, AgentReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::agent_events::AgentEvent;

    #[test]
    fn agent_channel_send_receive() {
        let (tx, mut rx) = agent_channel();
        tx.send(AgentIpcEvent::Agent(AgentEvent::Aborted {
            agent_id: "a1".into(),
        }))
        .unwrap();
        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            AgentIpcEvent::Agent(AgentEvent::Aborted { .. })
        ));
    }

    #[test]
    fn agent_channel_team_event() {
        let (tx, mut rx) = agent_channel();
        tx.send(AgentIpcEvent::Team(
            crate::ipc::agent_events::TeamEvent::MemberLeft {
                team_name: "t1".into(),
                agent_id: "a1".into(),
                agent_name: "worker".into(),
            },
        ))
        .unwrap();
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, AgentIpcEvent::Team(_)));
    }
}
