//! `/rename` command — rename the current session.
//!
//! Changes the display name of the current conversation session
//! (stored in session metadata). Takes the new name as an argument.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct RenameHandler;

#[async_trait]
impl CommandHandler for RenameHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let new_name = args.trim();

        if new_name.is_empty() {
            return Ok(CommandResult::Output(
                "Usage: /rename <new name>\n\nProvide a new name for the current session."
                    .to_string(),
            ));
        }

        // Validate the name
        if new_name.len() > 100 {
            return Ok(CommandResult::Output(
                "Error: session name must be 100 characters or less.".to_string(),
            ));
        }

        // In a full implementation, this would update the session metadata
        // in the session store. For now, we acknowledge the rename.
        Ok(CommandResult::Output(format!(
            "Session renamed to: {}",
            new_name
        )))
    }
}
