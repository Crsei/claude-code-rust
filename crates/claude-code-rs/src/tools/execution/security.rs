//! Security validation for tool execution.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use crate::permissions::dangerous;
use crate::permissions::path_validation;
use crate::types::tool::ToolUseContext;
use crate::types::tool::{PermissionMode, Tool, Tools};

use super::{make_error_result, ToolExecutionResult};

/// Centralized security checks run before hooks and permission evaluation.
///
/// Returns `Some(ToolExecutionResult)` if the tool call should be **rejected**,
/// or `None` if validation passed and the pipeline should continue.
///
/// Checks performed (in order):
///   3c.1  Plan-mode gate — non-read-only tools are blocked in Plan mode
///         except writes to the dedicated plan file
///   3c.2  Dangerous command detection — Bash/PowerShell commands screened
///   3c.3  Path boundary enforcement — Write/Edit paths must be within allowed dirs
///
/// All checks are skipped when the permission mode is `Bypass`.
pub(crate) fn security_validate(
    tool_use_id: &str,
    tool_name: &str,
    input: &Value,
    tool: &dyn Tool,
    ctx: &ToolUseContext,
    started: Instant,
) -> Option<ToolExecutionResult> {
    let app_state = (ctx.get_app_state)();
    let mode = &app_state.tool_permission_context.mode;

    // Bypass mode skips all security validation
    if *mode == PermissionMode::Bypass {
        return None;
    }

    let plan_file_write_allowed =
        *mode == PermissionMode::Plan && is_plan_mode_plan_file_write(tool_name, input);

    // ── 3c.1: Plan mode gate ───────────────────────────────────────
    if *mode == PermissionMode::Plan && !tool.is_read_only(input) && !plan_file_write_allowed {
        return Some(make_error_result(
            tool_use_id,
            tool_name,
            &format!(
                "Tool '{}' is not available in Plan mode (read-only exploration only)",
                tool_name
            ),
            started,
        ));
    }

    // ── 3c.2: Dangerous command check (Bash / PowerShell) ──────────
    if tool_name == "Bash" || tool_name == "PowerShell" {
        if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
            let danger = if tool_name == "PowerShell" {
                dangerous::is_dangerous_powershell_command(command)
            } else {
                dangerous::is_dangerous_command(command)
            };

            if let Some(reason) = danger {
                return Some(make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!("Dangerous command blocked: {}", reason),
                    started,
                ));
            }
        }
    }

    // ── 3c.3: Path boundary check (Write / Edit) ──────────────────
    const FILE_TOOL_NAMES: &[&str] = &["Write", "Edit", "FileWrite", "FileEdit"];
    if FILE_TOOL_NAMES.contains(&tool_name) && !plan_file_write_allowed {
        if let Some(file_path_str) = input.get("file_path").and_then(|v| v.as_str()) {
            // Step 1: validate path structure (traversal attacks, null bytes, etc.)
            let canonical = match path_validation::validate_file_path(file_path_str) {
                Ok(p) => p,
                Err(e) => {
                    return Some(make_error_result(
                        tool_use_id,
                        tool_name,
                        &format!("Invalid file path: {}", e),
                        started,
                    ));
                }
            };

            // Step 2: check the path is within allowed directories
            let cwd = crate::bootstrap::PROCESS_STATE.read().original_cwd.clone();
            let perm_ctx = &app_state.tool_permission_context;

            if !path_validation::is_path_within_allowed_directories(&canonical, &cwd, perm_ctx) {
                return Some(make_error_result(
                    tool_use_id,
                    tool_name,
                    &format!(
                        "Path '{}' is outside the allowed working directories. \
                         Allowed: {}{}",
                        file_path_str,
                        cwd.display(),
                        if perm_ctx.additional_working_directories.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " (and {} additional directories)",
                                perm_ctx.additional_working_directories.len()
                            )
                        }
                    ),
                    started,
                ));
            }
        }
    }

    None // all checks passed
}

/// True when a write/edit tool targets the dedicated plan file that plan mode
/// is allowed to maintain.
pub(crate) fn is_plan_mode_plan_file_write(tool_name: &str, input: &Value) -> bool {
    const PLAN_FILE_WRITE_TOOLS: &[&str] = &["Write", "Edit", "FileWrite", "FileEdit"];
    if !PLAN_FILE_WRITE_TOOLS.contains(&tool_name) {
        return false;
    }

    let Some(file_path) = input.get("file_path").and_then(|value| value.as_str()) else {
        return false;
    };
    let Ok(candidate) = path_validation::validate_file_path(file_path) else {
        return false;
    };

    let cwd = crate::bootstrap::state::original_cwd();
    let plan_path = crate::config::paths::current_plan_file_path(&cwd);
    paths_equivalent_for_plan_file(&candidate, &plan_path)
}

fn paths_equivalent_for_plan_file(candidate: &Path, plan_path: &Path) -> bool {
    normalize_for_compare(candidate) == normalize_for_compare(plan_path)
}

fn normalize_for_compare(path: &Path) -> String {
    let normalized = path
        .canonicalize()
        .unwrap_or_else(|_| normalize_nonexistent_path(path));
    let mut rendered = normalized.to_string_lossy().replace('\\', "/");
    while rendered.len() > 1 && rendered.ends_with('/') && !rendered.ends_with(":/") {
        rendered.pop();
    }
    if cfg!(windows) {
        rendered.make_ascii_lowercase();
    }
    rendered
}

fn normalize_nonexistent_path(path: &Path) -> PathBuf {
    if let Some(parent) = path.parent() {
        if let Ok(parent) = parent.canonicalize() {
            return path
                .file_name()
                .map(|name| parent.join(name))
                .unwrap_or(parent);
        }
    }
    path.to_path_buf()
}

/// Find a tool by name, with alias fallback.
pub(crate) fn find_tool(name: &str, tools: &Tools) -> Option<Arc<dyn Tool>> {
    // Primary: exact name match
    if let Some(tool) = tools.iter().find(|t| t.name() == name) {
        return Some(Arc::clone(tool));
    }

    // Fallback: check user_facing_name
    if let Some(tool) = tools.iter().find(|t| t.user_facing_name(None) == name) {
        return Some(Arc::clone(tool));
    }

    None
}

/// Enforce tool result size limit by truncating if necessary.
pub(crate) fn enforce_result_size(data: Value, max_chars: usize) -> Value {
    match &data {
        Value::String(s) if s.len() > max_chars => {
            let head = &s[..max_chars / 2];
            let tail = &s[s.len() - max_chars / 4..];
            let omitted = s.len() - head.len() - tail.len();
            Value::String(format!(
                "{}\n\n[... {} characters omitted ...]\n\n{}",
                head, omitted, tail
            ))
        }
        _ => data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_enforce_result_size_non_string() {
        let data = json!({"key": "value"});
        let result = enforce_result_size(data.clone(), 10);
        assert_eq!(result, data);
    }

    #[test]
    fn test_enforce_result_size_short_string() {
        let data = json!("short text");
        let result = enforce_result_size(data.clone(), 1000);
        assert_eq!(result, data);
    }

    #[test]
    fn test_enforce_result_size_long_string() {
        let long = "x".repeat(10_000);
        let data = json!(long);
        let result = enforce_result_size(data, 1000);
        let s = result.as_str().unwrap();
        assert!(s.contains("characters omitted"));
        assert!(s.len() < 10_000);
    }

    #[test]
    fn test_enforce_result_size_exact_boundary() {
        let exact = "a".repeat(1000);
        let data = json!(exact);
        let result = enforce_result_size(data.clone(), 1000);
        // Exactly at limit should NOT be truncated (the condition is >)
        assert_eq!(result.as_str().unwrap().len(), 1000);
    }

    #[test]
    fn test_enforce_result_size_null() {
        let data = json!(null);
        let result = enforce_result_size(data.clone(), 100);
        assert_eq!(result, data);
    }

    #[test]
    fn test_enforce_result_size_array() {
        let data = json!([1, 2, 3]);
        let result = enforce_result_size(data.clone(), 5);
        assert_eq!(result, data);
    }

    #[test]
    fn test_enforce_result_size_truncation_structure() {
        // Verify head/tail sizes match the documented formula:
        // head = max_chars / 2, tail = max_chars / 4
        let max = 1000usize;
        let long = "y".repeat(max * 3);
        let data = json!(long);
        let result = enforce_result_size(data, max);
        let s = result.as_str().unwrap();

        // head portion: max/2 = 500 'y's
        let head_part: String = s.chars().take(max / 2).collect();
        assert_eq!(head_part, "y".repeat(max / 2));

        // tail portion after the omission marker: max/4 = 250 'y's
        let tail_part: String = s
            .chars()
            .rev()
            .take(max / 4)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        assert_eq!(tail_part, "y".repeat(max / 4));

        // omission message present
        assert!(s.contains(&format!(
            "{} characters omitted",
            long.len() - max / 2 - max / 4
        )));
    }

    #[test]
    fn test_enforce_result_size_number() {
        let data = json!(42);
        let result = enforce_result_size(data.clone(), 1);
        assert_eq!(result, data);
    }

    #[test]
    fn test_enforce_result_size_bool() {
        let data = json!(true);
        let result = enforce_result_size(data.clone(), 1);
        assert_eq!(result, data);
    }
}
