//! OAuth 2.0 Authorization Code + PKCE login flow.
//!
//! Supports two modes:
//! - **Claude.ai**: Bearer token for Pro/Max subscribers
//! - **Console**: Creates an API key via OAuth
//!
//! The login flow is two-step (driven by `/login` and `/login-code` commands):
//! 1. `/login 2|3` → generates PKCE, prints auth URL
//! 2. `/login-code <code>` → exchanges code for tokens, stores them

pub mod client;
pub mod config;
pub mod pkce;

use anyhow::Result;
pub use config::OAuthMethod;

use crate::auth::token;

/// Open a browser to the given URL.
///
/// Reserved for future implementation (auto-open browser + localhost callback).
/// Currently a no-op — the manual flow prints the URL for the user to copy.
pub async fn open_browser(_url: &str) -> Result<()> {
    // Future: use `open` crate to launch system browser
    // + spawn localhost HTTP callback server
    Ok(())
}

/// Try to auto-refresh the stored OAuth token if it is about to expire.
///
/// Returns the (possibly refreshed) access token and method, or `None`.
pub async fn try_refresh_if_needed() -> Result<Option<(String, String)>> {
    let stored = match token::load_token()? {
        Some(t) => t,
        None => return Ok(None),
    };

    let method = stored.oauth_method.clone().unwrap_or_default();

    if !token::is_token_expired(&stored) {
        return Ok(Some((stored.access_token, method)));
    }

    // Token expired — try refresh
    let refresh_tok = match &stored.refresh_token {
        Some(t) => t.clone(),
        None => {
            // No refresh token — clear and bail
            let _ = token::remove_token();
            return Ok(None);
        }
    };

    let scopes: Vec<&str> = stored.scopes.iter().map(|s| s.as_str()).collect();
    let scopes_ref = if scopes.is_empty() {
        config::CLAUDE_AI_SCOPES
    } else {
        &scopes
    };

    match client::refresh_token(&refresh_tok, scopes_ref).await {
        Ok(resp) => {
            let expires_at = chrono::Utc::now().timestamp() + resp.expires_in as i64;
            let new_scopes: Vec<String> = resp
                .scope
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            let updated = token::StoredToken {
                access_token: resp.access_token.clone(),
                refresh_token: resp.refresh_token.or(Some(refresh_tok)),
                expires_at: Some(expires_at),
                token_type: "bearer".into(),
                scopes: if new_scopes.is_empty() {
                    stored.scopes
                } else {
                    new_scopes
                },
                oauth_method: stored.oauth_method,
            };
            token::save_token(&updated)?;
            Ok(Some((resp.access_token, method)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "OAuth token refresh failed, clearing credentials");
            let _ = token::remove_token();
            Ok(None)
        }
    }
}
