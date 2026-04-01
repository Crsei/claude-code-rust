//! `/effort` command — set the thinking effort level.
//!
//! Controls the reasoning depth for the model. Valid values are
//! "low", "medium", "high", or a numeric budget token count.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct EffortHandler;

const VALID_LEVELS: &[&str] = &["low", "medium", "high"];

#[async_trait]
impl CommandHandler for EffortHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        if arg.is_empty() {
            let current = ctx
                .app_state
                .effort_value
                .as_deref()
                .unwrap_or("(not set — default)");

            return Ok(CommandResult::Output(format!(
                "Current effort level: {}\n\n\
                 Usage: /effort <level>\n\
                 Valid levels: low, medium, high",
                current
            )));
        }

        if VALID_LEVELS.contains(&arg.as_str()) {
            ctx.app_state.effort_value = Some(arg.clone());
            Ok(CommandResult::Output(format!(
                "Effort level set to: {}",
                arg
            )))
        } else {
            Ok(CommandResult::Output(format!(
                "Invalid effort level: '{}'\nValid levels: {}",
                arg,
                VALID_LEVELS.join(", ")
            )))
        }
    }
}
