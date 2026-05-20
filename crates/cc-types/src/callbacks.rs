use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structured payload for an interactive permission request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequestPayload {
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub message: String,
    pub options: Vec<String>,
}

impl PermissionRequestPayload {
    /// Legacy display string used by older IPC clients that only render a
    /// command/description field.
    pub fn legacy_command(&self) -> String {
        if self.message.trim().is_empty() {
            self.tool_name.clone()
        } else {
            format!("{}: {}", self.tool_name, self.message)
        }
    }
}

/// Async callback for interactive permission requests.
pub type PermissionCallback = Arc<
    dyn Fn(PermissionRequestPayload) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync,
>;

/// Async callback for interactive "ask the user" tool requests.
pub type AskUserCallback =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

/// Tool progress callback payload.
#[derive(Debug, Clone)]
pub struct ToolProgress {
    pub tool_use_id: String,
    pub data: Value,
}

/// Host capability needed to install interactive callbacks.
pub trait CallbackHost {
    fn set_permission_callback(&self, cb: PermissionCallback);
    fn set_ask_user_callback(&self, cb: AskUserCallback);
    fn set_tool_progress_callback(&self, cb: Arc<dyn Fn(ToolProgress) + Send + Sync>);
}
