//! OAuth endpoint configuration and authorization URL construction.

/// OAuth login method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthMethod {
    /// Claude.ai subscriber (Bearer token mode)
    ClaudeAi,
    /// Console user (API Key creation mode)
    Console,
}

pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const AUTH_URL: &str = "https://platform.claude.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
pub const CREATE_API_KEY_URL: &str =
    "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
pub const MANUAL_REDIRECT_URL: &str = "https://platform.claude.com/oauth/code/callback";

pub const CLAUDE_AI_SCOPES: &[&str] = &[
    "user:profile",
    "user:inference",
    "user:sessions:claude_code",
    "user:mcp_servers",
    "user:file_upload",
];

pub const CONSOLE_SCOPES: &[&str] = &["org:create_api_key", "user:profile"];

/// Return the scopes for the given login method.
pub fn scopes_for(method: OAuthMethod) -> &'static [&'static str] {
    match method {
        OAuthMethod::ClaudeAi => CLAUDE_AI_SCOPES,
        OAuthMethod::Console => CONSOLE_SCOPES,
    }
}

/// Build the full authorization URL with PKCE parameters.
pub fn authorization_url(
    method: OAuthMethod,
    code_challenge: &str,
    state: &str,
) -> String {
    let scopes = scopes_for(method).join(" ");
    format!(
        "{}?code=true&client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        AUTH_URL,
        CLIENT_ID,
        urlencoding::encode(MANUAL_REDIRECT_URL),
        urlencoding::encode(&scopes),
        code_challenge,
        state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_ai_scopes() {
        let scopes = scopes_for(OAuthMethod::ClaudeAi);
        assert!(scopes.contains(&"user:inference"));
        assert!(!scopes.contains(&"org:create_api_key"));
    }

    #[test]
    fn test_console_scopes() {
        let scopes = scopes_for(OAuthMethod::Console);
        assert!(scopes.contains(&"org:create_api_key"));
        assert!(!scopes.contains(&"user:inference"));
    }

    #[test]
    fn test_authorization_url_contains_required_params() {
        let url = authorization_url(OAuthMethod::ClaudeAi, "test_challenge", "test_state");
        assert!(url.starts_with(AUTH_URL));
        assert!(url.contains("client_id=9d1c250a"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge=test_challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=test_state"));
        assert!(url.contains("redirect_uri="));
    }
}
