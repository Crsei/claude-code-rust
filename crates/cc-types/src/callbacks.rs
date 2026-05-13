use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

/// Async callback for interactive permission requests.
pub type PermissionCallback = Arc<
    dyn Fn(String, String, String, Vec<String>) -> Pin<Box<dyn Future<Output = String> + Send>>
        + Send
        + Sync,
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
