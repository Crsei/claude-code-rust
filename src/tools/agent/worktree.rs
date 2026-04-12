//! Git worktree execution for agent isolation.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::types::config::QuerySource;
use crate::types::tool::*;

use super::{
    build_child_config, collect_stream_result, count_worktree_changes, find_git_root,
    get_head_sha, AgentInput, AgentTool,
};
use crate::engine::lifecycle::QueryEngine;

impl AgentTool {
    /// Run the agent inside an isolated git worktree.
    ///
    /// Creates a temporary worktree + branch, points the child QueryEngine's
    /// cwd at it, runs the agent, and then cleans up if no changes were made.
    pub(super) async fn run_in_worktree(
        &self,
        params: &AgentInput,
        ctx: &ToolUseContext,
        agent_id: &str,
        agent_model: &str,
        parent_model: &str,
        current_depth: usize,
    ) -> Result<ToolResult> {
        let cwd = std::env::current_dir()?;

        // -- 1. Find git root
        let git_root = match find_git_root(&cwd).await {
            Ok(root) => root,
            Err(e) => {
                warn!(
                    agent_id = %agent_id,
                    error = %e,
                    "worktree isolation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — {}]\n\n{}",
                                e, s
                            ));
                        }
                        r
                    });
            }
        };

        let original_head = get_head_sha(&git_root).await;

        // -- 2. Create branch + worktree
        let short_id = &Uuid::new_v4().to_string()[..8];
        let branch_name = format!("agent-worktree-{}", short_id);
        let worktree_path = std::env::temp_dir().join(format!("agent-worktree-{}", short_id));

        info!(
            agent_id = %agent_id,
            worktree_path = %worktree_path.display(),
            branch = %branch_name,
            "creating agent worktree"
        );

        let wt_output = tokio::process::Command::new("git")
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
            .await;

        match wt_output {
            Ok(ref o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                warn!(
                    agent_id = %agent_id,
                    error = %stderr,
                    "worktree creation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — git worktree add failed: {}]\n\n{}",
                                stderr.trim(), s
                            ));
                        }
                        r
                    });
            }
            Err(e) => {
                warn!(
                    agent_id = %agent_id,
                    error = %e,
                    "worktree creation failed — falling back to normal execution"
                );
                return self
                    .run_agent_normal(
                        params,
                        ctx,
                        agent_id,
                        agent_model,
                        parent_model,
                        current_depth,
                    )
                    .await
                    .map(|mut r| {
                        if let Some(s) = r.data.as_str() {
                            r.data = json!(format!(
                                "[WARNING: worktree isolation skipped — {}]\n\n{}",
                                e, s
                            ));
                        }
                        r
                    });
            }
            Ok(_) => {
                debug!(
                    agent_id = %agent_id,
                    worktree_path = %worktree_path.display(),
                    "worktree created successfully"
                );
            }
        }

        // -- 3. Run the agent with cwd = worktree
        let child_config = build_child_config(
            worktree_path.to_string_lossy().to_string(),
            ctx,
            agent_id,
            agent_model,
            parent_model,
            current_depth,
        );

        let child_engine = QueryEngine::new(child_config);
        let stream =
            child_engine.submit_message(&params.prompt, QuerySource::Agent(agent_id.to_string()));

        let (mut result_text, had_error) = collect_stream_result(stream).await;

        // -- 4. Check for changes
        let changes = count_worktree_changes(&worktree_path, original_head.as_deref()).await;

        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true, // fail-closed: assume changes if we can't tell
        };

        if has_changes {
            // Keep the worktree — include location info in result
            let (files, commits) = changes.unwrap_or((0, 0));
            info!(
                agent_id = %agent_id,
                worktree_path = %worktree_path.display(),
                branch = %branch_name,
                changed_files = files,
                new_commits = commits,
                "agent worktree has changes — keeping"
            );

            result_text.push_str(&format!(
                "\n\n[Worktree isolation: changes detected ({} file(s), {} commit(s)). \
                 Worktree kept at: {} on branch: {}]",
                files,
                commits,
                worktree_path.display(),
                branch_name,
            ));
        } else {
            // No changes — clean up worktree + branch
            info!(
                agent_id = %agent_id,
                worktree_path = %worktree_path.display(),
                branch = %branch_name,
                "agent worktree has no changes — cleaning up"
            );

            Self::cleanup_worktree(&git_root, &worktree_path, &branch_name, agent_id).await;

            result_text
                .push_str("\n\n[Worktree isolation: no changes detected — worktree cleaned up]");
        }

        debug!(
            agent_id = %agent_id,
            result_len = result_text.len(),
            error = had_error,
            "subagent (worktree) completed"
        );

        Ok(ToolResult {
            data: json!(result_text),
            new_messages: vec![],
        })
    }

    /// Remove a worktree and its branch after agent completes with no changes.
    async fn cleanup_worktree(
        git_root: &PathBuf,
        worktree_path: &PathBuf,
        branch_name: &str,
        agent_id: &str,
    ) {
        let remove_result = tokio::process::Command::new("git")
            .args([
                "-C",
                &git_root.to_string_lossy(),
                "worktree",
                "remove",
                "--force",
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .await;

        match remove_result {
            Ok(o) if o.status.success() => {
                debug!(agent_id = %agent_id, "agent worktree directory removed");
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                warn!(
                    agent_id = %agent_id,
                    "worktree remove warning: {}", stderr
                );
            }
            Err(e) => {
                warn!(agent_id = %agent_id, "worktree remove failed: {}", e);
            }
        }

        // Brief pause to let git release locks before branch delete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let branch_result = tokio::process::Command::new("git")
            .args([
                "-C",
                &git_root.to_string_lossy(),
                "branch",
                "-D",
                branch_name,
            ])
            .output()
            .await;

        match branch_result {
            Ok(o) if o.status.success() => {
                debug!(agent_id = %agent_id, "agent worktree branch deleted");
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                warn!(
                    agent_id = %agent_id,
                    "branch delete warning: {}", stderr
                );
            }
            Err(e) => {
                warn!(agent_id = %agent_id, "branch delete failed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // Branch and path naming patterns
    // -----------------------------------------------------------------------

    /// Branch names must follow the `agent-worktree-{8-char-id}` pattern.
    #[test]
    fn test_branch_name_format() {
        let short_id = "abcd1234";
        let branch_name = format!("agent-worktree-{}", short_id);
        assert!(branch_name.starts_with("agent-worktree-"));
        assert_eq!(branch_name, "agent-worktree-abcd1234");
    }

    /// Worktree path must be under temp dir with matching suffix.
    #[test]
    fn test_worktree_path_format() {
        let short_id = "abcd1234";
        let worktree_path = std::env::temp_dir().join(format!("agent-worktree-{}", short_id));
        let path_str = worktree_path.to_string_lossy();
        assert!(path_str.contains("agent-worktree-abcd1234"));
    }

    /// Branch name and worktree path suffix must be consistent (same short_id).
    #[test]
    fn test_branch_name_and_path_share_same_id() {
        let short_id = "deadbeef";
        let branch_name = format!("agent-worktree-{}", short_id);
        let worktree_path = std::env::temp_dir().join(format!("agent-worktree-{}", short_id));
        let path_tail = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        assert_eq!(branch_name, path_tail);
    }

    // -----------------------------------------------------------------------
    // Result text suffix — has_changes path
    // -----------------------------------------------------------------------

    #[test]
    fn test_result_text_suffix_with_changes() {
        let files: usize = 3;
        let commits: usize = 1;
        let worktree_path = PathBuf::from("/tmp/agent-worktree-abcd1234");
        let branch_name = "agent-worktree-abcd1234";

        let suffix = format!(
            "\n\n[Worktree isolation: changes detected ({} file(s), {} commit(s)). \
             Worktree kept at: {} on branch: {}]",
            files,
            commits,
            worktree_path.display(),
            branch_name,
        );

        assert!(suffix.contains("changes detected"));
        assert!(suffix.contains("3 file(s)"));
        assert!(suffix.contains("1 commit(s)"));
        assert!(suffix.contains("Worktree kept at"));
        assert!(suffix.contains("agent-worktree-abcd1234"));
    }

    #[test]
    fn test_result_text_suffix_no_changes() {
        let suffix = "\n\n[Worktree isolation: no changes detected — worktree cleaned up]";
        assert!(suffix.contains("no changes detected"));
        assert!(suffix.contains("cleaned up"));
    }

    // -----------------------------------------------------------------------
    // Result text suffix — fallback warning message formats
    // -----------------------------------------------------------------------

    #[test]
    fn test_fallback_warning_format_git_root_error() {
        let error_msg = "Not a git repository";
        let base = "some result text";
        let result = format!(
            "[WARNING: worktree isolation skipped — {}]\n\n{}",
            error_msg, base
        );
        assert!(result.starts_with("[WARNING: worktree isolation skipped —"));
        assert!(result.contains("Not a git repository"));
        assert!(result.ends_with("some result text"));
    }

    #[test]
    fn test_fallback_warning_format_worktree_add_failed() {
        let stderr = "fatal: branch already exists";
        let base = "normal result";
        let result = format!(
            "[WARNING: worktree isolation skipped — git worktree add failed: {}]\n\n{}",
            stderr.trim(),
            base
        );
        assert!(result.contains("git worktree add failed"));
        assert!(result.contains("fatal: branch already exists"));
        assert!(result.ends_with("normal result"));
    }

    // -----------------------------------------------------------------------
    // has_changes logic — fail-closed when count_worktree_changes returns None
    // -----------------------------------------------------------------------

    #[test]
    fn test_has_changes_true_when_none() {
        let changes: Option<(usize, usize)> = None;
        // fail-closed: if we can't tell, assume changes
        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true,
        };
        assert!(has_changes, "None result should be treated as has_changes=true");
    }

    #[test]
    fn test_has_changes_false_when_zero_files_and_commits() {
        let changes: Option<(usize, usize)> = Some((0, 0));
        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true,
        };
        assert!(!has_changes);
    }

    #[test]
    fn test_has_changes_true_when_files_changed() {
        let changes: Option<(usize, usize)> = Some((5, 0));
        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true,
        };
        assert!(has_changes);
    }

    #[test]
    fn test_has_changes_true_when_only_commits() {
        let changes: Option<(usize, usize)> = Some((0, 2));
        let has_changes = match changes {
            Some((files, commits)) => files > 0 || commits > 0,
            None => true,
        };
        assert!(has_changes);
    }
}
