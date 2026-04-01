//! `/login` command — authenticate with Anthropic.
//!
//! Currently supports:
//! - API Key: prompts user to set `ANTHROPIC_API_KEY` env var
//! - OAuth: interface defined but not implemented
//!
//! OAuth login flow will be implemented when network features are ready.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;

pub struct LoginHandler;

#[async_trait]
impl CommandHandler for LoginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let current_auth = auth::resolve_auth();

        // Show current auth status
        if args.trim() == "status" {
            let status = match &current_auth {
                auth::AuthMethod::ApiKey(key) => {
                    let masked = mask_key(key);
                    format!("Authenticated via API Key: {}", masked)
                }
                auth::AuthMethod::ExternalToken(_) => {
                    "Authenticated via external auth token (ANTHROPIC_AUTH_TOKEN)".to_string()
                }
                auth::AuthMethod::None => {
                    "Not authenticated".to_string()
                }
            };
            return Ok(CommandResult::Output(status));
        }

        // Already authenticated?
        if current_auth.is_authenticated() {
            let method = match &current_auth {
                auth::AuthMethod::ApiKey(key) => format!("API Key ({})", mask_key(key)),
                auth::AuthMethod::ExternalToken(_) => "External Auth Token".to_string(),
                auth::AuthMethod::None => unreachable!(),
            };
            return Ok(CommandResult::Output(format!(
                "Already authenticated: {}\n\
                 Use `/logout` to sign out first, or `/login status` to view details.",
                method
            )));
        }

        // Not authenticated — guide the user
        Ok(CommandResult::Output(
            "Authentication methods:\n\n\
             1. Set API Key (recommended):\n   \
                export ANTHROPIC_API_KEY=\"sk-ant-...\"\n\n\
             2. Set external auth token:\n   \
                export ANTHROPIC_AUTH_TOKEN=\"your-token\"\n\n\
             3. OAuth login (not yet implemented):\n   \
                Will be available in a future version.\n\n\
             Set the environment variable and restart to authenticate."
            .to_string(),
        ))
    }
}

/// Mask an API key for display: show prefix and last 4 chars.
fn mask_key(key: &str) -> String {
    if key.len() > 12 {
        format!("{}...{}", &key[..7], &key[key.len() - 4..])
    } else {
        "sk-ant-****".to_string()
    }
}
