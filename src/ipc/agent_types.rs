//! Shared data types for agent tree, agent info, and team member IPC messages.
//!
//! These types are used by:
//! - Agent-related IPC events and protocol extensions
//! - The `SystemStatus` tool (flat `AgentInfo` variant)
//! - Background agent orchestration and team coordination
//!
//! All types are `Serialize + Deserialize + Debug + Clone` so they can flow
//! freely across the JSONL/SSE boundary between the Rust backend and any
//! frontend process.

#![allow(dead_code)] // Types are pre-defined for upcoming agent IPC extension tasks

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Agent tree node
// ---------------------------------------------------------------------------

/// Recursive tree node representing a running or completed agent.
///
/// Used to build a full agent hierarchy for the frontend, where each node
/// may have zero or more `children` forming an arbitrarily deep tree.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentNode {
    /// Unique identifier for this agent instance.
    pub agent_id: String,
    /// Identifier of the parent agent that spawned this one, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    /// Human-readable description of this agent's purpose.
    pub description: String,
    /// Optional agent type label (e.g. "tool", "coordinator", "worker").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// Model used by this agent (e.g. "claude-sonnet-4-20250514").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Current lifecycle state: "running", "completed", "error", or "aborted".
    pub state: String,
    /// Whether this agent runs in the background (non-blocking).
    pub is_background: bool,
    /// Nesting depth in the agent tree (root = 0).
    pub depth: usize,
    /// Chain identifier grouping related agents in a single execution chain.
    pub chain_id: String,
    /// Unix timestamp (milliseconds) when this agent was spawned.
    pub spawned_at: i64,
    /// Unix timestamp (milliseconds) when this agent completed, if finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    /// Wall-clock duration in milliseconds from spawn to completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Truncated preview of the agent's result or output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_preview: Option<String>,
    /// Whether this agent encountered an error during execution.
    pub had_error: bool,
    /// Child agents spawned by this agent.
    pub children: Vec<AgentNode>,
}

// ---------------------------------------------------------------------------
// Flat agent info (for SystemStatus tool)
// ---------------------------------------------------------------------------

/// Flat (non-recursive) agent information used by the `SystemStatus` tool.
///
/// Unlike `AgentNode`, this struct does not carry children — it is intended
/// for simple status listings rather than tree rendering.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentInfo {
    /// Unique identifier for this agent instance.
    pub agent_id: String,
    /// Identifier of the parent agent, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    /// Human-readable description of this agent's purpose.
    pub description: String,
    /// Current lifecycle state: "running", "completed", "error", or "aborted".
    pub state: String,
    /// Whether this agent runs in the background (non-blocking).
    pub is_background: bool,
    /// Nesting depth in the agent tree (root = 0).
    pub depth: usize,
    /// Wall-clock duration in milliseconds from spawn to completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Team member info
// ---------------------------------------------------------------------------

/// Status information for a team member in a coordinated agent group.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TeamMemberInfo {
    /// Unique identifier for this team member's agent.
    pub agent_id: String,
    /// Display name of this team member.
    pub agent_name: String,
    /// Optional role label (e.g. "reviewer", "implementer").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Whether this team member is currently active/online.
    pub is_active: bool,
    /// Number of unread messages from this team member.
    pub unread_messages: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_node_with_children_serializes_correctly() {
        let child = AgentNode {
            agent_id: "agent-child-1".to_string(),
            parent_agent_id: Some("agent-root".to_string()),
            description: "Run tests".to_string(),
            agent_type: None,
            model: None,
            state: "completed".to_string(),
            is_background: false,
            depth: 1,
            chain_id: "chain-abc".to_string(),
            spawned_at: 1713168001000,
            completed_at: Some(1713168005000),
            duration_ms: Some(4000),
            result_preview: Some("All 42 tests passed".to_string()),
            had_error: false,
            children: vec![],
        };

        let root = AgentNode {
            agent_id: "agent-root".to_string(),
            parent_agent_id: None,
            description: "Implement feature X".to_string(),
            agent_type: Some("coordinator".to_string()),
            model: Some("claude-sonnet-4-20250514".to_string()),
            state: "running".to_string(),
            is_background: false,
            depth: 0,
            chain_id: "chain-abc".to_string(),
            spawned_at: 1713168000000,
            completed_at: None,
            duration_ms: None,
            result_preview: None,
            had_error: false,
            children: vec![child],
        };

        let value = serde_json::to_value(&root).expect("serialize AgentNode");

        // Root: None fields should be omitted
        assert!(
            value.get("parent_agent_id").is_none(),
            "None parent_agent_id should be omitted"
        );
        assert!(
            value.get("completed_at").is_none(),
            "None completed_at should be omitted"
        );
        assert!(
            value.get("duration_ms").is_none(),
            "None duration_ms should be omitted"
        );
        assert!(
            value.get("result_preview").is_none(),
            "None result_preview should be omitted"
        );

        // Root: present Optional fields should be included
        assert_eq!(value["agent_type"], "coordinator");
        assert_eq!(value["model"], "claude-sonnet-4-20250514");

        // Root: required fields
        assert_eq!(value["agent_id"], "agent-root");
        assert_eq!(value["state"], "running");
        assert_eq!(value["depth"], 0);
        assert_eq!(value["is_background"], false);
        assert_eq!(value["had_error"], false);

        // Children
        let children = value["children"].as_array().expect("children should be array");
        assert_eq!(children.len(), 1);

        let child_val = &children[0];
        assert_eq!(child_val["agent_id"], "agent-child-1");
        assert_eq!(child_val["parent_agent_id"], "agent-root");
        assert_eq!(child_val["completed_at"], 1713168005000_i64);
        assert_eq!(child_val["duration_ms"], 4000);
        assert_eq!(child_val["result_preview"], "All 42 tests passed");
        // Child: None optional fields omitted
        assert!(child_val.get("agent_type").is_none());
        assert!(child_val.get("model").is_none());

        // Roundtrip
        let json = serde_json::to_string(&root).expect("serialize to string");
        let parsed: AgentNode = serde_json::from_str(&json).expect("deserialize AgentNode");
        assert_eq!(parsed.agent_id, "agent-root");
        assert_eq!(parsed.children.len(), 1);
        assert_eq!(parsed.children[0].agent_id, "agent-child-1");
    }

    #[test]
    fn agent_info_serializes_and_omits_none_fields() {
        let info = AgentInfo {
            agent_id: "agent-42".to_string(),
            parent_agent_id: None,
            description: "Background linter".to_string(),
            state: "running".to_string(),
            is_background: true,
            depth: 0,
            duration_ms: None,
        };

        let value = serde_json::to_value(&info).expect("serialize AgentInfo");
        assert_eq!(value["agent_id"], "agent-42");
        assert_eq!(value["is_background"], true);
        assert!(value.get("parent_agent_id").is_none());
        assert!(value.get("duration_ms").is_none());

        // With optional fields present
        let info_full = AgentInfo {
            agent_id: "agent-43".to_string(),
            parent_agent_id: Some("agent-42".to_string()),
            description: "Sub-task".to_string(),
            state: "completed".to_string(),
            is_background: false,
            depth: 1,
            duration_ms: Some(1500),
        };

        let value_full = serde_json::to_value(&info_full).expect("serialize full AgentInfo");
        assert_eq!(value_full["parent_agent_id"], "agent-42");
        assert_eq!(value_full["duration_ms"], 1500);

        // Roundtrip
        let json = serde_json::to_string(&info_full).expect("serialize to string");
        let parsed: AgentInfo = serde_json::from_str(&json).expect("deserialize AgentInfo");
        assert_eq!(parsed.agent_id, "agent-43");
        assert_eq!(parsed.duration_ms, Some(1500));
    }

    #[test]
    fn team_member_info_serializes_and_omits_none_role() {
        let member = TeamMemberInfo {
            agent_id: "team-member-1".to_string(),
            agent_name: "Alice".to_string(),
            role: None,
            is_active: true,
            unread_messages: 3,
        };

        let value = serde_json::to_value(&member).expect("serialize TeamMemberInfo");
        assert_eq!(value["agent_id"], "team-member-1");
        assert_eq!(value["agent_name"], "Alice");
        assert_eq!(value["is_active"], true);
        assert_eq!(value["unread_messages"], 3);
        assert!(value.get("role").is_none(), "None role should be omitted");

        // With role present
        let member_with_role = TeamMemberInfo {
            agent_id: "team-member-2".to_string(),
            agent_name: "Bob".to_string(),
            role: Some("reviewer".to_string()),
            is_active: false,
            unread_messages: 0,
        };

        let value2 = serde_json::to_value(&member_with_role).expect("serialize");
        assert_eq!(value2["role"], "reviewer");
        assert_eq!(value2["is_active"], false);

        // Roundtrip
        let json = serde_json::to_string(&member_with_role).expect("serialize to string");
        let parsed: TeamMemberInfo =
            serde_json::from_str(&json).expect("deserialize TeamMemberInfo");
        assert_eq!(parsed.agent_name, "Bob");
        assert_eq!(parsed.role.as_deref(), Some("reviewer"));
        assert_eq!(parsed.unread_messages, 0);
    }
}
