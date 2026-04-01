//! Authentication system
//!
//! Supports two active auth methods:
//! - API Key: via `ANTHROPIC_API_KEY` env var or system keychain
//! - External Auth Token: via `ANTHROPIC_AUTH_TOKEN` env var
//!
//! OAuth login flow is defined as interface only (not implemented).

pub mod api_key;
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
    /// No authentication configured
    None,
}

#[allow(dead_code)]
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
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Resolve auth from environment
// ---------------------------------------------------------------------------

/// Resolve authentication from environment variables and keychain.
///
/// Priority:
/// 1. `ANTHROPIC_API_KEY` env var
/// 2. `ANTHROPIC_AUTH_TOKEN` env var
/// 3. API key from system keychain (if `auth` feature enabled)
/// 4. `AuthMethod::None`
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

    // 3. Keychain
    if let Ok(Some(key)) = api_key::load_api_key() {
        if api_key::validate_api_key(&key) {
            return AuthMethod::ApiKey(key);
        }
    }

    AuthMethod::None
}

// ---------------------------------------------------------------------------
// OAuth login interface (not implemented)
// ---------------------------------------------------------------------------

/// OAuth login configuration — interface only.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
}

/// OAuth token pair — interface only.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
}

/// OAuth login flow — interface only, not implemented.
///
/// When implemented, this would:
/// 1. Generate PKCE code_verifier / code_challenge
/// 2. Open browser to authorization URL
/// 3. Listen on localhost for OAuth callback
/// 4. Exchange authorization code for tokens
/// 5. Store tokens via `token::save_token()`
#[allow(dead_code)]
pub async fn oauth_login(_config: &OAuthConfig) -> anyhow::Result<OAuthTokens> {
    anyhow::bail!("OAuth login is not implemented — use ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN")
}

/// OAuth token refresh — interface only, not implemented.
#[allow(dead_code)]
pub async fn oauth_refresh(_refresh_token: &str, _config: &OAuthConfig) -> anyhow::Result<OAuthTokens> {
    anyhow::bail!("OAuth refresh is not implemented — use ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN")
}

/// OAuth logout — interface only, not implemented.
///
/// When implemented, this would clear stored OAuth tokens from
/// keychain and disk.
#[allow(dead_code)]
pub async fn oauth_logout() -> anyhow::Result<()> {
    anyhow::bail!("OAuth logout is not implemented")
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
}
