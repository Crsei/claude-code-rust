//! Authentication system
//!
//! Supports three active auth methods:
//! - API Key: via `ANTHROPIC_API_KEY` env var or system keychain
//! - External Auth Token: via `ANTHROPIC_AUTH_TOKEN` env var
//! - OAuth Token: from `~/.cc-rust/credentials.json` (Claude.ai or Console)

pub mod api_key;
pub mod oauth;
pub mod token;

// ---------------------------------------------------------------------------
// Auth method enum
// ---------------------------------------------------------------------------

/// Authentication method resolved at startup.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Direct API key (`ANTHROPIC_API_KEY`)
    ApiKey(String),
    /// External auth token (`ANTHROPIC_AUTH_TOKEN`)
    ExternalToken(String),
    /// OAuth access token (Claude.ai or Console)
    OAuthToken {
        access_token: String,
        /// "claude_ai" or "console"
        method: String,
    },
    /// No authentication configured
    None,
}

impl AuthMethod {
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn api_key(&self) -> Option<&str> {
        match self {
            Self::ApiKey(key) => Some(key),
            _ => None,
        }
    }

    pub fn bearer_token(&self) -> Option<&str> {
        match self {
            Self::ExternalToken(token) => Some(token),
            Self::OAuthToken { access_token, .. } => Some(access_token),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Resolve auth from environment
// ---------------------------------------------------------------------------

/// Resolve authentication from environment variables, OAuth tokens, and keychain.
///
/// Priority:
/// 1. `ANTHROPIC_API_KEY` env var
/// 2. `ANTHROPIC_AUTH_TOKEN` env var
/// 3. OAuth token from `~/.cc-rust/credentials.json` (if not expired)
/// 4. API key from system keychain
/// 5. `AuthMethod::None`
pub fn resolve_auth() -> AuthMethod {
    // 1. ANTHROPIC_API_KEY
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() && api_key::validate_api_key(&key) {
            return AuthMethod::ApiKey(key);
        }
    }

    // 2. ANTHROPIC_AUTH_TOKEN
    if let Ok(token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
        if !token.is_empty() {
            return AuthMethod::ExternalToken(token);
        }
    }

    // 3. OAuth token from disk (with auto-refresh if expired)
    if let Ok(Some((access_token, method))) = try_resolve_oauth() {
        if method == "console" {
            // Console mode: API key is in keychain (created at login).
            // Fall through to keychain check below.
        } else {
            return AuthMethod::OAuthToken {
                access_token,
                method,
            };
        }
    }

    // 4. Keychain
    if let Ok(Some(key)) = api_key::load_api_key() {
        if api_key::validate_api_key(&key) {
            return AuthMethod::ApiKey(key);
        }
    }

    AuthMethod::None
}

// ---------------------------------------------------------------------------
// OAuth helpers
// ---------------------------------------------------------------------------

/// Try to load OAuth token, auto-refreshing if expired.
///
/// Returns `Ok(Some((access_token, method)))` or `Ok(None)`.
fn try_resolve_oauth() -> anyhow::Result<Option<(String, String)>> {
    let stored = match token::load_token()? {
        Some(t) => t,
        None => return Ok(None),
    };

    let method = stored.oauth_method.clone().unwrap_or_default();

    if !token::is_token_expired(&stored) {
        return Ok(Some((stored.access_token, method)));
    }

    // Token expired — try synchronous refresh via a blocking runtime.
    // If we're already inside a tokio runtime, spawn a blocking task;
    // otherwise create a temporary one.
    let refresh_tok = match &stored.refresh_token {
        Some(t) => t.clone(),
        None => {
            let _ = token::remove_token();
            return Ok(None);
        }
    };

    let scopes: Vec<String> = stored.scopes.clone();
    match try_refresh_sync(&refresh_tok, &scopes, &stored) {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::warn!(error = %e, "OAuth auto-refresh failed, clearing credentials");
            let _ = token::remove_token();
            Ok(None)
        }
    }
}

/// Synchronous wrapper for token refresh (called from `resolve_auth()`).
fn try_refresh_sync(
    refresh_tok: &str,
    scopes: &[String],
    stored: &token::StoredToken,
) -> anyhow::Result<Option<(String, String)>> {
    let scope_strs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
    let scopes_ref: &[&str] = if scope_strs.is_empty() {
        oauth::config::CLAUDE_AI_SCOPES
    } else {
        &scope_strs
    };

    // Use tokio Handle if available, otherwise skip refresh
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return Ok(None), // No async runtime — can't refresh
    };

    let refresh_tok = refresh_tok.to_string();
    let scopes_owned: Vec<String> = scopes_ref.iter().map(|s| s.to_string()).collect();
    let stored_clone = stored.clone();

    let result = std::thread::spawn(move || {
        handle.block_on(async {
            let scope_strs: Vec<&str> = scopes_owned.iter().map(|s| s.as_str()).collect();
            match oauth::client::refresh_token(&refresh_tok, &scope_strs).await {
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
                            stored_clone.scopes
                        } else {
                            new_scopes
                        },
                        oauth_method: stored_clone.oauth_method,
                    };
                    let method = updated.oauth_method.clone().unwrap_or_default();
                    let _ = token::save_token(&updated);
                    Ok(Some((resp.access_token, method)))
                }
                Err(e) => Err(e),
            }
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("OAuth refresh thread panicked"))??;

    Ok(result)
}

/// Clear all OAuth state (tokens + keychain API key).
pub fn oauth_logout() -> anyhow::Result<()> {
    token::remove_token()?;
    let _ = api_key::remove_api_key();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_method_api_key() {
        let auth = AuthMethod::ApiKey("sk-ant-test-key-123456789".into());
        assert!(auth.is_authenticated());
        assert_eq!(auth.api_key(), Some("sk-ant-test-key-123456789"));
        assert_eq!(auth.bearer_token(), None);
    }

    #[test]
    fn test_auth_method_external_token() {
        let auth = AuthMethod::ExternalToken("ext-token-abc".into());
        assert!(auth.is_authenticated());
        assert_eq!(auth.api_key(), None);
        assert_eq!(auth.bearer_token(), Some("ext-token-abc"));
    }

    #[test]
    fn test_auth_method_none() {
        let auth = AuthMethod::None;
        assert!(!auth.is_authenticated());
        assert_eq!(auth.api_key(), None);
        assert_eq!(auth.bearer_token(), None);
    }

    #[test]
    fn test_auth_method_oauth_token() {
        let auth = AuthMethod::OAuthToken {
            access_token: "oauth-test-token".into(),
            method: "claude_ai".into(),
        };
        assert!(auth.is_authenticated());
        assert_eq!(auth.api_key(), None);
        assert_eq!(auth.bearer_token(), Some("oauth-test-token"));
    }
}
