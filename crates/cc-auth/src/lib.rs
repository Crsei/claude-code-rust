//! Authentication system
//!
//! Supports three active auth methods:
//! - API Key: via `ANTHROPIC_API_KEY` env var or system keychain
//! - External Auth Token: via `ANTHROPIC_AUTH_TOKEN` env var
//! - OAuth Token: from the registered credentials path (Claude.ai / Console / OpenAI Codex)

pub mod api_key;
pub mod codex_cli;
pub mod oauth;
pub mod token;

const OPENAI_CODEX_AUTH_TOKEN_ENV: &str = "OPENAI_CODEX_AUTH_TOKEN";

// ---------------------------------------------------------------------------
// Host-provided credentials path
// ---------------------------------------------------------------------------
//
// cc-auth used to call `crate::config::paths::credentials_path()` directly
// from `token.rs`. That's a cycle the moment `auth` moves out of the root
// crate, so the host now registers the path once at startup and cc-auth reads
// it back through this module.

use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::LazyLock;

static CREDENTIALS_PATH: LazyLock<RwLock<Option<PathBuf>>> = LazyLock::new(|| RwLock::new(None));

/// Register the OAuth credentials file path. The host calls this once during
/// process startup; if a caller reaches token I/O without it having run
/// (e.g. a unit test that exercises `resolve_auth` directly), the fallback
/// in [`credentials_path`] mirrors the root crate's
/// `config::paths::credentials_path()` layout.
pub fn set_credentials_path(path: PathBuf) {
    *CREDENTIALS_PATH.write() = Some(path);
}

/// Return the registered credentials path, falling back to
/// `{CC_RUST_HOME | ~/.cc-rust | $TMP/cc-rust}/credentials.json` when the host
/// hasn't registered one. Kept in sync with `config::paths::data_root` in the
/// root crate — a small duplication that decouples cc-auth from it.
pub(crate) fn credentials_path() -> PathBuf {
    if let Some(p) = CREDENTIALS_PATH.read().clone() {
        return p;
    }
    data_root_fallback().join("credentials.json")
}

fn data_root_fallback() -> PathBuf {
    if let Ok(override_dir) = std::env::var("CC_RUST_HOME") {
        if !override_dir.trim().is_empty() {
            return PathBuf::from(override_dir);
        }
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".cc-rust");
    }
    std::env::temp_dir().join("cc-rust")
}

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
        /// "claude_ai", "console", or "openai_codex"
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
        if method == "console" || method == "openai_codex" {
            // Console mode: API key is in keychain (created at login).
            // OpenAI Codex mode: handled by resolve_codex_auth_token().
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

/// Resolve OpenAI Codex auth token.
///
/// Priority:
/// 1. `OPENAI_CODEX_AUTH_TOKEN` environment variable
/// 2. OAuth token from `~/.cc-rust/credentials.json` when method is `openai_codex`
/// 3. Codex CLI credentials from `~/.codex/auth.json` (fallback)
pub fn resolve_codex_auth_token() -> Option<String> {
    // 1. Environment variable
    if let Ok(token) = std::env::var(OPENAI_CODEX_AUTH_TOKEN_ENV) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // 2. cc-rust credentials.json
    if let Some(token) = try_resolve_codex_from_credentials() {
        return Some(token);
    }

    // 3. Codex CLI fallback (~/.codex/auth.json)
    try_resolve_codex_cli()
}

/// Try to resolve Codex token from cc-rust's own `credentials.json`.
fn try_resolve_codex_from_credentials() -> Option<String> {
    let stored = token::load_token().ok().flatten()?;
    let method = stored.oauth_method.clone().unwrap_or_default();
    if !method.eq_ignore_ascii_case("openai_codex") {
        return None;
    }

    if !token::is_token_expired(&stored) {
        return Some(stored.access_token);
    }

    let refresh_tok = match &stored.refresh_token {
        Some(t) if !t.trim().is_empty() => t.clone(),
        _ => {
            let _ = token::remove_token();
            return None;
        }
    };

    let scopes: Vec<String> = stored.scopes.clone();
    match try_refresh_sync(&refresh_tok, &scopes, &stored) {
        Ok(Some((access_token, refreshed_method))) if refreshed_method == "openai_codex" => {
            Some(access_token)
        }
        Ok(Some(_)) | Ok(None) => {
            let _ = token::remove_token();
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "OpenAI Codex OAuth auto-refresh failed");
            let _ = token::remove_token();
            None
        }
    }
}

/// Try to resolve Codex token from Codex CLI's `~/.codex/auth.json`.
///
/// If the token is expired, attempt refresh using the Codex CLI client_id
/// and save the refreshed token to cc-rust's `credentials.json`.
fn try_resolve_codex_cli() -> Option<String> {
    let cred = codex_cli::read_codex_cli_credential()?;

    if !codex_cli::is_credential_expired(&cred) {
        return Some(cred.access_token);
    }

    // Token expired — try to refresh
    let refresh_tok = match &cred.refresh_token {
        Some(t) if !t.trim().is_empty() => t.clone(),
        _ => return None,
    };

    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return None,
    };

    let client_id = cred.client_id.clone();
    let token_url = oauth::config::token_url_for(oauth::config::OAuthMethod::OpenAiCodex);
    let scopes_owned: Vec<String> =
        oauth::config::resolved_scopes_for(oauth::config::OAuthMethod::OpenAiCodex);
    let refresh_tok_for_fallback = refresh_tok.clone();

    let result = std::thread::spawn(move || {
        handle.block_on(async {
            let scope_strs: Vec<&str> = scopes_owned.iter().map(|s| s.as_str()).collect();
            oauth::client::refresh_token_with_client_id(
                &client_id,
                &token_url,
                &refresh_tok,
                &scope_strs,
            )
            .await
        })
    })
    .join()
    .ok()?
    .ok()?;

    // Save refreshed token to cc-rust's credentials.json
    let expires_at = chrono::Utc::now().timestamp() + result.expires_in as i64;
    let new_scopes: Vec<String> = result
        .scope
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let stored = token::StoredToken {
        access_token: result.access_token.clone(),
        refresh_token: result.refresh_token.or(Some(refresh_tok_for_fallback)),
        expires_at: Some(expires_at),
        token_type: "bearer".into(),
        scopes: if new_scopes.is_empty() {
            oauth::config::OPENAI_CODEX_SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            new_scopes
        },
        oauth_method: Some("openai_codex".to_string()),
    };
    let _ = token::save_token(&stored);
    tracing::info!("Codex CLI token refreshed and saved to cc-rust credentials");
    Some(result.access_token)
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
    let oauth_method = stored
        .oauth_method
        .as_deref()
        .and_then(oauth::config::method_from_storage_name)
        .unwrap_or(oauth::config::OAuthMethod::ClaudeAi);

    let scope_strs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
    let scopes_ref: &[&str] = if scope_strs.is_empty() {
        oauth::config::scopes_for(oauth_method)
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
            match oauth::client::refresh_token(oauth_method, &refresh_tok, &scope_strs).await {
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
