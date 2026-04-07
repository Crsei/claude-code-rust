//! Worktree tools — EnterWorktree / ExitWorktree.
//!
//! Corresponds to TypeScript:
//!   src/tools/EnterWorktreeTool/EnterWorktreeTool.ts
//!   src/tools/ExitWorktreeTool/ExitWorktreeTool.ts
//!   src/utils/worktree.ts
//!
//! Creates an isolated git worktree for the agent to make changes in without
//! affecting the main working tree.  On exit the worktree can be kept
//! (branch + directory remain) or removed (cleaned up).
//!
//! Safety invariants:
//! - Cannot nest: only one worktree session at a time
//! - Fail-closed: if git status cannot be determined, refuse to remove
//! - Change detection: counts uncommitted files + new commits before removal
//! - Requires explicit `discard_changes: true` to remove with unsaved work

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// ---------------------------------------------------------------------------
// Worktree session state (process-global, single session)
// ---------------------------------------------------------------------------

/// Tracks the current worktree session.
#[derive(Debug, Clone)]
pub struct WorktreeSession {
    /// The path to the worktree directory.
    pub worktree_path: PathBuf,
    /// The branch name created for this worktree.
    pub branch_name: String,
    /// The original working directory before entering the worktree.
    pub original_cwd: PathBuf,
    /// The HEAD commit SHA when the worktree was created.
    pub original_head_commit: Option<String>,
}

static CURRENT_SESSION: LazyLock<Mutex<Option<WorktreeSession>>> =
    LazyLock::new(|| Mutex::new(None));

/// Get the current worktree session (if any).
pub fn get_current_worktree_session() -> Option<WorktreeSession> {
    CURRENT_SESSION.lock().ok()?.clone()
}

/// Set the current worktree session.
fn set_worktree_session(session: Option<WorktreeSession>) {
    if let Ok(mut s) = CURRENT_SESSION.lock() {
        *s = session;
    }
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

/// Count uncommitted file changes and new commits in a worktree.
async fn count_worktree_changes(
    worktree_path: &Path,
    original_head: Option<&str>,
) -> Option<(usize, usize)> {
    let status = tokio::process::Command::new("git")
        .args(["-C", &worktree_path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .await
        .ok()?;

    if !status.status.success() {
        return None;
    }

    let changed_files = String::from_utf8_lossy(&status.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();

    let Some(orig_head) = original_head else {
        return Some((changed_files, 0));
    };

    let rev_list = tokio::process::Command::new("git")
        .args([
            "-C",
            &worktree_path.to_string_lossy(),
            "rev-list",
            "--count",
            &format!("{}..HEAD", orig_head),
        ])
        .output()
        .await
        .ok()?;

    if !rev_list.status.success() {
        return None;
    }

    let commits = String::from_utf8_lossy(&rev_list.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0);

    Some((changed_files, commits))
}

/// Get the current HEAD sha of the repository at `cwd`.
async fn get_head_sha(cwd: &Path) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Find the canonical git root from a path.
async fn find_git_root(cwd: &Path) -> Option<PathBuf> {
    let output = tokio::process::Command::new("git")
        .args([
            "-C",
            &cwd.to_string_lossy(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(root))
    } else {
        None
    }
}

/// Validate a slug for path safety.
fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        bail!("Worktree name cannot be empty");
    }
    if slug.contains("..") || slug.contains('/') || slug.contains('\\') {
        bail!("Worktree name cannot contain path separators or '..'");
    }
    if slug.len() > 64 {
        bail!("Worktree name too long (max 64 chars)");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// EnterWorktree
// ---------------------------------------------------------------------------

/// EnterWorktree — create a temporary git worktree for isolated changes.
pub struct EnterWorktreeTool;

#[derive(Deserialize)]
struct EnterWorktreeInput {
    /// Optional name/slug for the worktree and branch.
    name: Option<String>,
}

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        "EnterWorktree"
    }

    async fn description(&self, _input: &Value) -> String {
        "Create a temporary git worktree for isolated changes.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Optional name for the worktree (used as branch suffix)"
                }
            },
            "additionalProperties": false
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false // modifies global cwd state
    }

    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        if get_current_worktree_session().is_some() {
            return ValidationResult::Error {
                message: "Already in a worktree session. Exit the current one first.".to_string(),
                error_code: 1,
            };
        }
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: EnterWorktreeInput = serde_json::from_value(input)?;

        let slug = params.name.unwrap_or_else(|| {
            uuid::Uuid::new_v4().to_string()[..8].to_string()
        });
        validate_slug(&slug)?;

        let cwd = std::env::current_dir()?;
        let git_root = find_git_root(&cwd).await.unwrap_or_else(|| cwd.clone());
        let original_head = get_head_sha(&git_root).await;

        let worktree_path = std::env::temp_dir().join(format!("cc-worktree-{}", slug));
        let branch_name = format!("cc-worktree-{}", slug);

        info!(
            worktree_path = %worktree_path.display(),
            branch = %branch_name,
            "creating git worktree"
        );

        let output = tokio::process::Command::new("git")
            .args([
                "-C",
                &git_root.to_string_lossy(),
                "worktree",
                "add",
                "-B",
                &branch_name,
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create worktree: {}", stderr);
        }

        set_worktree_session(Some(WorktreeSession {
            worktree_path: worktree_path.clone(),
            branch_name: branch_name.clone(),
            original_cwd: cwd,
            original_head_commit: original_head,
        }));

        info!(
            worktree_path = %worktree_path.display(),
            branch = %branch_name,
            "worktree created successfully"
        );

        Ok(ToolResult {
            data: json!({
                "worktree_path": worktree_path.display().to_string(),
                "branch": branch_name,
                "message": format!(
                    "Created worktree at {} on branch {}. \
                     Changes made here are isolated from the main working tree. \
                     Use ExitWorktree to keep or remove when done.",
                    worktree_path.display(),
                    branch_name,
                ),
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        concat!(
            "Create an isolated git worktree to make changes without affecting ",
            "the main working tree. Optionally provide a name for the worktree. ",
            "Use ExitWorktree with action 'keep' or 'remove' when done.",
        )
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// ExitWorktree
// ---------------------------------------------------------------------------

/// ExitWorktree — leave and optionally clean up a git worktree.
pub struct ExitWorktreeTool;

#[derive(Deserialize)]
struct ExitWorktreeInput {
    /// "keep" to leave worktree intact, "remove" to delete it.
    action: String,
    /// If true, force removal even with uncommitted changes.
    /// Checked in `validate_input` via raw JSON; kept here for schema completeness.
    #[serde(default)]
    #[allow(dead_code)]
    discard_changes: bool,
}

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        "ExitWorktree"
    }

    async fn description(&self, _input: &Value) -> String {
        "Leave and clean up a git worktree.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["keep", "remove"],
                    "description": "'keep' to preserve the worktree, 'remove' to delete it"
                },
                "discard_changes": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, force removal even with uncommitted changes"
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let session = get_current_worktree_session();
        if session.is_none() {
            return ValidationResult::Error {
                message: "No active worktree session to exit.".to_string(),
                error_code: 1,
            };
        }

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if action != "keep" && action != "remove" {
            return ValidationResult::Error {
                message: "action must be 'keep' or 'remove'.".to_string(),
                error_code: 3,
            };
        }

        // If removing, check for changes (fail-closed safety)
        let discard = input
            .get("discard_changes")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if action == "remove" && !discard {
            let session = session.expect("session guaranteed Some after is_none check");
            let changes = count_worktree_changes(
                &session.worktree_path,
                session.original_head_commit.as_deref(),
            )
            .await;

            match changes {
                None => {
                    return ValidationResult::Error {
                        message: concat!(
                            "Could not verify worktree state. ",
                            "Re-invoke with discard_changes: true to force removal, ",
                            "or use action: 'keep' to preserve the worktree.",
                        )
                        .to_string(),
                        error_code: 4,
                    };
                }
                Some((changed_files, commits)) if changed_files > 0 || commits > 0 => {
                    let mut parts = Vec::new();
                    if changed_files > 0 {
                        parts.push(format!("{} uncommitted file change(s)", changed_files));
                    }
                    if commits > 0 {
                        parts.push(format!("{} new commit(s)", commits));
                    }
                    return ValidationResult::Error {
                        message: format!(
                            "Worktree has {}. Removing will discard this work permanently. \
                             Confirm with the user, then re-invoke with discard_changes: true, \
                             or use action: 'keep' to preserve the worktree.",
                            parts.join(" and "),
                        ),
                        error_code: 2,
                    };
                }
                _ => {} // No changes — safe to remove
            }
        }

        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: ExitWorktreeInput = serde_json::from_value(input)?;

        let session = get_current_worktree_session()
            .ok_or_else(|| anyhow::anyhow!("No active worktree session"))?;

        let worktree_path = session.worktree_path.clone();
        let branch_name = session.branch_name.clone();
        let original_cwd = session.original_cwd.clone();

        match params.action.as_str() {
            "keep" => {
                info!(
                    worktree_path = %worktree_path.display(),
                    branch = %branch_name,
                    "keeping worktree"
                );

                set_worktree_session(None);

                Ok(ToolResult {
                    data: json!({
                        "action": "keep",
                        "worktree_path": worktree_path.display().to_string(),
                        "branch": branch_name,
                        "message": format!(
                            "Worktree kept at {} on branch {}. \
                             You can return to it later or merge the branch.",
                            worktree_path.display(),
                            branch_name,
                        ),
                    }),
                    new_messages: vec![],
                })
            }
            "remove" => {
                info!(
                    worktree_path = %worktree_path.display(),
                    branch = %branch_name,
                    "removing worktree"
                );

                let remove_result = tokio::process::Command::new("git")
                    .args([
                        "-C",
                        &original_cwd.to_string_lossy(),
                        "worktree",
                        "remove",
                        "--force",
                        &worktree_path.to_string_lossy(),
                    ])
                    .output()
                    .await;

                let mut warnings = Vec::new();

                match remove_result {
                    Ok(o) if o.status.success() => {
                        debug!("worktree directory removed");
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        warn!("git worktree remove warning: {}", stderr);
                        warnings.push(format!("worktree remove warning: {}", stderr.trim()));
                    }
                    Err(e) => {
                        warn!("git worktree remove failed: {}", e);
                        warnings.push(format!("worktree remove failed: {}", e));
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let branch_result = tokio::process::Command::new("git")
                    .args([
                        "-C",
                        &original_cwd.to_string_lossy(),
                        "branch",
                        "-D",
                        &branch_name,
                    ])
                    .output()
                    .await;

                match branch_result {
                    Ok(o) if o.status.success() => {
                        debug!("worktree branch deleted");
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        warn!("branch delete warning: {}", stderr);
                        warnings.push(format!("branch delete warning: {}", stderr.trim()));
                    }
                    Err(e) => {
                        warn!("branch delete failed: {}", e);
                        warnings.push(format!("branch delete failed: {}", e));
                    }
                }

                set_worktree_session(None);

                let mut result = json!({
                    "action": "remove",
                    "message": "Worktree removed and branch deleted.",
                });
                if !warnings.is_empty() {
                    result["warnings"] = json!(warnings);
                }

                Ok(ToolResult {
                    data: result,
                    new_messages: vec![],
                })
            }
            other => bail!("Unknown action: {}. Use 'keep' or 'remove'.", other),
        }
    }

    async fn prompt(&self) -> String {
        concat!(
            "Exit an active git worktree session. Use action 'keep' to preserve ",
            "the worktree and branch, or 'remove' to clean them up. ",
            "If removing with unsaved changes, you must set discard_changes: true.",
        )
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use crate::types::app_state::AppState;

    fn make_ctx() -> ToolUseContext {
        let state = Arc::new(RwLock::new(AppState::default()));
        let state_r = Arc::clone(&state);
        let state_w = Arc::clone(&state);

        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".to_string(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || state_r.read().unwrap().clone()),
            set_app_state: Arc::new(move |f: Box<dyn FnOnce(AppState) -> AppState>| {
                let mut s = state_w.write().unwrap();
                let old = s.clone();
                *s = f(old);
            }),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        }
    }

    #[test]
    fn test_enter_worktree_name() {
        let tool = EnterWorktreeTool;
        assert_eq!(tool.name(), "EnterWorktree");
    }

    #[test]
    fn test_exit_worktree_name() {
        let tool = ExitWorktreeTool;
        assert_eq!(tool.name(), "ExitWorktree");
    }

    #[test]
    fn test_validate_slug() {
        assert!(validate_slug("my-feature").is_ok());
        assert!(validate_slug("fix_123").is_ok());
        assert!(validate_slug("").is_err());
        assert!(validate_slug("../escape").is_err());
        assert!(validate_slug("path/traversal").is_err());
        assert!(validate_slug("back\\slash").is_err());
        assert!(validate_slug(&"x".repeat(65)).is_err());
    }

    #[test]
    fn test_enter_worktree_schema() {
        let tool = EnterWorktreeTool;
        let schema = tool.input_json_schema();
        assert!(schema["properties"].get("name").is_some());
    }

    #[test]
    fn test_exit_worktree_schema() {
        let tool = ExitWorktreeTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("action"));
        assert!(props.contains_key("discard_changes"));
    }

    #[tokio::test]
    async fn test_exit_worktree_no_session() {
        set_worktree_session(None);

        let tool = ExitWorktreeTool;
        let ctx = make_ctx();
        let result = tool.validate_input(&json!({"action": "keep"}), &ctx).await;
        assert!(matches!(result, ValidationResult::Error { error_code: 1, .. }));
    }

    #[tokio::test]
    async fn test_exit_worktree_invalid_action() {
        set_worktree_session(Some(WorktreeSession {
            worktree_path: PathBuf::from("/tmp/test"),
            branch_name: "test-branch".to_string(),
            original_cwd: PathBuf::from("/tmp"),
            original_head_commit: None,
        }));

        let tool = ExitWorktreeTool;
        let ctx = make_ctx();
        let result = tool
            .validate_input(&json!({"action": "invalid"}), &ctx)
            .await;
        assert!(matches!(result, ValidationResult::Error { error_code: 3, .. }));

        set_worktree_session(None);
    }

    #[test]
    fn test_worktree_session_lifecycle() {
        set_worktree_session(None);
        assert!(get_current_worktree_session().is_none());

        let session = WorktreeSession {
            worktree_path: PathBuf::from("/tmp/wt"),
            branch_name: "wt-branch".to_string(),
            original_cwd: PathBuf::from("/project"),
            original_head_commit: Some("abc123".to_string()),
        };
        set_worktree_session(Some(session));
        let current = get_current_worktree_session().unwrap();
        assert_eq!(current.branch_name, "wt-branch");
        assert_eq!(current.original_head_commit.as_deref(), Some("abc123"));

        set_worktree_session(None);
        assert!(get_current_worktree_session().is_none());
    }

    #[tokio::test]
    async fn test_enter_worktree_blocks_nesting() {
        set_worktree_session(Some(WorktreeSession {
            worktree_path: PathBuf::from("/tmp/existing"),
            branch_name: "existing".to_string(),
            original_cwd: PathBuf::from("/tmp"),
            original_head_commit: None,
        }));

        let tool = EnterWorktreeTool;
        let ctx = make_ctx();
        let result = tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(result, ValidationResult::Error { error_code: 1, .. }));

        set_worktree_session(None);
    }
}
