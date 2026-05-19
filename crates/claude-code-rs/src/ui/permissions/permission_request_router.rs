//! Routes tool permission requests to tool-specific renderers.

use super::bash_permission_request::bash_permission_request::render_bash_permission_request;
use super::fallback_permission_request::render_fallback_permission_request;
use super::file_edit_permission_request::file_edit_permission_request::render_file_edit_permission_request;
use super::file_permission_dialog::permission_options::file_permission_options;
use super::file_write_permission_request::file_write_permission_request::render_file_write_permission_request;
use super::power_shell_permission_request::power_shell_permission_request::render_power_shell_permission_request;
use super::shell_permission_helpers::shell_permission_options;
use super::utils::default_permission_options;
use super::web_fetch_permission_request::web_fetch_permission_request::render_web_fetch_permission_request;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRouteKind {
    Bash,
    FileEdit,
    FileWrite,
    PowerShell,
    WebFetch,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedPermissionRequest {
    pub kind: PermissionRouteKind,
    pub rendered: String,
    pub options: Vec<String>,
}

pub struct PermissionRequestRouter;

impl PermissionRequestRouter {
    pub fn route(
        tool_name: &str,
        input: &str,
        message: &str,
        selected_index: usize,
    ) -> RoutedPermissionRequest {
        let normalized_tool = tool_name.to_ascii_lowercase();
        let subject = subject(input, message);

        if is_web_fetch_tool(&normalized_tool) {
            let url = extract_url(input, message).unwrap_or_else(|| subject.clone());
            return RoutedPermissionRequest {
                kind: PermissionRouteKind::WebFetch,
                rendered: render_web_fetch_permission_request(&url, "GET", selected_index),
                options: option_labels(default_permission_options()),
            };
        }

        if is_powershell_tool(&normalized_tool) {
            let command = command_subject(input, message);
            return RoutedPermissionRequest {
                kind: PermissionRouteKind::PowerShell,
                rendered: render_power_shell_permission_request(&command, selected_index),
                options: option_labels(shell_permission_options(&command)),
            };
        }

        if is_bash_tool(&normalized_tool) {
            let command = command_subject(input, message);
            return RoutedPermissionRequest {
                kind: PermissionRouteKind::Bash,
                rendered: render_bash_permission_request(&command, selected_index),
                options: option_labels(shell_permission_options(&command)),
            };
        }

        if is_file_write_tool(&normalized_tool) {
            let path = path_subject(input, message);
            return RoutedPermissionRequest {
                kind: PermissionRouteKind::FileWrite,
                rendered: render_file_write_permission_request(&path, 0, 0, selected_index),
                options: option_labels(file_permission_options(&path, false)),
            };
        }

        if is_file_edit_tool(&normalized_tool) {
            let path = path_subject(input, message);
            return RoutedPermissionRequest {
                kind: PermissionRouteKind::FileEdit,
                rendered: render_file_edit_permission_request(&path, "edit", selected_index),
                options: option_labels(file_permission_options(&path, false)),
            };
        }

        let details = if message.trim().is_empty() || message.trim() == subject.trim() {
            Vec::<String>::new()
        } else {
            vec![message.trim().to_string()]
        };
        RoutedPermissionRequest {
            kind: PermissionRouteKind::Fallback,
            rendered: render_fallback_permission_request(tool_name, &subject, &details),
            options: option_labels(default_permission_options()),
        }
    }
}

fn option_labels(options: Vec<super::utils::PermissionOption>) -> Vec<String> {
    options.into_iter().map(|option| option.label).collect()
}

fn subject(input: &str, message: &str) -> String {
    if !input.trim().is_empty() {
        input.trim().to_string()
    } else if let Some(url) = extract_url(input, message) {
        url
    } else if !message.trim().is_empty() {
        message.trim().to_string()
    } else {
        "(no details supplied)".to_string()
    }
}

fn command_subject(input: &str, message: &str) -> String {
    structured_string(input, &["command", "cmd", "script"])
        .or_else(|| structured_string(message, &["command", "cmd", "script"]))
        .unwrap_or_else(|| subject(input, message))
}

fn path_subject(input: &str, message: &str) -> String {
    structured_string(
        input,
        &["path", "file_path", "filepath", "file", "notebook_path"],
    )
    .or_else(|| {
        structured_string(
            message,
            &["path", "file_path", "filepath", "file", "notebook_path"],
        )
    })
    .unwrap_or_else(|| subject(input, message))
}

fn extract_url(input: &str, message: &str) -> Option<String> {
    if let Some(url) = structured_string(input, &["url", "uri", "href"]) {
        return Some(url);
    }
    if let Some(url) = structured_string(message, &["url", "uri", "href"]) {
        return Some(url);
    }
    let source = if input.trim().is_empty() {
        message
    } else {
        input
    };
    source
        .split_whitespace()
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .map(|part| {
            part.trim_matches(|ch: char| "\"'(),".contains(ch))
                .to_string()
        })
}

fn structured_string(source: &str, keys: &[&str]) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(source).ok()?;
    find_structured_string(&value, keys)
}

fn find_structured_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(|value| value.as_str()) {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            map.values()
                .find_map(|value| find_structured_string(value, keys))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| find_structured_string(value, keys)),
        _ => None,
    }
}

fn is_web_fetch_tool(tool_name: &str) -> bool {
    tool_name.contains("webfetch")
        || tool_name.contains("web_fetch")
        || tool_name.contains("fetch")
        || tool_name.contains("url")
}

fn is_powershell_tool(tool_name: &str) -> bool {
    tool_name.contains("powershell") || tool_name.contains("pwsh")
}

fn is_bash_tool(tool_name: &str) -> bool {
    tool_name.contains("bash") || tool_name.contains("shell") || tool_name.contains("exec_command")
}

fn is_file_write_tool(tool_name: &str) -> bool {
    tool_name.contains("write") || tool_name.contains("create_file")
}

fn is_file_edit_tool(tool_name: &str) -> bool {
    tool_name.contains("edit")
        || tool_name.contains("patch")
        || tool_name.contains("multiedit")
        || tool_name.contains("notebook")
        || tool_name.contains("sed")
}

#[cfg(test)]
mod tests {
    use super::{PermissionRequestRouter, PermissionRouteKind};

    #[test]
    fn routes_bash_requests_to_bash_renderer() {
        let routed = PermissionRequestRouter::route("Bash", "cargo test --workspace", "", 0);
        assert_eq!(routed.kind, PermissionRouteKind::Bash);
        assert!(routed.rendered.contains("Bash command permission"));
        assert!(routed.rendered.contains("shell: bash"));
        assert!(routed.rendered.contains("risk:"));
    }

    #[test]
    fn extracts_structured_bash_command() {
        let routed = PermissionRequestRouter::route(
            "Bash",
            r#"{"command":"cargo test -p claude-code-rs"}"#,
            "",
            0,
        );
        assert_eq!(routed.kind, PermissionRouteKind::Bash);
        assert!(routed.rendered.contains("cargo test -p claude-code-rs"));
        assert!(!routed.rendered.contains("{\"command\""));
        assert!(routed
            .options
            .contains(&"Always allow exact command".to_string()));
    }

    #[test]
    fn routes_file_edit_requests_to_file_renderer() {
        let routed = PermissionRequestRouter::route("Edit", "src/main.rs", "", 0);
        assert_eq!(routed.kind, PermissionRouteKind::FileEdit);
        assert!(routed.rendered.contains("File edit permission"));
        assert!(routed.rendered.contains("src/main.rs"));
    }

    #[test]
    fn routes_web_fetch_requests_to_web_renderer() {
        let routed = PermissionRequestRouter::route(
            "WebFetch",
            "",
            "WebFetch permission required for https://example.com/docs?q=1",
            0,
        );
        assert_eq!(routed.kind, PermissionRouteKind::WebFetch);
        assert!(routed.rendered.contains("Web fetch permission"));
        assert!(routed.rendered.contains("https://example.com/docs?q=1"));
        assert!(routed.rendered.contains("method: GET"));
    }

    #[test]
    fn extracts_structured_web_fetch_url() {
        let routed = PermissionRequestRouter::route(
            "WebFetch",
            r#"{"url":"https://example.com/json","method":"POST"}"#,
            "",
            0,
        );
        assert_eq!(routed.kind, PermissionRouteKind::WebFetch);
        assert!(routed.rendered.contains("https://example.com/json"));
        assert!(!routed.rendered.contains("{\"url\""));
    }

    #[test]
    fn falls_back_for_unknown_tool_requests() {
        let routed =
            PermissionRequestRouter::route("UnknownTool", "", "Runs proprietary action", 0);
        assert_eq!(routed.kind, PermissionRouteKind::Fallback);
        assert!(routed.rendered.contains("Permission required"));
        assert!(routed.rendered.contains("unknown tool behavior"));
    }
}
