//! `/channels` command -- view connected channels (KAIROS).
//!
//! Shows the list of connected communication channels for the assistant mode.
//!
//! Requires `FEATURE_KAIROS_CHANNELS=1` (which itself requires `FEATURE_KAIROS=1`).

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::features::{self, Feature};

pub struct ChannelsHandler;

#[async_trait]
impl CommandHandler for ChannelsHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        if !features::enabled(Feature::KairosChannels) {
            return Ok(CommandResult::Output(
                "Channels require FEATURE_KAIROS_CHANNELS=1".into(),
            ));
        }

        match args.trim() {
            "" | "list" => Ok(CommandResult::Output("No channels connected.".into())),
            "status" => Ok(CommandResult::Output(
                "Channel status: no active connections.".into(),
            )),
            _ => Ok(CommandResult::Output(
                "Usage: /channels [list|status]".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_feature_gate() {
        let handler = ChannelsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("FEATURE_KAIROS_CHANNELS")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_unknown_argument_gated() {
        let handler = ChannelsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("connect foo", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("FEATURE_KAIROS_CHANNELS")),
            _ => panic!("Expected Output"),
        }
    }
}
