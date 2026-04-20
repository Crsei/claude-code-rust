//! Agent tree state manager — maintains a global tree of agent nodes.

#![allow(dead_code)] // Types are pre-defined for upcoming agent IPC extension tasks

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

use super::agent_types::AgentNode;

/// Manages the agent hierarchy tree.
pub struct AgentTreeManager {
    nodes: HashMap<String, AgentNode>,
    roots: Vec<String>,
}

/// Global agent tree instance.
pub static AGENT_TREE: LazyLock<Mutex<AgentTreeManager>> =
    LazyLock::new(|| Mutex::new(AgentTreeManager::new()));

impl AgentTreeManager {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            roots: Vec::new(),
        }
    }

    /// Register a new agent node.
    pub fn register(&mut self, node: AgentNode) {
        let id = node.agent_id.clone();
        let parent_id = node.parent_agent_id.clone();
        self.nodes.insert(id.clone(), node);
        if parent_id.is_none() && !self.roots.contains(&id) {
            self.roots.push(id);
        }
    }

    /// Update an agent's state.
    pub fn update_state(
        &mut self,
        agent_id: &str,
        state: &str,
        result_preview: Option<String>,
        duration_ms: Option<u64>,
        had_error: bool,
    ) {
        if let Some(node) = self.nodes.get_mut(agent_id) {
            node.state = state.to_string();
            node.had_error = had_error;
            if let Some(rp) = result_preview {
                node.result_preview = Some(rp);
            }
            if let Some(d) = duration_ms {
                node.duration_ms = Some(d);
                node.completed_at = Some(chrono::Utc::now().timestamp());
            }
        }
    }

    /// Build a tree snapshot (flat nodes -> nested tree).
    pub fn build_snapshot(&self) -> Vec<AgentNode> {
        self.roots
            .iter()
            .filter_map(|id| self.build_subtree(id))
            .collect()
    }

    fn build_subtree(&self, id: &str) -> Option<AgentNode> {
        let node = self.nodes.get(id)?;
        let mut cloned = node.clone();
        cloned.children = self
            .nodes
            .values()
            .filter(|n| n.parent_agent_id.as_deref() == Some(id))
            .filter_map(|n| self.build_subtree(&n.agent_id))
            .collect();
        Some(cloned)
    }

    /// Get a reference to a node.
    pub fn get(&self, agent_id: &str) -> Option<&AgentNode> {
        self.nodes.get(agent_id)
    }

    /// Get all currently running agents.
    pub fn active_agents(&self) -> Vec<&AgentNode> {
        self.nodes
            .values()
            .filter(|n| n.state == "running")
            .collect()
    }

    /// Remove completed agents older than max_age_secs.
    pub fn remove_completed(&mut self, max_age_secs: u64) {
        let now = chrono::Utc::now().timestamp();
        let to_remove: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, n)| {
                n.state != "running"
                    && n.completed_at
                        .map(|t| (now - t) as u64 > max_age_secs)
                        .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_remove {
            self.nodes.remove(id);
            self.roots.retain(|r| r != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::agent_types::AgentNode;

    fn make_node(id: &str, parent: Option<&str>, bg: bool) -> AgentNode {
        AgentNode {
            agent_id: id.into(),
            parent_agent_id: parent.map(|s| s.into()),
            description: format!("agent {}", id),
            agent_type: None,
            model: None,
            state: "running".into(),
            is_background: bg,
            depth: if parent.is_some() { 2 } else { 1 },
            chain_id: "c1".into(),
            spawned_at: 100,
            completed_at: None,
            duration_ms: None,
            result_preview: None,
            had_error: false,
            children: vec![],
        }
    }

    #[test]
    fn register_and_snapshot() {
        let mut mgr = AgentTreeManager::new();
        mgr.register(make_node("a1", None, true));
        mgr.register(make_node("a2", Some("a1"), false));
        let snap = mgr.build_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].agent_id, "a1");
        assert_eq!(snap[0].children.len(), 1);
        assert_eq!(snap[0].children[0].agent_id, "a2");
    }

    #[test]
    fn update_state() {
        let mut mgr = AgentTreeManager::new();
        mgr.register(make_node("a1", None, false));
        mgr.update_state("a1", "completed", Some("done".into()), Some(5000), false);
        let node = mgr.get("a1").unwrap();
        assert_eq!(node.state, "completed");
        assert_eq!(node.result_preview.as_deref(), Some("done"));
        assert_eq!(node.duration_ms, Some(5000));
    }

    #[test]
    fn active_agents_filters() {
        let mut mgr = AgentTreeManager::new();
        mgr.register(make_node("a1", None, true));
        mgr.register(make_node("a2", None, false));
        mgr.update_state("a2", "completed", None, None, false);
        let active = mgr.active_agents();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].agent_id, "a1");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let mgr = AgentTreeManager::new();
        assert!(mgr.get("nope").is_none());
    }

    #[test]
    fn empty_snapshot() {
        let mgr = AgentTreeManager::new();
        assert!(mgr.build_snapshot().is_empty());
    }
}
