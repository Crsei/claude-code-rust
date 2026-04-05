//! /permissions command -- view and modify permission settings.
//!
//! Subcommands:
//! - `/permissions`           -- show current permission mode and rules
//! - `/permissions mode <m>`  -- switch permission mode
//! - `/permissions allow <t>` -- add a tool to the always-allow list
//! - `/permissions deny <t>`  -- add a tool to the always-deny list
//! - `/permissions reset`     -- reset all permission rules to defaults
//!
//! The TypeScript version opens a React-based PermissionRuleList component.
//! In the Rust CLI we display a text listing and accept subcommands.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::tool::PermissionMode;

/// Handler for the `/permissions` slash command.
pub struct PermissionsHandler;

#[async_trait]
impl CommandHandler for PermissionsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();

        match parts.first().copied() {
            Some("mode") => handle_mode(&parts[1..], ctx),
            Some("allow") => handle_allow(&parts[1..], ctx),
            Some("deny") => handle_deny(&parts[1..], ctx),
            Some("reset") => handle_reset(ctx),
            None => handle_show(ctx),
            Some(sub) => Ok(CommandResult::Output(format!(
                "Unknown permissions subcommand: '{}'\n\
                 Usage:\n  \
                   /permissions               -- show current settings\n  \
                   /permissions mode <mode>    -- set mode (default, auto, bypass, plan)\n  \
                   /permissions allow <tool>   -- always allow a tool\n  \
                   /permissions deny <tool>    -- always deny a tool\n  \
                   /permissions reset          -- reset to defaults",
                sub
            ))),
        }
    }
}

/// Display current permission settings.
fn handle_show(ctx: &CommandContext) -> Result<CommandResult> {
    let perm = &ctx.app_state.tool_permission_context;

    let mut lines = Vec::new();
    lines.push("Permission settings:".into());
    lines.push(String::new());
    lines.push(format!("  Mode: {:?}", perm.mode));
    lines.push(format!(
        "  Bypass available: {}",
        perm.is_bypass_permissions_mode_available
    ));

    // Always-allow rules.
    if !perm.always_allow_rules.is_empty() {
        lines.push(String::new());
        lines.push("  Always allow:".into());
        for (source, rules) in &perm.always_allow_rules {
            for rule in rules {
                lines.push(format!("    {} (from {})", rule, source));
            }
        }
    }

    // Always-deny rules.
    if !perm.always_deny_rules.is_empty() {
        lines.push(String::new());
        lines.push("  Always deny:".into());
        for (source, rules) in &perm.always_deny_rules {
            for rule in rules {
                lines.push(format!("    {} (from {})", rule, source));
            }
        }
    }

    // Always-ask rules.
    if !perm.always_ask_rules.is_empty() {
        lines.push(String::new());
        lines.push("  Always ask:".into());
        for (source, rules) in &perm.always_ask_rules {
            for rule in rules {
                lines.push(format!("    {} (from {})", rule, source));
            }
        }
    }

    if perm.always_allow_rules.is_empty()
        && perm.always_deny_rules.is_empty()
        && perm.always_ask_rules.is_empty()
    {
        lines.push(String::new());
        lines.push("  No custom permission rules configured.".into());
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

/// Switch the permission mode.
fn handle_mode(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(format!(
            "Current mode: {:?}\n\
             Usage: /permissions mode <default|auto|bypass|plan>",
            ctx.app_state.tool_permission_context.mode
        )));
    }

    let mode_str = parts[0];
    let mode = match mode_str.to_lowercase().as_str() {
        "default" | "ask" => PermissionMode::Default,
        "auto" => PermissionMode::Auto,
        "bypass" => {
            if !ctx.app_state.tool_permission_context.is_bypass_permissions_mode_available {
                return Ok(CommandResult::Output(
                    "Bypass mode is not available in the current configuration.".into(),
                ));
            }
            PermissionMode::Bypass
        }
        "plan" | "readonly" => PermissionMode::Plan,
        _ => {
            return Ok(CommandResult::Output(format!(
                "Unknown permission mode: '{}'\n\
                 Available modes: default, auto, bypass, plan",
                mode_str
            )));
        }
    };

    ctx.app_state.tool_permission_context.mode = mode;
    Ok(CommandResult::Output(format!(
        "Permission mode set to: {:?}",
        ctx.app_state.tool_permission_context.mode
    )))
}

/// Add a tool to the always-allow list.
fn handle_allow(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /permissions allow <tool_name>".into(),
        ));
    }

    let tool_name = parts[0].to_string();
    ctx.app_state
        .tool_permission_context
        .always_allow_rules
        .entry("user".into())
        .or_default()
        .push(tool_name.clone());

    Ok(CommandResult::Output(format!(
        "Tool '{}' added to always-allow list.",
        tool_name
    )))
}

/// Add a tool to the always-deny list.
fn handle_deny(parts: &[&str], ctx: &mut CommandContext) -> Result<CommandResult> {
    if parts.is_empty() {
        return Ok(CommandResult::Output(
            "Usage: /permissions deny <tool_name>".into(),
        ));
    }

    let tool_name = parts[0].to_string();
    ctx.app_state
        .tool_permission_context
        .always_deny_rules
        .entry("user".into())
        .or_default()
        .push(tool_name.clone());

    Ok(CommandResult::Output(format!(
        "Tool '{}' added to always-deny list.",
        tool_name
    )))
}

/// Reset all permission rules to defaults.
fn handle_reset(ctx: &mut CommandContext) -> Result<CommandResult> {
    let default = crate::types::app_state::AppState::default();
    ctx.app_state.tool_permission_context = default.tool_permission_context;

    Ok(CommandResult::Output(
        "Permission rules reset to defaults.".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_permissions_show() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Permission settings"));
                assert!(text.contains("Mode:"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_permissions_mode_switch() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("mode auto", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Auto"));
            }
            _ => panic!("Expected Output result"),
        }
        assert_eq!(ctx.app_state.tool_permission_context.mode, PermissionMode::Auto);
    }

    #[tokio::test]
    async fn test_permissions_allow() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("allow Bash", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Bash"));
                assert!(text.contains("always-allow"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_permissions_deny() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("deny Bash", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Bash"));
                assert!(text.contains("always-deny"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_permissions_reset() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        // First add a rule.
        handler.execute("allow Bash", &mut ctx).await.unwrap();
        assert!(!ctx.app_state.tool_permission_context.always_allow_rules.is_empty());

        // Then reset.
        let result = handler.execute("reset", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("reset to defaults"));
            }
            _ => panic!("Expected Output result"),
        }
        assert!(ctx.app_state.tool_permission_context.always_allow_rules.is_empty());
    }

    #[tokio::test]
    async fn test_permissions_unknown_subcommand() {
        let handler = PermissionsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("foobar", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Unknown permissions subcommand"));
            }
            _ => panic!("Expected Output result"),
        }
    }
}
