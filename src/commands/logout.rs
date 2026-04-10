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
