//! `/login-code` command — complete OAuth login by exchanging an authorization code.
//!
//! Usage:
//!   /login-code <authorization-code>
//!
//! This is the second step of the OAuth flow started by `/login 2` or `/login 3`.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth::oauth::{client, config, pkce};
use crate::auth::{api_key, token};

/// Pending OAuth state (PKCE verifier, state, method).
static PENDING_OAUTH: parking_lot::Mutex<Option<PendingOAuth>> =
    parking_lot::Mutex::new(None);

struct PendingOAuth {
    method: config::OAuthMethod,
    verifier: String,
    state: String,
}

/// Start a pending OAuth flow. Called from `/login 2` or `/login 3`.
///
/// Generates PKCE params, stores them, and returns the message with the auth URL.
pub fn start_pending(method: config::OAuthMethod) -> String {
    let verifier = pkce::generate_code_verifier();
    let challenge = pkce::generate_code_challenge(&verifier);
    let state = pkce::generate_state();
    let url = config::authorization_url(method, &challenge, &state);

    *PENDING_OAUTH.lock() = Some(PendingOAuth {
        method,
        verifier,
        state,
    });

    let method_name = match method {
        config::OAuthMethod::ClaudeAi => "Claude.ai",
        config::OAuthMethod::Console => "Console",
    };

    format!(
        "Opening {} authorization...\n\n\
         Please visit this URL to authorize:\n\n  {}\n\n\
         After authorizing, paste the code:\n  /login-code <paste-code-here>",
        method_name, url
    )
}

pub struct LoginCodeHandler;

#[async_trait]
impl CommandHandler for LoginCodeHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let code = args.trim();
        if code.is_empty() {
            return Ok(CommandResult::Output(
                "Usage: /login-code <authorization-code>\n\
                 Start the OAuth flow first with /login 2 or /login 3"
                    .to_string(),
            ));
        }

        let pending = PENDING_OAUTH.lock().take();
        let pending = match pending {
            Some(p) => p,
            None => {
                return Ok(CommandResult::Output(
                    "No pending OAuth flow. Start one with /login 2 or /login 3".to_string(),
                ));
            }
        };

        // Exchange code for tokens
        let token_resp = match client::exchange_code(
            code,
            &pending.verifier,
            &pending.state,
            config::MANUAL_REDIRECT_URL,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(CommandResult::Output(format!(
                    "Token exchange failed: {}\n\nPlease retry with /login 2 or /login 3",
                    e
                )));
            }
        };

        // Store tokens
        let expires_at = chrono::Utc::now().timestamp() + token_resp.expires_in as i64;
        let method_str = match pending.method {
            config::OAuthMethod::ClaudeAi => "claude_ai",
            config::OAuthMethod::Console => "console",
        };
        let scopes: Vec<String> = token_resp
            .scope
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let stored = token::StoredToken {
            access_token: token_resp.access_token.clone(),
            refresh_token: token_resp.refresh_token.clone(),
            expires_at: Some(expires_at),
            token_type: "bearer".into(),
            scopes,
            oauth_method: Some(method_str.to_string()),
        };

        if let Err(e) = token::save_token(&stored) {
            return Ok(CommandResult::Output(format!(
                "Failed to save OAuth tokens: {}",
                e
            )));
        }

        // Console mode: create API key
        if pending.method == config::OAuthMethod::Console {
            match client::create_api_key(&token_resp.access_token).await {
                Ok(raw_key) => {
                    if let Err(e) = api_key::store_api_key(&raw_key) {
                        return Ok(CommandResult::Output(format!(
                            "OAuth tokens saved, but keychain storage failed: {}",
                            e
                        )));
                    }
                    return Ok(CommandResult::Output(
                        "Logged in successfully (Console). API key stored to keychain."
                            .to_string(),
                    ));
                }
                Err(e) => {
                    return Ok(CommandResult::Output(format!(
                        "OAuth tokens saved, but API key creation failed: {}\n\
                         You can retry with /login 3",
                        e
                    )));
                }
            }
        }

        Ok(CommandResult::Output(
            "Logged in successfully (Claude.ai).".to_string(),
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

    // ---- start_pending() pure helpers ----

    #[test]
    fn test_start_pending_claude_ai_contains_url() {
        // Clear any pre-existing state first
        let _ = PENDING_OAUTH.lock().take();
        let msg = start_pending(config::OAuthMethod::ClaudeAi);
        assert!(
            msg.contains("Claude.ai"),
            "should mention Claude.ai, got: {}",
            msg
        );
        assert!(
            msg.contains("https://"),
            "should contain an auth URL, got: {}",
            msg
        );
        assert!(
            msg.contains("/login-code"),
            "should instruct user to use /login-code, got: {}",
            msg
        );
    }

    #[test]
    fn test_start_pending_console_contains_url() {
        let _ = PENDING_OAUTH.lock().take();
        let msg = start_pending(config::OAuthMethod::Console);
        assert!(msg.contains("Console"), "should mention Console, got: {}", msg);
        assert!(msg.contains("https://"));
    }

    #[test]
    fn test_start_pending_stores_state() {
        let _ = PENDING_OAUTH.lock().take();
        start_pending(config::OAuthMethod::ClaudeAi);
        let pending = PENDING_OAUTH.lock().take();
        assert!(
            pending.is_some(),
            "start_pending should store pending OAuth state"
        );
    }

    // ---- LoginCodeHandler error paths (no network required) ----

    #[tokio::test]
    async fn test_login_code_empty_args_shows_usage() {
        let handler = LoginCodeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("Usage"),
                    "expected usage hint, got: {}",
                    text
                );
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_login_code_no_pending_flow() {
        // Ensure no pending state
        let _ = PENDING_OAUTH.lock().take();
        let handler = LoginCodeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("some-fake-code", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(
                    text.contains("No pending OAuth flow"),
                    "expected no-pending message, got: {}",
                    text
                );
            }
            _ => panic!("Expected Output"),
        }
    }
}
