//! CLAUDE.md loading and context building.
//!
//! Searches for `CLAUDE.md` files in the working directory and its ancestors,
//! loads their contents, and assembles them into a single context string that
//! is injected into the system prompt.

#![allow(unused)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Search for `CLAUDE.md` files starting at `cwd` and walking up to the
/// filesystem root.
///
/// Returns paths ordered from the **most distant ancestor** first (root-most)
/// to the working directory last, mirroring how context should be layered
/// (general first, specific last).
pub fn find_claude_md_files(cwd: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dir = cwd.to_path_buf();

    loop {
        let candidate = dir.join("CLAUDE.md");
        if candidate.is_file() {
            found.push(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    // Reverse so root-most files come first, project-level files last.
    found.reverse();
    found
}

/// Read the contents of a single `CLAUDE.md` file.
pub fn load_claude_md(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read CLAUDE.md at {}", path.display()))
}

/// Find all `CLAUDE.md` files from `cwd` upwards and combine their contents
/// into a single context string.
///
/// Each file's content is prefixed with a header indicating its source path.
/// Files are concatenated in order from root-most to project-level so that
/// more specific instructions appear last and can override general ones.
///
/// Returns `Ok(String)` with the combined context, or an empty string if no
/// `CLAUDE.md` files were found.
pub fn build_claude_md_context(cwd: &Path) -> Result<String> {
    let files = find_claude_md_files(cwd);

    if files.is_empty() {
        return Ok(String::new());
    }

    let mut sections: Vec<String> = Vec::with_capacity(files.len());

    for path in &files {
        let content = load_claude_md(path)?;
        if content.trim().is_empty() {
            continue;
        }

        let display_path = path.display();
        sections.push(format!(
            "Contents of {} (project instructions, checked into the codebase):\n\n{}",
            display_path,
            content.trim()
        ));
    }

    Ok(sections.join("\n\n---\n\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Generate a unique temp directory to avoid collisions between test runs.
    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let id = uuid::Uuid::new_v4().to_string();
        std::env::temp_dir().join(format!("{}_{}", prefix, &id[..8]))
    }

    #[test]
    fn test_find_claude_md_empty_dir() {
        let dir = unique_temp_dir("claude_md_empty");
        let _ = fs::create_dir_all(&dir);
        let result = find_claude_md_files(&dir);
        // May or may not find files depending on the temp dir hierarchy,
        // but should not panic.
        let _ = result;
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_context_no_files() {
        let dir = unique_temp_dir("claude_md_nofiles");
        let _ = fs::create_dir_all(&dir);
        let ctx = build_claude_md_context(&dir).unwrap();
        // Context from the working dir itself; if there's no CLAUDE.md, section is empty.
        // We just verify it doesn't error.
        let _ = ctx;
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_context_with_file() {
        let dir = unique_temp_dir("claude_md_withfile");
        fs::create_dir_all(&dir).unwrap();
        let md_path = dir.join("CLAUDE.md");
        fs::write(&md_path, "# Test instructions\nDo the thing.").unwrap();

        let ctx = build_claude_md_context(&dir).unwrap();
        assert!(ctx.contains("Test instructions"));
        assert!(ctx.contains("Do the thing."));

        let _ = fs::remove_dir_all(&dir);
    }
}
