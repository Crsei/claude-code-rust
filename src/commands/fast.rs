//! `/fast` command — toggle fast mode.
//!
//! Fast mode uses the same model with faster output generation.
//! Toggle on/off without arguments, or explicitly set with "on"/"off".

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct FastHandler;

#[async_trait]
impl CommandHandler for FastHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        let new_state = match arg.as_str() {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            "" => !ctx.app_state.fast_mode, // toggle
            _ => {
                return Ok(CommandResult::Output(
                    "Usage: /fast [on|off]\n\nToggles fast mode without arguments.".to_string(),
                ));
            }
        };

        ctx.app_state.fast_mode = new_state;

        let status = if new_state { "ON" } else { "OFF" };
        Ok(CommandResult::Output(format!("Fast mode: {}", status)))
    }
}
