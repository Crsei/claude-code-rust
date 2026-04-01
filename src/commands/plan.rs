//! `/plan` command — toggle plan mode.
//!
//! Plan mode changes the assistant's behavior to focus on planning
//! and analysis rather than execution. In plan mode, tools are
//! restricted to read-only operations.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::tool::PermissionMode;

pub struct PlanHandler;

#[async_trait]
impl CommandHandler for PlanHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        let is_plan_mode = ctx.app_state.tool_permission_context.mode == PermissionMode::Plan;

        match arg.as_str() {
            "on" | "enter" | "start" => {
                if is_plan_mode {
                    return Ok(CommandResult::Output(
                        "Already in plan mode.".to_string(),
                    ));
                }
                // Save the current mode so we can restore it
                ctx.app_state.tool_permission_context.pre_plan_mode =
                    Some(ctx.app_state.tool_permission_context.mode.clone());
                ctx.app_state.tool_permission_context.mode = PermissionMode::Plan;

                Ok(CommandResult::Output(
                    "Entered plan mode. Tools are restricted to read-only operations.\n\
                     Use `/plan off` to exit."
                        .to_string(),
                ))
            }
            "off" | "exit" | "stop" => {
                if !is_plan_mode {
                    return Ok(CommandResult::Output(
                        "Not in plan mode.".to_string(),
                    ));
                }
                // Restore previous mode
                let prev = ctx
                    .app_state
                    .tool_permission_context
                    .pre_plan_mode
                    .take()
                    .unwrap_or(PermissionMode::Default);
                ctx.app_state.tool_permission_context.mode = prev;

                Ok(CommandResult::Output(
                    "Exited plan mode. Normal tool access restored.".to_string(),
                ))
            }
            "" => {
                // Toggle
                if is_plan_mode {
                    let prev = ctx
                        .app_state
                        .tool_permission_context
                        .pre_plan_mode
                        .take()
                        .unwrap_or(PermissionMode::Default);
                    ctx.app_state.tool_permission_context.mode = prev;
                    Ok(CommandResult::Output(
                        "Exited plan mode.".to_string(),
                    ))
                } else {
                    ctx.app_state.tool_permission_context.pre_plan_mode =
                        Some(ctx.app_state.tool_permission_context.mode.clone());
                    ctx.app_state.tool_permission_context.mode = PermissionMode::Plan;
                    Ok(CommandResult::Output(
                        "Entered plan mode.".to_string(),
                    ))
                }
            }
            _ => Ok(CommandResult::Output(
                "Usage: /plan [on|off]\n\nToggles plan mode without arguments.".to_string(),
            )),
        }
    }
}
