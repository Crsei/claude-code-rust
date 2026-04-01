//! System prompt construction.
//!
//! Corresponds to TypeScript: `fetchSystemPromptParts()` + assembly in
//! `submitMessage()` (QueryEngine.ts).
//!
//! Builds the system prompt by combining:
//! 1. Default or custom base prompt
//! 2. Tool descriptions
//! 3. CLAUDE.md context (project instructions)
//! 4. User/system context metadata

#![allow(unused)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tracing::debug;

use crate::config::claude_md;
use crate::types::tool::Tool;

// ---------------------------------------------------------------------------
// build_system_prompt
// ---------------------------------------------------------------------------

/// Build the full system prompt from configuration parts.
///
/// Returns `(system_prompt_parts, user_context, system_context)`.
///
/// # Behaviour
///
/// 1. If `custom_prompt` is `Some`, it is used as the *only* system prompt
///    part (tool descriptions are still appended below it via
///    `user_context`).
/// 2. Otherwise the default prompt is built, including a header that
///    identifies the assistant and descriptions for every enabled tool.
/// 3. CLAUDE.md files from `cwd` upwards are loaded and injected.
/// 4. `user_context` always contains `cwd`, `date`, and `platform`.
/// 5. `system_context` is currently empty (reserved for future use).
/// 6. If `append_prompt` is `Some`, it is added as an additional part at
///    the end.
pub fn build_system_prompt(
    custom_prompt: Option<&str>,
    append_prompt: Option<&str>,
    tools: &[Arc<dyn Tool>],
    model: &str,
    cwd: &str,
) -> (Vec<String>, HashMap<String, String>, HashMap<String, String>) {
    let mut parts: Vec<String> = Vec::new();

    // -- System prompt parts --------------------------------------------------
    if let Some(custom) = custom_prompt {
        // Custom prompt replaces the default entirely.
        parts.push(custom.to_string());
    } else {
        // Default prompt header.
        parts.push(format!(
            "You are Claude, an AI assistant by Anthropic, operating as part of Claude Code.\n\
             You are helpful, harmless, and honest.\n\
             Model: {model}"
        ));

        // Tool descriptions.
        let enabled_tools: Vec<&Arc<dyn Tool>> =
            tools.iter().filter(|t| t.is_enabled()).collect();

        if !enabled_tools.is_empty() {
            let mut tool_section = String::from("\n# Available tools\n");
            for tool in &enabled_tools {
                tool_section.push_str(&format!("\n## {}\n", tool.name()));
                let schema = tool.input_json_schema();
                tool_section.push_str(&format!(
                    "Input schema: {}\n",
                    serde_json::to_string(&schema).unwrap_or_default()
                ));
            }
            parts.push(tool_section);
        }
    }

    // -- CLAUDE.md context injection ------------------------------------------
    let cwd_path = Path::new(cwd);
    match claude_md::build_claude_md_context(cwd_path) {
        Ok(context) if !context.is_empty() => {
            debug!(
                cwd = cwd,
                context_len = context.len(),
                "injecting CLAUDE.md context into system prompt"
            );
            parts.push(format!(
                "# Project Instructions (CLAUDE.md)\n\n\
                 IMPORTANT: These instructions OVERRIDE any default behavior \
                 and you MUST follow them exactly as written.\n\n\
                 {}",
                context
            ));
        }
        Ok(_) => {
            debug!(cwd = cwd, "no CLAUDE.md files found");
        }
        Err(e) => {
            debug!(
                cwd = cwd,
                error = %e,
                "failed to load CLAUDE.md context, continuing without it"
            );
        }
    }

    // -- Append prompt (added regardless of custom/default) -------------------
    if let Some(append) = append_prompt {
        parts.push(append.to_string());
    }

    // -- User context ---------------------------------------------------------
    let mut user_context = HashMap::new();
    user_context.insert("cwd".to_string(), cwd.to_string());
    user_context.insert(
        "date".to_string(),
        chrono::Utc::now().format("%Y-%m-%d").to_string(),
    );
    user_context.insert("platform".to_string(), std::env::consts::OS.to_string());
    user_context.insert("model".to_string(), model.to_string());

    // -- System context (reserved) --------------------------------------------
    let system_context: HashMap<String, String> = HashMap::new();

    (parts, user_context, system_context)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_prompt() {
        let (parts, user_ctx, sys_ctx) =
            build_system_prompt(None, None, &[], "claude-sonnet-4-20250514", "/tmp");

        assert!(!parts.is_empty());
        assert!(parts[0].contains("Claude"));
        assert!(parts[0].contains("claude-sonnet-4-20250514"));
        assert_eq!(user_ctx.get("cwd").unwrap(), "/tmp");
        assert!(user_ctx.contains_key("date"));
        assert!(user_ctx.contains_key("platform"));
        assert!(sys_ctx.is_empty());
    }

    #[test]
    fn test_custom_prompt_replaces_default() {
        let (parts, _, _) = build_system_prompt(
            Some("You are a coding assistant."),
            None,
            &[],
            "claude-sonnet-4-20250514",
            "/tmp",
        );

        // Custom prompt is the first part; CLAUDE.md may add more
        assert_eq!(parts[0], "You are a coding assistant.");
    }

    #[test]
    fn test_append_prompt() {
        let (parts, _, _) = build_system_prompt(
            None,
            Some("Always be concise."),
            &[],
            "claude-sonnet-4-20250514",
            "/tmp",
        );

        assert!(parts.last().unwrap() == "Always be concise.");
    }

    #[test]
    fn test_custom_plus_append() {
        let (parts, _, _) = build_system_prompt(
            Some("Custom base."),
            Some("Appended."),
            &[],
            "claude-sonnet-4-20250514",
            "/tmp",
        );

        assert_eq!(parts[0], "Custom base.");
        assert_eq!(parts.last().unwrap(), "Appended.");
    }

    #[test]
    fn test_claude_md_injection() {
        let dir = std::env::temp_dir().join(format!(
            "sysprompt_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();

        let md_path = dir.join("CLAUDE.md");
        fs::write(&md_path, "# Project Rules\nAlways use snake_case.").unwrap();

        let cwd = dir.to_str().unwrap();
        let (parts, _, _) = build_system_prompt(None, None, &[], "test-model", cwd);

        // At least one part should contain the CLAUDE.md content
        let has_claude_md = parts.iter().any(|p| {
            p.contains("Project Rules") && p.contains("snake_case")
        });
        assert!(has_claude_md, "CLAUDE.md content should be in system prompt parts");

        // It should also contain the override header
        let has_header = parts.iter().any(|p| p.contains("OVERRIDE"));
        assert!(has_header, "CLAUDE.md section should contain OVERRIDE instruction");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_claude_md_with_custom_prompt() {
        // Even with custom prompt, CLAUDE.md should be injected
        let dir = std::env::temp_dir().join(format!(
            "sysprompt_custom_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();

        let md_path = dir.join("CLAUDE.md");
        fs::write(&md_path, "Use tabs for indentation.").unwrap();

        let cwd = dir.to_str().unwrap();
        let (parts, _, _) =
            build_system_prompt(Some("You are a helper."), None, &[], "test-model", cwd);

        assert_eq!(parts[0], "You are a helper.");
        let has_claude_md = parts.iter().any(|p| p.contains("tabs for indentation"));
        assert!(has_claude_md, "CLAUDE.md should be injected even with custom prompt");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_no_claude_md_no_crash() {
        // A temp dir with no CLAUDE.md should not cause issues
        let dir = std::env::temp_dir().join(format!(
            "sysprompt_nomd_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();

        let cwd = dir.to_str().unwrap();
        let (parts, _, _) = build_system_prompt(None, None, &[], "test-model", cwd);

        // Should have at least the default prompt
        assert!(!parts.is_empty());
        // No part should contain the CLAUDE.md header
        let has_claude_md_header = parts.iter().any(|p| p.contains("Project Instructions"));
        assert!(!has_claude_md_header, "should not have CLAUDE.md section when no file exists");

        let _ = fs::remove_dir_all(&dir);
    }
}
