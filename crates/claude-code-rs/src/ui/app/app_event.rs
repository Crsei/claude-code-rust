//! App event model for backend-to-TUI routing.

use serde::Serialize;

use cc_ipc_protocol::BackendMessage;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    Backend { message: Box<BackendMessage> },
    LocalNotice { message: String },
    Tick,
    Shutdown,
}

impl AppEvent {
    pub fn backend(message: BackendMessage) -> Self {
        Self::Backend {
            message: Box::new(message),
        }
    }
}
