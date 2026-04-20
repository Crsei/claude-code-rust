//! Agent and team event/command enums for IPC.
//!
//! **Event enums** (Backend -> Frontend): `Serialize + Debug + Clone`, tagged by `"kind"`.
//! **Command enums** (Frontend -> Backend): `Deserialize + Debug`, tagged by `"kind"`.
//!
//! These types are consumed by:
//! - `protocol.rs` — wrapped inside `BackendMessage` / `FrontendMessage` variants
//! - `headless.rs` — event dispatch loop
//! - Agent orchestration and team coordination modules

#![allow(dead_code)] // Types are pre-defined for upcoming agent IPC extension tasks

use serde::{Deserialize, Serialize};

use super::agent_types::*;

// ===========================================================================
// AgentEvent (Backend → Frontend)
// ===========================================================================

/// Events emitted by the agent subsystem to notify the frontend about
/// agent lifecycle, streaming output, tool usage, and tree snapshots.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A new agent was spawned.
    Spawned {
        agent_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_agent_id: Option<String>,
        description: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        is_background: bool,
        depth: usize,
        chain_id: String,
    },
    /// An agent completed successfully.
    Completed {
        agent_id: String,
        result_preview: String,
        had_error: bool,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
    },
    /// An agent encountered a fatal error.
    Error {
        agent_id: String,
        error: String,
        duration_ms: u64,
    },
    /// An agent was aborted by the user or system.
    Aborted { agent_id: String },
    /// A streaming text delta from an agent's response.
    StreamDelta { agent_id: String, text: String },
    /// A streaming thinking/reasoning delta from an agent.
    ThinkingDelta { agent_id: String, thinking: String },
    /// An agent initiated a tool use.
    ToolUse {
        agent_id: String,
        tool_use_id: String,
        tool_name: String,
        input: serde_json::Value,
    },
    /// A tool returned its result to an agent.
    ToolResult {
        agent_id: String,
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    /// Full snapshot of the agent tree hierarchy.
    TreeSnapshot { roots: Vec<AgentNode> },
}

// ===========================================================================
// AgentCommand (Frontend → Backend)
// ===========================================================================

/// Commands the frontend can send to control or query agents.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentCommand {
    /// Request to abort a specific agent.
    AbortAgent { agent_id: String },
    /// Request the list of all currently active agents.
    QueryActiveAgents,
    /// Request the full output of a specific agent.
    QueryAgentOutput { agent_id: String },
}

// ===========================================================================
// TeamEvent (Backend → Frontend)
// ===========================================================================

/// Events emitted by the team coordination subsystem.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TeamEvent {
    /// A new member joined a team.
    MemberJoined {
        team_name: String,
        agent_id: String,
        agent_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    /// A member left a team.
    MemberLeft {
        team_name: String,
        agent_id: String,
        agent_name: String,
    },
    /// A message was routed between team members.
    MessageRouted {
        team_name: String,
        from: String,
        to: String,
        text: String,
        timestamp: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Full status snapshot of a team.
    StatusSnapshot {
        team_name: String,
        members: Vec<TeamMemberInfo>,
        pending_messages: usize,
    },
}

// ===========================================================================
// TeamCommand (Frontend → Backend)
// ===========================================================================

/// Commands the frontend can send to interact with team coordination.
#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TeamCommand {
    /// Inject a message to a specific team member.
    InjectMessage {
        team_name: String,
        to: String,
        text: String,
    },
    /// Query the current status of a team.
    QueryTeamStatus { team_name: String },
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // AgentEvent serialization
    // -----------------------------------------------------------------------

    #[test]
    fn agent_event_spawned_serializes_with_kind() {
        let event = AgentEvent::Spawned {
            agent_id: "agent-1".to_string(),
            parent_agent_id: None,
            description: "Implement feature".to_string(),
            agent_type: Some("coordinator".to_string()),
            model: Some("claude-sonnet-4-20250514".to_string()),
            is_background: false,
            depth: 0,
            chain_id: "chain-abc".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize AgentEvent::Spawned");
        assert_eq!(value["kind"], "spawned");
        assert_eq!(value["agent_id"], "agent-1");
        assert_eq!(value["description"], "Implement feature");
        assert_eq!(value["agent_type"], "coordinator");
        assert_eq!(value["model"], "claude-sonnet-4-20250514");
        assert_eq!(value["is_background"], false);
        assert_eq!(value["depth"], 0);
        assert_eq!(value["chain_id"], "chain-abc");
        assert!(
            value.get("parent_agent_id").is_none(),
            "None parent_agent_id should be omitted"
        );
    }

    #[test]
    fn agent_event_spawned_with_parent_serializes() {
        let event = AgentEvent::Spawned {
            agent_id: "agent-2".to_string(),
            parent_agent_id: Some("agent-1".to_string()),
            description: "Run tests".to_string(),
            agent_type: None,
            model: None,
            is_background: true,
            depth: 1,
            chain_id: "chain-abc".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "spawned");
        assert_eq!(value["parent_agent_id"], "agent-1");
        assert_eq!(value["is_background"], true);
        assert!(value.get("agent_type").is_none());
        assert!(value.get("model").is_none());
    }

    #[test]
    fn agent_event_completed_serializes_with_kind() {
        let event = AgentEvent::Completed {
            agent_id: "agent-1".to_string(),
            result_preview: "All tests passed".to_string(),
            had_error: false,
            duration_ms: 5000,
            output_tokens: Some(1234),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "completed");
        assert_eq!(value["agent_id"], "agent-1");
        assert_eq!(value["result_preview"], "All tests passed");
        assert_eq!(value["had_error"], false);
        assert_eq!(value["duration_ms"], 5000);
        assert_eq!(value["output_tokens"], 1234);
    }

    #[test]
    fn agent_event_completed_without_output_tokens() {
        let event = AgentEvent::Completed {
            agent_id: "agent-1".to_string(),
            result_preview: "Done".to_string(),
            had_error: true,
            duration_ms: 100,
            output_tokens: None,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "completed");
        assert_eq!(value["had_error"], true);
        assert!(value.get("output_tokens").is_none());
    }

    #[test]
    fn agent_event_error_serializes_with_kind() {
        let event = AgentEvent::Error {
            agent_id: "agent-3".to_string(),
            error: "API timeout".to_string(),
            duration_ms: 30000,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "error");
        assert_eq!(value["agent_id"], "agent-3");
        assert_eq!(value["error"], "API timeout");
        assert_eq!(value["duration_ms"], 30000);
    }

    #[test]
    fn agent_event_aborted_serializes_with_kind() {
        let event = AgentEvent::Aborted {
            agent_id: "agent-4".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "aborted");
        assert_eq!(value["agent_id"], "agent-4");
    }

    #[test]
    fn agent_event_stream_delta_serializes_with_kind() {
        let event = AgentEvent::StreamDelta {
            agent_id: "agent-1".to_string(),
            text: "Hello, world!".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize AgentEvent::StreamDelta");
        assert_eq!(value["kind"], "stream_delta");
        assert_eq!(value["agent_id"], "agent-1");
        assert_eq!(value["text"], "Hello, world!");
    }

    #[test]
    fn agent_event_thinking_delta_serializes() {
        let event = AgentEvent::ThinkingDelta {
            agent_id: "agent-1".to_string(),
            thinking: "Let me consider...".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "thinking_delta");
        assert_eq!(value["thinking"], "Let me consider...");
    }

    #[test]
    fn agent_event_tool_use_serializes() {
        let event = AgentEvent::ToolUse {
            agent_id: "agent-1".to_string(),
            tool_use_id: "tu-001".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({"command": "ls -la"}),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "tool_use");
        assert_eq!(value["tool_name"], "Bash");
        assert_eq!(value["input"]["command"], "ls -la");
    }

    #[test]
    fn agent_event_tool_result_serializes() {
        let event = AgentEvent::ToolResult {
            agent_id: "agent-1".to_string(),
            tool_use_id: "tu-001".to_string(),
            output: "file1.rs\nfile2.rs".to_string(),
            is_error: false,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "tool_result");
        assert_eq!(value["is_error"], false);
    }

    #[test]
    fn agent_event_tree_snapshot_serializes_with_kind() {
        let node = AgentNode {
            agent_id: "agent-root".to_string(),
            parent_agent_id: None,
            description: "Root agent".to_string(),
            agent_type: None,
            model: None,
            state: "running".to_string(),
            is_background: false,
            depth: 0,
            chain_id: "chain-1".to_string(),
            spawned_at: 1713168000000,
            completed_at: None,
            duration_ms: None,
            result_preview: None,
            had_error: false,
            children: vec![],
        };
        let event = AgentEvent::TreeSnapshot { roots: vec![node] };
        let value = serde_json::to_value(&event).expect("serialize AgentEvent::TreeSnapshot");
        assert_eq!(value["kind"], "tree_snapshot");
        let roots = value["roots"].as_array().expect("roots should be array");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0]["agent_id"], "agent-root");
    }

    // -----------------------------------------------------------------------
    // AgentCommand deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn agent_command_abort_agent_deserializes() {
        let json = r#"{"kind":"abort_agent","agent_id":"agent-42"}"#;
        let cmd: AgentCommand = serde_json::from_str(json).expect("deserialize AgentCommand");
        match cmd {
            AgentCommand::AbortAgent { agent_id } => assert_eq!(agent_id, "agent-42"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn agent_command_query_active_agents_deserializes() {
        let json = r#"{"kind":"query_active_agents"}"#;
        let cmd: AgentCommand =
            serde_json::from_str(json).expect("deserialize AgentCommand::QueryActiveAgents");
        assert!(matches!(cmd, AgentCommand::QueryActiveAgents));
    }

    #[test]
    fn agent_command_query_agent_output_deserializes() {
        let json = r#"{"kind":"query_agent_output","agent_id":"agent-7"}"#;
        let cmd: AgentCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            AgentCommand::QueryAgentOutput { agent_id } => assert_eq!(agent_id, "agent-7"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // TeamEvent serialization
    // -----------------------------------------------------------------------

    #[test]
    fn team_event_member_joined_serializes() {
        let event = TeamEvent::MemberJoined {
            team_name: "backend-team".to_string(),
            agent_id: "agent-10".to_string(),
            agent_name: "Alice".to_string(),
            role: Some("reviewer".to_string()),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "member_joined");
        assert_eq!(value["team_name"], "backend-team");
        assert_eq!(value["role"], "reviewer");
    }

    #[test]
    fn team_event_member_joined_without_role() {
        let event = TeamEvent::MemberJoined {
            team_name: "team-1".to_string(),
            agent_id: "agent-11".to_string(),
            agent_name: "Bob".to_string(),
            role: None,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "member_joined");
        assert!(value.get("role").is_none());
    }

    #[test]
    fn team_event_member_left_serializes() {
        let event = TeamEvent::MemberLeft {
            team_name: "team-1".to_string(),
            agent_id: "agent-10".to_string(),
            agent_name: "Alice".to_string(),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "member_left");
        assert_eq!(value["agent_name"], "Alice");
    }

    #[test]
    fn team_event_message_routed_serializes_with_kind() {
        let event = TeamEvent::MessageRouted {
            team_name: "backend-team".to_string(),
            from: "agent-10".to_string(),
            to: "agent-11".to_string(),
            text: "Please review PR #42".to_string(),
            timestamp: "2026-04-15T10:00:00Z".to_string(),
            summary: Some("Review request".to_string()),
        };
        let value = serde_json::to_value(&event).expect("serialize TeamEvent::MessageRouted");
        assert_eq!(value["kind"], "message_routed");
        assert_eq!(value["team_name"], "backend-team");
        assert_eq!(value["from"], "agent-10");
        assert_eq!(value["to"], "agent-11");
        assert_eq!(value["text"], "Please review PR #42");
        assert_eq!(value["summary"], "Review request");
    }

    #[test]
    fn team_event_message_routed_without_summary() {
        let event = TeamEvent::MessageRouted {
            team_name: "team-1".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            text: "hi".to_string(),
            timestamp: "2026-04-15T10:00:00Z".to_string(),
            summary: None,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "message_routed");
        assert!(value.get("summary").is_none());
    }

    #[test]
    fn team_event_status_snapshot_serializes() {
        let event = TeamEvent::StatusSnapshot {
            team_name: "backend-team".to_string(),
            members: vec![TeamMemberInfo {
                agent_id: "agent-10".to_string(),
                agent_name: "Alice".to_string(),
                role: Some("reviewer".to_string()),
                is_active: true,
                unread_messages: 2,
            }],
            pending_messages: 5,
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["kind"], "status_snapshot");
        assert_eq!(value["pending_messages"], 5);
        assert_eq!(value["members"].as_array().unwrap().len(), 1);
    }

    // -----------------------------------------------------------------------
    // TeamCommand deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn team_command_inject_message_deserializes() {
        let json = r#"{"kind":"inject_message","team_name":"backend-team","to":"agent-10","text":"hello"}"#;
        let cmd: TeamCommand =
            serde_json::from_str(json).expect("deserialize TeamCommand::InjectMessage");
        match cmd {
            TeamCommand::InjectMessage {
                team_name,
                to,
                text,
            } => {
                assert_eq!(team_name, "backend-team");
                assert_eq!(to, "agent-10");
                assert_eq!(text, "hello");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn team_command_query_team_status_deserializes() {
        let json = r#"{"kind":"query_team_status","team_name":"ops-team"}"#;
        let cmd: TeamCommand = serde_json::from_str(json).expect("deserialize");
        match cmd {
            TeamCommand::QueryTeamStatus { team_name } => assert_eq!(team_name, "ops-team"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }
}
