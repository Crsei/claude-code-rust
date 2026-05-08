//! Rust-side helper for grouped tool-use output.

use crate::ui::theme::Theme;
use crate::ui::tool_activity::{render_grouped_activity, ToolActivity, ToolState};

pub fn render_grouped_tool_use_content(tool_names: &[&str], _theme: &Theme) -> String {
    let activities = tool_names
        .iter()
        .map(|name| ToolActivity::from_tool_use(name, "", ToolState::Queued))
        .collect::<Vec<_>>();
    format!(
        "Grouped tool uses:\n{}",
        render_grouped_activity(&activities)
    )
}
