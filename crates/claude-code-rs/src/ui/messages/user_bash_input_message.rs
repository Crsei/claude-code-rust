//! Rust-side helper for bash input messages.

use crate::ui::theme::Theme;

pub fn render_user_bash_input_message(command: &str, _theme: &Theme) -> String {
    if command.trim().is_empty() {
        "You ran a bash command (empty)".to_string()
    } else {
        format!("> {command}")
    }
}
