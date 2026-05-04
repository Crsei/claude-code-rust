//! Rust-side helper for assistant tool-use messages.

use crate::ui::theme::Theme;
use crate::ui::tool_activity::{ToolActivity, ToolState};

pub fn render_assistant_tool_use_message(tool_name: &str, input: &str, _theme: &Theme) -> String {
    let activity = ToolActivity::from_tool_use(tool_name, input, ToolState::Running);
    format!("Assistant used {}", activity.display_call())
}
