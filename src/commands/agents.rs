//! `/agents` command -- list and manage agent teams.
//!
//! Shows existing agent teams and provides basic team management.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct AgentsHandler;

#[async_trait]
impl CommandHandler for AgentsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let subcmd = args.trim().to_lowercase();

        match subcmd.as_str() {
            "list" | "ls" => list_teams(ctx),
            "" => show_help(),
            _ => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\n{}",
                subcmd,
                help_text()
            ))),
        }
    }
}

fn list_teams(ctx: &CommandContext) -> Result<CommandResult> {
    let mut lines = Vec::new();
    lines.push("Agent Teams".to_string());
    lines.push("─".repeat(30));

    match &ctx.app_state.team_context {
        Some(team_ctx) => {
            lines.push(format!("Team:   {}", team_ctx.team_name));
            let is_leader = team_ctx.is_leader.unwrap_or(false);
            lines.push(format!("Role:   {}", if is_leader { "leader" } else { "member" }));
            lines.push(format!("Leader: {}", team_ctx.lead_agent_id));
            if let Some(ref self_name) = team_ctx.self_agent_name {
                lines.push(format!("Self:   {}", self_name));
            }
            if !team_ctx.teammates.is_empty() {
                lines.push(format!("Teammates ({}):", team_ctx.teammates.len()));
                for (id, mate) in &team_ctx.teammates {
                    lines.push(format!("  - {} ({})", mate.name, id));
                }
            }
        }
        None => {
            lines.push("No active agent team.".to_string());
            lines.push(String::new());
            lines.push("To start a team, use the coordinator mode or".to_string());
            lines.push("configure agents in .cc-rust/settings.json.".to_string());
        }
    }

    Ok(CommandResult::Output(lines.join("\n")))
}

fn show_help() -> Result<CommandResult> {
    Ok(CommandResult::Output(help_text()))
}

fn help_text() -> String {
    "Usage: /agents <subcommand>\n\n\
     Subcommands:\n  \
       list    Show existing agent teams\n\n\
     Agent teams enable multi-agent coordination where\n\
     multiple Claude instances collaborate on tasks."
        .to_string()
}

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

    #[tokio::test]
    async fn test_help_on_empty_args() {
        let handler = AgentsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Usage:"));
                assert!(text.contains("list"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_list_no_team() {
        let handler = AgentsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("No active agent team"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
