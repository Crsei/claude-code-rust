//! OAuth HTTP client — token exchange, refresh, and API key creation.

use anyhow::{Context, Result};
use serde::Deserialize;

use super::config;

/// Response from the token endpoint.
#[derive(Debug, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    #[serde(default)]
    pub scope: String,
}

/// Response from the create_api_key endpoint.
#[derive(Debug, Deserialize)]
struct CreateApiKeyResponse {
    raw_key: String,
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    code: &str,
    code_verifier: &str,
    state: &str,
    redirect_uri: &str,
) -> Result<OAuthTokenResponse> {
    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": redirect_uri,
        "client_id": config::CLIENT_ID,
        "code_verifier": code_verifier,
        "state": state,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(config::TOKEN_URL)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to reach token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed ({}): {}", status, text);
    }

    resp.json::<OAuthTokenResponse>()
        .await
        .context("Failed to parse token response")
}

/// Refresh an OAuth token.
pub async fn refresh_token(
    refresh_tok: &str,
    scopes: &[&str],
) -> Result<OAuthTokenResponse> {
    let scope_str = scopes.join(" ");
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_tok,
        "client_id": config::CLIENT_ID,
        "scope": scope_str,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(config::TOKEN_URL)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to reach token endpoint for refresh")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token refresh failed ({}): {}", status, text);
    }

    resp.json::<OAuthTokenResponse>()
        .await
        .context("Failed to parse refresh response")
}

/// Create an API key using an OAuth access token (Console mode).
pub async fn create_api_key(access_token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(config::CREATE_API_KEY_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to reach create_api_key endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Create API key failed ({}): {}", status, text);
    }

    let parsed: CreateApiKeyResponse = resp
        .json()
        .await
        .context("Failed to parse create_api_key response")?;
    Ok(parsed.raw_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network — integration test
    async fn test_exchange_invalid_code_returns_error() {
        let result = exchange_code(
            "invalid_code",
            "fake_verifier",
            "fake_state",
            config::MANUAL_REDIRECT_URL,
        )
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("401") || err.contains("400") || err.contains("failed"),
            "Expected HTTP error, got: {}",
            err
        );
    }

    #[tokio::test]
    #[ignore] // Requires network — integration test
    async fn test_refresh_invalid_token_returns_error() {
        let result =
            refresh_token("invalid_refresh_token", config::CLAUDE_AI_SCOPES).await;
        assert!(result.is_err());
    }
}
