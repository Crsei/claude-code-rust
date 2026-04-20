//! `/login` command — authenticate with an LLM provider.
//!
//! Usage:
//!   /login              — interactive login (choose method)
//!   /login status       — show current auth status
//!   /login sk-ant-...   — store API key directly
//!   /login 1|2|3|4      — select login method

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
            "4" | "codex" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::OpenAiCodex,
            ))),
            "5" | "codex-cli" => Ok(CommandResult::Output(check_codex_cli())),
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
     \n  [4] OpenAI Codex OAuth (ChatGPT subscription)\
     \n  [5] Import from Codex CLI (~/.codex/auth.json)\
     \n\nType /login 1, /login 2, /login 3, /login 4, or /login 5"
        .to_string()
}

fn auth_status_text() -> String {
    if let Some(codex_status) = codex_auth_status_text() {
        return codex_status;
    }

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

fn codex_auth_status_text() -> Option<String> {
    if std::env::var("OPENAI_CODEX_AUTH_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return Some("Authenticated: OpenAI Codex OAuth (env OPENAI_CODEX_AUTH_TOKEN)".to_string());
    }

    // Check cc-rust's own credentials.json
    if let Ok(Some(stored)) = auth::token::load_token() {
        let method = stored.oauth_method.as_deref().unwrap_or_default();
        if method.eq_ignore_ascii_case("openai_codex") {
            return if auth::token::is_token_expired(&stored) {
                Some("OpenAI Codex OAuth token is expired. Run /login 4 to refresh.".to_string())
            } else {
                Some("Authenticated: OpenAI Codex OAuth (stored credentials)".to_string())
            };
        }
    }

    // Check Codex CLI fallback
    if let Some(cred) = auth::codex_cli::read_codex_cli_credential() {
        return if auth::codex_cli::is_credential_expired(&cred) {
            Some(
                "OpenAI Codex OAuth (from Codex CLI) is expired. \
                 Run /login 5 to refresh or /login 4 for a fresh login."
                    .to_string(),
            )
        } else {
            Some(
                "Authenticated: OpenAI Codex OAuth (from Codex CLI ~/.codex/auth.json)".to_string(),
            )
        };
    }

    None
}

fn check_codex_cli() -> String {
    let cred = match auth::codex_cli::read_codex_cli_credential() {
        Some(c) => c,
        None => {
            // Distinguish: file doesn't exist vs. wrong auth_mode
            if auth::codex_cli::codex_cli_auth_path().is_none() {
                return "Codex CLI not found. Install Codex CLI and run \
                        'codex' to log in first."
                    .to_string();
            }
            return "Codex CLI auth.json found but not usable \
                    (auth_mode is not chatgpt or tokens are missing). \
                    Use /login 1 to paste your API key, or /login 4 for OAuth."
                .to_string();
        }
    };

    if !auth::codex_cli::is_credential_expired(&cred) {
        return "Codex CLI credentials detected and valid. \
                cc-rust will use them automatically."
            .to_string();
    }

    // Expired — try to refresh now
    match auth::resolve_codex_auth_token() {
        Some(_) => "Codex CLI token was expired but has been refreshed successfully. \
             cc-rust will use it automatically."
            .to_string(),
        None => "Codex CLI token is expired and refresh failed. \
             Run /login 4 for a fresh OAuth login, or re-login in Codex CLI."
            .to_string(),
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
        assert!(menu.contains("[4]"));
        assert!(menu.contains("[5]"));
        assert!(menu.contains("API Key"));
        assert!(menu.contains("OAuth"));
        assert!(menu.contains("Codex CLI"));
    }

    #[test]
    fn test_check_codex_cli_no_file() {
        // Point CODEX_HOME at a nonexistent directory
        let dir = tempfile::TempDir::new().unwrap();
        let empty_sub = dir.path().join("empty");
        std::fs::create_dir_all(&empty_sub).unwrap();
        let prev = std::env::var("CODEX_HOME").ok();
        std::env::set_var("CODEX_HOME", empty_sub.to_str().unwrap());

        let msg = check_codex_cli();
        assert!(
            msg.contains("not found"),
            "expected 'not found' message, got: {}",
            msg
        );

        match prev {
            Some(v) => std::env::set_var("CODEX_HOME", v),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }
}
