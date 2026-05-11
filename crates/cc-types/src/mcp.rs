use serde::{Deserialize, Serialize};

/// OAuth configuration for an MCP server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOAuthConfig {
    /// Public OAuth client identifier. When omitted, cc-rust uses a stable
    /// default public-client id (`cc-rust`).
    #[serde(default, rename = "clientId", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Loopback callback port to place in the OAuth redirect URI.
    #[serde(
        default,
        rename = "callbackPort",
        skip_serializing_if = "Option::is_none"
    )]
    pub callback_port: Option<u16>,
    /// Authorization-server metadata URL (RFC 8414). If absent, discovery tries
    /// the MCP resource metadata endpoint first and then the origin's default
    /// authorization-server metadata path.
    #[serde(
        default,
        rename = "authServerMetadataUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_server_metadata_url: Option<String>,
    /// OAuth scopes to request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
}
