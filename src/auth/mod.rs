#![allow(unused)]
//! Phase 10: Authentication system (network required) — Low Priority

pub mod api_key;
pub mod token;

/// Authentication method
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Direct API key (ANTHROPIC_API_KEY)
    ApiKey(String),
    /// OAuth access token (from console login)
    OAuthToken { access_token: String, refresh_token: Option<String>, expires_at: Option<i64> },
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
            Self::OAuthToken { access_token, .. } => Some(access_token),
            _ => None,
        }
    }
}

/// Resolve authentication from environment and keychain
pub fn resolve_auth() -> AuthMethod {
    // 1. Check ANTHROPIC_API_KEY env var
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            return AuthMethod::ApiKey(key);
        }
    }

    // 2. Check stored token (would use keyring crate when 'auth' feature enabled)
    // For now, return None
    AuthMethod::None
}
