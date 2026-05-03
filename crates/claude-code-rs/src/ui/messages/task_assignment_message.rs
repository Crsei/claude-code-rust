//! Rust-side helper for task assignment messages.

use crate::ui::theme::Theme;

pub fn render_task_assignment_message(task: &str, owner: &str, _theme: &Theme) -> String {
    let owner = owner.trim();
    let owner = if owner.is_empty() {
        "unassigned"
    } else {
        owner
    };
    format!("Task '{task}' assigned to {owner}")
}
