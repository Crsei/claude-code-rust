use super::*;

// ---------------------------------------------------------------------------

pub(super) fn show_memory(cwd: &Path) -> Result<CommandResult> {
    let context = config_claude_md::build_claude_md_context(cwd)?;

    if context.is_empty() {
        Ok(CommandResult::Output(
            "No CLAUDE.md found in project hierarchy.\n\n\
             Use `/memory edit` to create one."
                .to_string(),
        ))
    } else {
        Ok(CommandResult::Output(format!(
            "**Project Instructions (CLAUDE.md)**\n\n{}",
            context
        )))
    }
}

pub(super) fn show_paths(cwd: &Path) -> Result<CommandResult> {
    let files = config_claude_md::find_claude_md_files(cwd);

    if files.is_empty() {
        Ok(CommandResult::Output(
            "No CLAUDE.md files found.".to_string(),
        ))
    } else {
        let mut lines = vec!["CLAUDE.md files found:".to_string()];
        for path in &files {
            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            lines.push(format!("  {} ({} bytes)", path.display(), size));
        }
        Ok(CommandResult::Output(lines.join("\n")))
    }
}

pub(super) fn edit_memory(cwd: &Path) -> Result<CommandResult> {
    let claude_md_path = cwd.join("CLAUDE.md");
    if !claude_md_path.exists() {
        let template = "# CLAUDE.md\n\n\
            This file provides guidance to Claude Code when working with code in this repository.\n\n\
            ## Project Overview\n\n\
            <!-- Describe your project here -->\n";
        fs::write(&claude_md_path, template)?;
    }
    Ok(CommandResult::Output(format!(
        "CLAUDE.md location: {}\n\nEdit this file to add project instructions.",
        claude_md_path.display()
    )))
}

// ---------------------------------------------------------------------------
// Selector (default entry point 鈥?issue #45)
