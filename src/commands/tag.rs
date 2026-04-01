//! /tag command -- tag the current session with a label.
//!
//! Tags are simple string labels associated with the session. They are stored
//! in memory on the AppState (the `extra` field on settings or a dedicated
//! list). If no arguments are given, the current tags are displayed.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// In-process session tags. These persist for the lifetime of the session.
/// A production implementation would persist these alongside session storage.
static TAGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Handler for the `/tag` slash command.
pub struct TagHandler;

#[async_trait]
impl CommandHandler for TagHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let tag = args.trim();

        if tag.is_empty() {
            return show_tags();
        }

        add_tag(tag)
    }
}

/// Show current tags.
fn show_tags() -> Result<CommandResult> {
    let tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
    if tags.is_empty() {
        Ok(CommandResult::Output(
            "No tags set for this session.\n\
             Usage: /tag <label> -- add a tag to the current session"
                .to_string(),
        ))
    } else {
        let mut lines = Vec::new();
        lines.push(format!("Session tags ({}):", tags.len()));
        for tag in tags.iter() {
            lines.push(format!("  - {}", tag));
        }
        Ok(CommandResult::Output(lines.join("\n")))
    }
}

/// Add a tag to the session.
fn add_tag(tag: &str) -> Result<CommandResult> {
    let mut tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());

    if tags.iter().any(|t| t == tag) {
        return Ok(CommandResult::Output(format!(
            "Tag '{}' already exists on this session.",
            tag
        )));
    }

    tags.push(tag.to_string());
    Ok(CommandResult::Output(format!(
        "Tag '{}' added. Session now has {} tag(s).",
        tag,
        tags.len()
    )))
}

/// Clear all tags (used by tests).
#[cfg(test)]
fn clear_tags() {
    let mut tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
    tags.clear();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test/project"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_tag_no_args_shows_empty() {
        clear_tags();
        let handler = TagHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No tags"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_tag_add_and_show() {
        clear_tags();
        let handler = TagHandler;
        let mut ctx = test_ctx();

        let result = handler.execute("important", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("important"));
                assert!(text.contains("added"));
            }
            _ => panic!("Expected Output result"),
        }

        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("important"));
            }
            _ => panic!("Expected Output result"),
        }
        clear_tags();
    }

    #[tokio::test]
    async fn test_tag_duplicate() {
        clear_tags();
        let handler = TagHandler;
        let mut ctx = test_ctx();

        handler.execute("dup-test", &mut ctx).await.unwrap();
        let result = handler.execute("dup-test", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("already exists"));
            }
            _ => panic!("Expected Output result"),
        }
        clear_tags();
    }
}
