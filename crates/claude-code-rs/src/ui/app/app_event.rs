//! App event model for backend-to-TUI routing.

use serde::Serialize;

use cc_ipc_protocol::BackendMessage;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    Backend {
        message: Box<BackendMessage>,
    },
    #[cfg(test)]
    #[allow(dead_code)] // Phase 1: upstream parity — not wired to production event loop
    LocalNotice {
        message: String,
    },
    #[cfg(test)]
    #[allow(dead_code)] // Phase 1: upstream parity — not wired to production event loop
    Notification {
        key: String,
        message: String,
        level: String,
        timeout_ms: Option<u64>,
    },
    #[cfg(test)]
    #[allow(dead_code)] // Phase 1: upstream parity — not wired to production event loop
    Tick,
    #[cfg(test)]
    #[allow(dead_code)] // Phase 1: upstream parity — not wired to production event loop
    Shutdown,
}

impl AppEvent {
    pub fn backend(message: BackendMessage) -> Self {
        Self::Backend {
            message: Box::new(message),
        }
    }
}
