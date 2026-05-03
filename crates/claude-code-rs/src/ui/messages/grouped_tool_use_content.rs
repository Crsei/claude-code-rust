//! Rust-side helper for grouped tool-use output.

use crate::ui::theme::Theme;

pub fn render_grouped_tool_use_content(tool_names: &[&str], _theme: &Theme) -> String {
    if tool_names.is_empty() {
        return "Grouped tool uses: none".to_string();
    }

    let mut out = String::from("Grouped tool uses:");
    for name in tool_names {
        out.push_str(&format!("\n- {name}"));
    }
    out
}
