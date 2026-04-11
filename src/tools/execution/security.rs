//! Security validation for tool execution.

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use crate::permissions::dangerous;
use crate::permissions::path_validation;
use crate::types::tool::{PermissionMode, Tool, Tools};
use crate::types::tool::ToolUseContext;

use super::{make_error_result, ToolExecutionResult};

/// Centralized security checks run before hooks and permission evaluation.
///
/// Returns `Some(ToolExecutionResult)` if the tool call should be **rejected**,
/// or `None` if validation passed and the pipeline should continue.
///
/// Checks performed (in order):
///   3c.1  Plan-mode gate — non-read-only tools are blocked in Plan mode
///   3c.2  Dangerous command detection — Bash/PowerShell commands screened
///   3c.3  Path boundary enforcement — Write/Edit paths must be within allowed dirs
///
/// All checks are skipped when the permission mode is `Bypass`.
pub(super) fn security_validate(
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

    // ── 3c.1: Plan mode gate ───────────────────────────────────────
    if *mode == PermissionMode::Plan && !tool.is_read_only(input) {
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
            if let Some(reason) = dangerous::is_dangerous_command(command) {
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
    if FILE_TOOL_NAMES.contains(&tool_name) {
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
            let cwd = crate::bootstrap::PROCESS_STATE
                .read()
                .original_cwd
                .clone();
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

/// Find a tool by name, with alias fallback.
pub(super) fn find_tool(name: &str, tools: &Tools) -> Option<Arc<dyn Tool>> {
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
pub(super) fn enforce_result_size(data: Value, max_chars: usize) -> Value {
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
