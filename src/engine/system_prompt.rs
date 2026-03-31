//! System prompt construction.
//!
//! Corresponds to TypeScript: `fetchSystemPromptParts()` + assembly in
//! `submitMessage()` (QueryEngine.ts).

#![allow(unused)]

use std::collections::HashMap;
use std::sync::Arc;

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
/// 3. `user_context` always contains `cwd`, `date`, and `platform`.
/// 4. `system_context` is currently empty (reserved for future use).
/// 5. If `append_prompt` is `Some`, it is added as an additional part at
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

    // Append prompt (added regardless of custom/default).
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

        assert_eq!(parts.len(), 1);
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

        assert!(parts.len() >= 2);
        assert_eq!(parts.last().unwrap(), "Always be concise.");
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

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "Custom base.");
        assert_eq!(parts[1], "Appended.");
    }
}
