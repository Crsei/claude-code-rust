//! Rust-side helper for bash output messages.

use crate::ui::theme::Theme;

pub fn render_user_bash_output_message(command: &str, output: &str, _theme: &Theme) -> String {
    let output = output.trim();
    let body = if output.is_empty() {
        "(no output)"
    } else {
        output
    };
    format!("{command} -> {body}")
}
