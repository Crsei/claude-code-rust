//! `/peers` command -- peer session management.
//!
//! Peer sessions allow multiple Claude Code instances to collaborate by
//! sharing context and coordinating actions. This is a feature-gated
//! capability that requires multi-session infrastructure.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct PeersHandler;

#[async_trait]
impl CommandHandler for PeersHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let trimmed = args.trim();

        if trimmed.is_empty() {
            return Ok(CommandResult::Output(overview()));
        }

        let (subcmd, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((cmd, remainder)) => (cmd, remainder.trim()),
            None => (trimmed, ""),
        };

        match subcmd.to_lowercase().as_str() {
            "list" | "ls" => Ok(CommandResult::Output(
                "No active peer sessions.".to_string(),
            )),
            "connect" => {
                if rest.is_empty() {
                    Ok(CommandResult::Output(
                        "Usage: /peers connect <session-id>\n\n\
                         Specify the session ID of the peer to connect to."
                            .to_string(),
                    ))
                } else {
                    Ok(CommandResult::Output(
                        "Peer connection not available in standalone mode.".to_string(),
                    ))
                }
            }
            _ => Ok(CommandResult::Output(format!(
                "Unknown subcommand: '{}'\n\n{}",
                subcmd,
                overview()
            ))),
        }
    }
}

fn overview() -> String {
    "Peer sessions allow multiple Claude Code instances to collaborate.\n\n\
     Usage: /peers <subcommand>\n\n\
     Subcommands:\n  \
       list              List active peer sessions\n  \
       connect <id>      Connect to a peer session\n\n\
     Peer sessions enable real-time collaboration between Claude Code\n\
     instances, allowing them to share context, divide work, and\n\
     coordinate actions across a shared codebase."
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
    async fn test_default_shows_overview() {
        let handler = PeersHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("multiple Claude Code instances to collaborate"));
                assert!(text.contains("list"));
                assert!(text.contains("connect"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_connect_standalone_mode() {
        let handler = PeersHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("connect abc-123", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("not available in standalone mode"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
