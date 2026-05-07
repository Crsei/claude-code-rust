//! MCP reconnect status rendering.

use super::utils::reconnect_helpers::{ReconnectAttempt, reconnect_label};

pub fn render_mcp_reconnect(attempt: &ReconnectAttempt) -> String {
    format!("{}\nEnter retry | Esc cancel", reconnect_label(attempt))
}
