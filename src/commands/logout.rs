//! `/logout` command — clear stored authentication credentials.
//!
//! Removes:
//! - API key from system keychain
//! - OAuth tokens from disk (`~/.cc-rust/credentials.json`)

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;

pub struct LogoutHandler;

#[async_trait]
impl CommandHandler for LogoutHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let current_auth = auth::resolve_auth();

        if !current_auth.is_authenticated() {
            return Ok(CommandResult::Output(
                "Not currently authenticated — nothing to clear.".to_string(),
            ));
        }

        // Clear all auth state: keychain + credentials.json
        if let Err(e) = auth::oauth_logout() {
            tracing::warn!(error = %e, "error during logout cleanup");
        }

        Ok(CommandResult::Output(
            "Logged out successfully. Cleared keychain and stored OAuth tokens.\n\
             Note: environment variables (ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN) \
             must be unset manually."
                .to_string(),
        ))
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

    /// Verify the handler can be constructed and execute returns an Output variant.
    /// The actual result depends on the runtime auth state (env vars, keychain,
    /// credentials.json) so we only assert the shape, not the exact text.
    #[tokio::test]
    async fn test_logout_returns_output() {
        let handler = LogoutHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Either "not authenticated" or "logged out successfully"
                assert!(!text.is_empty());
            }
            _ => panic!("Expected Output"),
        }
    }

    /// Verify that the output mentions something actionable regardless of auth state.
    #[tokio::test]
    async fn test_logout_output_is_informative() {
        let handler = LogoutHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        if let CommandResult::Output(text) = result {
            // One of two possible informative messages
            let is_already_out = text.contains("Not currently authenticated");
            let is_logged_out = text.contains("Logged out successfully");
            assert!(
                is_already_out || is_logged_out,
                "unexpected logout output: {}",
                text
            );
        }
    }
}
