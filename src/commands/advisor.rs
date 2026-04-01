//! `/advisor` command -- configure an advisor model.
//!
//! The advisor model is a secondary model that can be consulted for
//! second opinions, cross-checking, or specialized expertise.
//!
//! Usage:
//!   /advisor            — show current advisor model
//!   /advisor <model>    — set advisor to <model>
//!   /advisor off|unset  — clear advisor model

use std::sync::{LazyLock, Mutex};

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Global advisor model setting.
static ADVISOR_MODEL: LazyLock<Mutex<Option<String>>> =
    LazyLock::new(|| Mutex::new(None));

pub struct AdvisorHandler;

#[async_trait]
impl CommandHandler for AdvisorHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();

        if trimmed.is_empty() {
            return show_current();
        }

        let lower = trimmed.to_lowercase();
        match lower.as_str() {
            "off" | "unset" | "clear" | "none" => clear_advisor(),
            _ => set_advisor(trimmed),
        }
    }
}

/// Show the current advisor model.
fn show_current() -> Result<CommandResult> {
    let guard = ADVISOR_MODEL.lock().unwrap();
    match guard.as_ref() {
        Some(model) => Ok(CommandResult::Output(format!(
            "Current advisor model: {}",
            model
        ))),
        None => Ok(CommandResult::Output(
            "No advisor model is currently set.\n\n\
             Usage: /advisor <model-name> to set an advisor model."
                .to_string(),
        )),
    }
}

/// Set the advisor model.
fn set_advisor(model: &str) -> Result<CommandResult> {
    let mut guard = ADVISOR_MODEL.lock().unwrap();
    let previous = guard.replace(model.to_string());

    let msg = match previous {
        Some(prev) => format!(
            "Advisor model changed from '{}' to '{}'.",
            prev, model
        ),
        None => format!("Advisor model set to '{}'.", model),
    };

    Ok(CommandResult::Output(msg))
}

/// Clear the advisor model.
fn clear_advisor() -> Result<CommandResult> {
    let mut guard = ADVISOR_MODEL.lock().unwrap();
    let was_set = guard.take().is_some();

    if was_set {
        Ok(CommandResult::Output(
            "Advisor model cleared.".to_string(),
        ))
    } else {
        Ok(CommandResult::Output(
            "No advisor model was set.".to_string(),
        ))
    }
}

/// Get the current advisor model name, if set. Exposed for the query loop.
pub fn get_advisor_model() -> Option<String> {
    ADVISOR_MODEL.lock().ok()?.clone()
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
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    fn reset_advisor() {
        let mut guard = ADVISOR_MODEL.lock().unwrap();
        *guard = None;
    }

    #[tokio::test]
    async fn test_set_and_show_advisor() {
        // This test exercises set, show current, change, and clear in sequence
        // to avoid global state races with parallel tests.
        let handler = AdvisorHandler;
        let mut ctx = test_ctx();

        // Clear first
        reset_advisor();

        // Show when not set
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No advisor"));
            }
            _ => panic!("Expected Output"),
        }

        // Set advisor
        let result = handler.execute("claude-sonnet-4", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("set to"));
                assert!(text.contains("claude-sonnet-4"));
            }
            _ => panic!("Expected Output"),
        }

        // Show current
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Current advisor model:"));
                assert!(text.contains("claude-sonnet-4"));
            }
            _ => panic!("Expected Output"),
        }

        // Change advisor
        let result = handler.execute("model-b", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("changed from"));
                assert!(text.contains("model-b"));
            }
            _ => panic!("Expected Output"),
        }

        // Clear advisor
        let result = handler.execute("off", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("cleared")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(get_advisor_model(), None);
    }

    #[tokio::test]
    async fn test_clear_when_not_set() {
        let handler = AdvisorHandler;
        let mut ctx = test_ctx();

        // Ensure cleared first
        reset_advisor();

        let result = handler.execute("unset", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("No advisor model was set")),
            _ => panic!("Expected Output"),
        }
    }
}
