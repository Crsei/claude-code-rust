//! Background agent types — shared between the Agent tool, query loop,
//! and the headless/TUI event loop.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

/// Result from a completed background agent.
#[derive(Debug, Clone)]
pub struct CompletedBackgroundAgent {
    pub agent_id: String,
    pub description: String,
    pub result_text: String,
    pub had_error: bool,
    pub duration: Duration,
}

/// Sender half — cloned into each background agent spawn.
pub type BgAgentSender = tokio::sync::mpsc::UnboundedSender<CompletedBackgroundAgent>;

/// Shared buffer of completed agents waiting to be injected into the query loop.
///
/// The event loop pushes completed agents here after notifying the frontend.
/// The query loop drains at turn boundaries and injects system messages.
/// Internal `Mutex` means this is safe to clone and share without external locking.
#[derive(Debug, Clone, Default)]
pub struct PendingBackgroundResults {
    inner: Arc<Mutex<Vec<CompletedBackgroundAgent>>>,
}

impl PendingBackgroundResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a completed agent result (called by event loop).
    pub fn push(&self, agent: CompletedBackgroundAgent) {
        self.inner.lock().push(agent);
    }

    /// Drain all pending results (called by query loop at turn start).
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
    fn test_pending_results_push_and_drain() {
        let pending = PendingBackgroundResults::new();
        assert!(pending.drain_all().is_empty());

        pending.push(make_completed("a1", "task one"));
        pending.push(make_completed("a2", "task two"));

        let drained = pending.drain_all();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].agent_id, "a1");
        assert_eq!(drained[1].agent_id, "a2");

        // Second drain is empty
        assert!(pending.drain_all().is_empty());
    }

    #[test]
    fn test_pending_results_clone_shares_state() {
        let pending1 = PendingBackgroundResults::new();
        let pending2 = pending1.clone();

        pending1.push(make_completed("a1", "task"));
        let drained = pending2.drain_all();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].agent_id, "a1");
    }

    #[test]
    fn test_channel_send_recv() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(make_completed("bg1", "background task")).unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.agent_id, "bg1");
        assert_eq!(received.description, "background task");
        assert!(!received.had_error);
    }
}
