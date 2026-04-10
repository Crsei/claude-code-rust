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

/// Receiver half — owned by the event loop (headless/TUI).
#[allow(dead_code)]
pub type BgAgentReceiver = tokio::sync::mpsc::UnboundedReceiver<CompletedBackgroundAgent>;

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
    #[allow(dead_code)]
    pub fn push(&self, agent: CompletedBackgroundAgent) {
        self.inner.lock().push(agent);
    }

    /// Drain all pending results (called by query loop at turn start).
    pub fn drain_all(&self) -> Vec<CompletedBackgroundAgent> {
        let mut guard = self.inner.lock();
        std::mem::take(&mut *guard)
    }
}
