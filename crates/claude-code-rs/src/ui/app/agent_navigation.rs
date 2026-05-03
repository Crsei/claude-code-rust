//! Pure multi-agent navigation state for the cc-rust TUI.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentThreadEntry {
    pub thread_id: String,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub is_primary: bool,
    pub is_closed: bool,
}

impl AgentThreadEntry {
    pub fn label(&self) -> String {
        if self.is_primary {
            return "Primary".to_string();
        }
        self.agent_nickname
            .as_deref()
            .or(self.agent_role.as_deref())
            .unwrap_or("Agent")
            .to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentNavigationDirection {
    Previous,
    Next,
}

#[derive(Debug, Default, Clone)]
pub struct AgentNavigationState {
    entries: BTreeMap<String, AgentThreadEntry>,
    order: Vec<String>,
}

impl AgentNavigationState {
    pub fn upsert(&mut self, entry: AgentThreadEntry) {
        if !self.entries.contains_key(&entry.thread_id) {
            self.order.push(entry.thread_id.clone());
        }
        self.entries.insert(entry.thread_id.clone(), entry);
    }

    pub fn mark_closed(&mut self, thread_id: &str) {
        if let Some(entry) = self.entries.get_mut(thread_id) {
            entry.is_closed = true;
        }
    }

    pub fn remove(&mut self, thread_id: &str) {
        self.entries.remove(thread_id);
        self.order.retain(|candidate| candidate != thread_id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn ordered_threads(&self) -> Vec<&AgentThreadEntry> {
        self.order
            .iter()
            .filter_map(|thread_id| self.entries.get(thread_id))
            .collect()
    }

    pub fn adjacent_thread_id(
        &self,
        current_thread_id: &str,
        direction: AgentNavigationDirection,
    ) -> Option<String> {
        let ordered = self.ordered_threads();
        if ordered.len() < 2 {
            return None;
        }
        let current = ordered
            .iter()
            .position(|entry| entry.thread_id == current_thread_id)?;
        let next = match direction {
            AgentNavigationDirection::Next => (current + 1) % ordered.len(),
            AgentNavigationDirection::Previous => {
                if current == 0 {
                    ordered.len() - 1
                } else {
                    current - 1
                }
            }
        };
        Some(ordered[next].thread_id.clone())
    }

    pub fn active_agent_label(&self, current_thread_id: &str) -> Option<String> {
        if self.entries.len() <= 1 {
            return None;
        }
        self.entries
            .get(current_thread_id)
            .map(AgentThreadEntry::label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> AgentThreadEntry {
        AgentThreadEntry {
            thread_id: id.to_string(),
            agent_nickname: None,
            agent_role: None,
            is_primary: false,
            is_closed: false,
        }
    }

    #[test]
    fn navigates_in_insert_order() {
        let mut state = AgentNavigationState::default();
        state.upsert(entry("a"));
        state.upsert(entry("b"));
        assert_eq!(
            state.adjacent_thread_id("a", AgentNavigationDirection::Next),
            Some("b".to_string())
        );
    }
}
