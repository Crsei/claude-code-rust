//! Engine-owned runtime support for background Agent executions.
//!
//! These types bridge the Agent runtime event loop and the query lifecycle:
//! the event loop pushes completed background Agent results, and the engine
//! drains them at turn boundaries for injection into the conversation.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

/// Result from a completed background Agent.
#[derive(Debug, Clone)]
pub struct CompletedBackgroundAgent {
    pub agent_id: String,
    pub description: String,
    pub result_text: String,
    pub had_error: bool,
    pub duration: Duration,
}

/// Shared buffer of completed agents waiting to be injected into the query loop.
///
/// Internal locking keeps clones connected to the same queue without requiring
/// external synchronization at the event-loop/query-loop boundary.
#[derive(Debug, Clone, Default)]
pub struct PendingBackgroundResults {
    inner: Arc<Mutex<Vec<CompletedBackgroundAgent>>>,
}

impl PendingBackgroundResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a completed agent result from the runtime event loop.
    pub fn push(&self, agent: CompletedBackgroundAgent) {
        self.inner.lock().push(agent);
    }

    /// Drain all pending results at a query turn boundary.
    pub fn drain_all(&self) -> Vec<CompletedBackgroundAgent> {
        let mut guard = self.inner.lock();
        std::mem::take(&mut *guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_completed(id: &str, desc: &str) -> CompletedBackgroundAgent {
        CompletedBackgroundAgent {
            agent_id: id.to_string(),
            description: desc.to_string(),
            result_text: format!("Result from {}", desc),
            had_error: false,
            duration: Duration::from_secs(1),
        }
    }

    #[test]
    fn pending_results_push_and_drain() {
        let pending = PendingBackgroundResults::new();
        assert!(pending.drain_all().is_empty());

        pending.push(make_completed("a1", "task one"));
        pending.push(make_completed("a2", "task two"));

        let drained = pending.drain_all();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].agent_id, "a1");
        assert_eq!(drained[1].agent_id, "a2");

        assert!(pending.drain_all().is_empty());
    }

    #[test]
    fn pending_results_clone_shares_state() {
        let pending1 = PendingBackgroundResults::new();
        let pending2 = pending1.clone();

        pending1.push(make_completed("a1", "task"));
        let drained = pending2.drain_all();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].agent_id, "a1");
    }
}
