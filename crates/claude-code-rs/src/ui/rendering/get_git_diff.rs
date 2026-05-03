//! Git diff helpers shared by TUI surfaces.

use std::path::Path;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitDiffMode {
    Staged,
    Unstaged,
    Combined,
}

pub fn get_git_diff(cwd: impl AsRef<Path>, mode: GitDiffMode) -> Result<String> {
    let repo = git2::Repository::discover(cwd).context("not a git repository")?;
    match mode {
        GitDiffMode::Staged => get_staged_diff(&repo),
        GitDiffMode::Unstaged => get_unstaged_diff(&repo),
        GitDiffMode::Combined => {
            let staged = get_staged_diff(&repo)?;
            let unstaged = get_unstaged_diff(&repo)?;
            Ok(join_diff_sections(&staged, &unstaged))
        }
    }
}

pub fn get_status_summary(cwd: impl AsRef<Path>) -> Result<String> {
    let repo = git2::Repository::discover(cwd).context("not a git repository")?;
    let statuses = repo.statuses(None).context("failed to read git status")?;
    if statuses.is_empty() {
        return Ok("Working tree clean.".to_string());
    }

    let mut lines = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        let marker = if status.contains(git2::Status::INDEX_NEW) {
            "A "
        } else if status.contains(git2::Status::INDEX_MODIFIED) {
            "M "
        } else if status.contains(git2::Status::INDEX_DELETED) {
            "D "
        } else if status.contains(git2::Status::WT_NEW) {
            "??"
        } else if status.contains(git2::Status::WT_MODIFIED) {
            " M"
        } else if status.contains(git2::Status::WT_DELETED) {
            " D"
        } else {
            "  "
        };
        lines.push(format!(
            "{} {}",
            marker,
            entry.path().unwrap_or("(unknown)")
        ));
    }
    Ok(lines.join("\n"))
}

fn get_staged_diff(repo: &git2::Repository) -> Result<String> {
    let head_tree = match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit().context("failed to peel HEAD")?;
            Some(commit.tree().context("failed to read HEAD tree")?)
        }
        Err(_) => None,
    };
    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, None)
        .context("failed to compute staged diff")?;
    diff_to_string(&diff)
}

fn get_unstaged_diff(repo: &git2::Repository) -> Result<String> {
    let diff = repo
        .diff_index_to_workdir(None, None)
        .context("failed to compute unstaged diff")?;
    diff_to_string(&diff)
}

fn diff_to_string(diff: &git2::Diff<'_>) -> Result<String> {
    let mut output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => output.push(line.origin()),
            _ => {}
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            output.push_str(content);
        }
        true
    })
    .context("failed to format git diff")?;
    Ok(output)
}

fn join_diff_sections(staged: &str, unstaged: &str) -> String {
    let mut combined = String::new();
    if !staged.is_empty() {
        combined.push_str("=== Staged changes ===\n\n");
        combined.push_str(staged);
    }
    if !unstaged.is_empty() {
        if !combined.is_empty() {
            combined.push_str("\n\n");
        }
        combined.push_str("=== Unstaged changes ===\n\n");
        combined.push_str(unstaged);
    }
    combined
}
