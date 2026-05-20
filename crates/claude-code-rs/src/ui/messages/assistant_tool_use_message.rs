//! Rust-side helper for assistant tool-use messages.
//!
//! Renders tool-use status with a state machine covering queued, in-progress,
//! resolved, error, waiting-for-permission, and classifier-checking states.
//! Integrates with `ToolActivity` for structured in-progress/resolved rendering.

use crate::ui::theme::Theme;
use crate::ui::tool_activity::{ToolActivity, ToolState};

/// Tool-use lifecycle states used by the renderer.
///
/// Mirrors the TS state calculation in `AssistantToolUseMessage.tsx:95-97`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolUseState {
    /// Tool is waiting in the execution queue.
    #[cfg(test)]
    Queued,
    /// Tool is currently executing.
    InProgress,
    /// Tool execution completed successfully.
    #[cfg(test)]
    Resolved,
    /// Tool execution resulted in an error.
    #[cfg(test)]
    Error,
    /// Waiting for user permission to execute.
    #[cfg(test)]
    WaitingForPermission,
    /// Content classifier is reviewing the tool use.
    #[cfg(test)]
    ClassifierChecking,
}

/// Tools that wrap their content transparently (shell/file-editing tools).
///
/// These are considered "transparent wrappers" — in the TS reference
/// (`AssistantToolUseMessage.tsx:99-114`), they render differently when
/// the real content follows immediately.
#[cfg(test)]
const TRANSPARENT_WRAPPER_TOOLS: &[&str] = &[
    "Bash",
    "PowerShell",
    "Write",
    "Edit",
    "NotebookEdit",
    "Read",
];

/// Check whether a tool name is a transparent wrapper.
#[cfg(test)]
fn is_transparent_wrapper_tool(tool_name: &str) -> bool {
    TRANSPARENT_WRAPPER_TOOLS.contains(&tool_name)
}

/// Render a tool that is queued (waiting its turn).
///
/// TS reference: `AssistantToolUseMessage.tsx:138-140` (queued dot)
#[cfg(test)]
fn render_tool_use_queued_message(tool_name: &str) -> String {
    format!("● {tool_name}")
}

/// Render a tool that is currently executing with progress.
///
/// TS reference: `AssistantToolUseMessage.tsx:221-268`
fn render_tool_use_progress_message(
    tool_name: &str,
    input_summary: &str,
    has_hook_progress: bool,
) -> String {
    let summary = if input_summary.is_empty() {
        tool_name.to_string()
    } else {
        format!("{tool_name}({input_summary})")
    };
    if has_hook_progress {
        format!("● {summary} [hook running]")
    } else {
        format!("● {summary}")
    }
}

/// Render a tool that encountered an error.
#[cfg(test)]
fn render_tool_use_error_state(tool_name: &str, input_summary: &str) -> String {
    let summary = if input_summary.is_empty() {
        tool_name.to_string()
    } else {
        format!("{tool_name}({input_summary})")
    };
    format!("● {summary} [error]")
}

/// Render a tool that is being checked by the content classifier.
///
/// TS reference: `AssistantToolUseMessage.tsx:173-178`
#[cfg(test)]
fn render_classifier_checking(tool_name: &str) -> String {
    format!("● {tool_name} (classifier checking...)")
}

/// Render a tool waiting for user permission.
///
/// TS reference: `AssistantToolUseMessage.tsx:179-182`
#[cfg(test)]
fn render_waiting_for_permission(tool_name: &str) -> String {
    format!("● {tool_name} (waiting for permission...)")
}

/// Render the main tool-use message for a given state.
///
/// `tool_name`: the name of the tool (e.g. "Bash", "Read", "Edit")
/// `input`: raw JSON input string for the tool call
/// `state`: current lifecycle state of the tool use
/// `has_hook_progress`: whether a hook is currently running for this tool
///
/// Returns a single-line string representation.
pub fn render_assistant_tool_use_message(
    tool_name: &str,
    input: &str,
    state: ToolUseState,
    has_hook_progress: bool,
    _theme: &Theme,
) -> String {
    // Build summary from the tool input.
    let activity = ToolActivity::from_tool_use(tool_name, input, ToolState::Running);
    let input_summary = activity.display_call();
    // The display_call includes the tool name, so strip it for summary-only.
    let summary_only = input_summary
        .strip_prefix(&format!(
            "{}(",
            activity.user_facing_name.as_deref().unwrap_or(tool_name)
        ))
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or("");

    match state {
        #[cfg(test)]
        ToolUseState::Queued => render_tool_use_queued_message(tool_name),
        ToolUseState::InProgress => {
            render_tool_use_progress_message(tool_name, summary_only, has_hook_progress)
        }
        #[cfg(test)]
        ToolUseState::Resolved => {
            if is_transparent_wrapper_tool(tool_name) {
                // Keep an explicit completion marker unless the main render path
                // replaces this helper with a richer tool-result row.
                format!("● {tool_name}")
            } else {
                let summary = if summary_only.is_empty() {
                    tool_name.to_string()
                } else {
                    format!("{tool_name}({summary_only})")
                };
                format!("● {summary}")
            }
        }
        #[cfg(test)]
        ToolUseState::Error => render_tool_use_error_state(tool_name, summary_only),
        #[cfg(test)]
        ToolUseState::WaitingForPermission => render_waiting_for_permission(tool_name),
        #[cfg(test)]
        ToolUseState::ClassifierChecking => render_classifier_checking(tool_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_state_renders_dot_and_name() {
        let result = render_assistant_tool_use_message(
            "Read",
            "",
            ToolUseState::Queued,
            false,
            &Theme::default(),
        );
        assert_eq!(result, "● Read");
    }

    #[test]
    fn in_progress_shows_tool_and_input() {
        let result = render_assistant_tool_use_message(
            "Read",
            r#"{"file_path":"src/main.rs"}"#,
            ToolUseState::InProgress,
            false,
            &Theme::default(),
        );
        assert!(result.contains("●"));
        assert!(result.contains("Read"));
        assert!(result.contains("src/main.rs"));
    }

    #[test]
    fn in_progress_with_hook_shows_hook_indicator() {
        let result = render_assistant_tool_use_message(
            "Bash",
            r#"{"command":"cargo build"}"#,
            ToolUseState::InProgress,
            true,
            &Theme::default(),
        );
        assert!(result.contains("[hook running]"));
    }

    #[test]
    fn resolved_transparent_wrapper_keeps_completion_marker() {
        let result = render_assistant_tool_use_message(
            "Bash",
            r#"{"command":"ls"}"#,
            ToolUseState::Resolved,
            false,
            &Theme::default(),
        );
        assert_eq!(result, "● Bash");
    }

    #[test]
    fn resolved_non_transparent_shows_summary() {
        let result = render_assistant_tool_use_message(
            "Grep",
            r#"{"pattern":"fn main"}"#,
            ToolUseState::Resolved,
            false,
            &Theme::default(),
        );
        assert!(result.contains("●"));
    }

    #[test]
    fn error_state_shows_error_indicator() {
        let result = render_assistant_tool_use_message(
            "Bash",
            r#"{"command":"invalid"}"#,
            ToolUseState::Error,
            false,
            &Theme::default(),
        );
        assert!(result.contains("[error]"));
    }

    #[test]
    fn waiting_for_permission_shows_prompt() {
        let result = render_assistant_tool_use_message(
            "Bash",
            "",
            ToolUseState::WaitingForPermission,
            false,
            &Theme::default(),
        );
        assert!(result.contains("waiting for permission"));
    }

    #[test]
    fn classifier_checking_shows_classifier() {
        let result = render_assistant_tool_use_message(
            "Bash",
            "",
            ToolUseState::ClassifierChecking,
            false,
            &Theme::default(),
        );
        assert!(result.contains("classifier checking"));
    }
}
