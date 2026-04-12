//! `/login` command — authenticate with an LLM provider.
//!
//! Usage:
//!   /login              — interactive login (choose method)
//!   /login status       — show current auth status
//!   /login sk-ant-...   — store API key directly
//!   /login 1|2|3        — select login method

use anyhow::Result;
use async_trait::async_trait;

use super::login_code;
use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;
use crate::auth::oauth::OAuthMethod;

pub struct LoginHandler;

#[async_trait]
impl CommandHandler for LoginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if args == "status" {
            return Ok(CommandResult::Output(auth_status_text()));
        }

        if args.starts_with("sk-ant-") {
            return Ok(CommandResult::Output(store_api_key(args)));
        }

        if args.is_empty() {
            return Ok(CommandResult::Output(login_menu()));
        }

        match args {
            "1" => Ok(CommandResult::Output(
                "Paste your API key:\n  /login sk-ant-api03-...".to_string(),
            )),
            "2" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::ClaudeAi,
            ))),
            "3" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::Console,
            ))),
            _ => Ok(CommandResult::Output(format!(
                "Unknown option: \"{}\"\n\n{}",
                args,
                login_menu()
            ))),
        }
    }
}

fn login_menu() -> String {
    "Select login method:\n\
     \n  [1] API Key (paste manually)\
     \n  [2] Claude.ai OAuth (Pro/Max subscription)\
     \n  [3] Console OAuth (API billing)\
     \n\nType /login 1, /login 2, or /login 3"
        .to_string()
}

fn auth_status_text() -> String {
    let current = auth::resolve_auth();
    match &current {
        auth::AuthMethod::ApiKey(key) => {
            let source = if std::env::var("ANTHROPIC_API_KEY")
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            {
                "env ANTHROPIC_API_KEY"
            } else {
                "system keychain"
            };
            format!(
                "Authenticated: API Key {} (source: {})",
                mask_key(key),
                source
            )
        }
        auth::AuthMethod::ExternalToken(_) => {
            "Authenticated: External Token (ANTHROPIC_AUTH_TOKEN)".to_string()
        }
        auth::AuthMethod::OAuthToken { method, .. } => {
            format!("Authenticated: OAuth ({})", method)
        }
        auth::AuthMethod::None => "Not authenticated".to_string(),
    }
}

fn store_api_key(key: &str) -> String {
    if !auth::api_key::validate_api_key(key) {
        return "Invalid API key format. Keys start with \"sk-ant-\" and are >20 chars.".into();
    }
    match auth::api_key::store_api_key(key) {
        Ok(()) => format!("API Key {} stored to keychain.", mask_key(key)),
        Err(e) => format!("Failed to store API key: {}", e),
    }
}

fn mask_key(key: &str) -> String {
    if key.len() > 12 {
        format!("{}...{}", &key[..7], &key[key.len() - 4..])
    } else {
        "sk-ant-****".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_key_long() {
        let key = "sk-ant-api03-abcdefghijklmnop";
        let masked = mask_key(key);
        assert!(masked.starts_with("sk-ant-"));
        assert!(masked.contains("..."));
        assert!(masked.ends_with(&key[key.len() - 4..]));
    }

    #[test]
    fn test_mask_key_short() {
        let masked = mask_key("short");
        assert_eq!(masked, "sk-ant-****");
    }

    #[test]
    fn test_login_menu_contains_options() {
        let menu = login_menu();
        assert!(menu.contains("[1]"));
        assert!(menu.contains("[2]"));
        assert!(menu.contains("[3]"));
        assert!(menu.contains("API Key"));
        assert!(menu.contains("OAuth"));
    }
}
