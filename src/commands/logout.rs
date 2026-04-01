//! `/logout` command — clear stored authentication credentials.
//!
//! Removes:
//! - API key from system keychain (if `auth` feature enabled)
//! - OAuth tokens from disk (`~/.claude/credentials.json`)

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

        let mut cleared = Vec::new();

        // Remove API key from keychain
        if let Err(e) = auth::api_key::remove_api_key() {
            tracing::warn!(error = %e, "failed to remove API key from keychain");
        } else {
            cleared.push("keychain API key");
        }

        // Remove OAuth tokens from disk
        if let Err(e) = auth::token::remove_token() {
            tracing::warn!(error = %e, "failed to remove OAuth tokens from disk");
        } else {
            cleared.push("stored OAuth tokens");
        }

        let msg = if cleared.is_empty() {
            "Logged out. Note: environment variables (ANTHROPIC_API_KEY, \
             ANTHROPIC_AUTH_TOKEN) are still set in your shell — \
             unset them manually if needed."
                .to_string()
        } else {
            format!(
                "Logged out. Cleared: {}.\n\
                 Note: environment variables (ANTHROPIC_API_KEY, \
                 ANTHROPIC_AUTH_TOKEN) must be unset manually.",
                cleared.join(", ")
            )
        };

        Ok(CommandResult::Output(msg))
    }
}
