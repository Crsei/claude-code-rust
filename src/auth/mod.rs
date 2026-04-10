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

    // 3. OAuth token from disk
    if let Ok(Some(stored)) = token::load_token() {
        if !token::is_token_expired(&stored) {
            let method = stored.oauth_method.clone().unwrap_or_default();
            // Console mode: API key should be in keychain (created at login)
            // Claude.ai mode: use Bearer token directly
            if method == "console" {
                // Fall through to keychain check below
            } else {
                return AuthMethod::OAuthToken {
                    access_token: stored.access_token,
                    method,
                };
            }
        }
        // Note: expired tokens are handled by try_refresh_if_needed() at startup
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
// OAuth convenience functions
// ---------------------------------------------------------------------------

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
