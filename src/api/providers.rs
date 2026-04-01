//! API provider abstractions
//!
//! Active: Anthropic Direct, Azure Foundry
//! Interface only: AWS Bedrock, GCP Vertex AI (not implemented)

/// Provider trait for abstracting API differences
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn base_url(&self) -> String;
    fn model_id(&self, model: &str) -> String;
}

// ── Anthropic Direct ──

pub struct AnthropicProvider {
    pub api_key: String,
    pub base_url: String,
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
        }
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }
    fn base_url(&self) -> String { self.base_url.clone() }
    fn model_id(&self, model: &str) -> String { model.to_string() }
}

// ── AWS Bedrock (interface only) ──

#[allow(dead_code)]
pub struct BedrockProvider {
    pub region: String,
    pub model_id: String,
}

#[allow(dead_code)]
impl BedrockProvider {
    pub fn new(region: &str, model_id: &str) -> Self {
        Self {
            region: region.to_string(),
            model_id: model_id.to_string(),
        }
    }

    /// Build Bedrock endpoint URL.
    pub fn endpoint_url(&self) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/invoke-with-response-stream",
            self.region, self.model_id
        )
    }

    /// Sign request with AWS SigV4 — not implemented.
    pub async fn sign_request(&self, _body: &[u8]) -> anyhow::Result<std::collections::HashMap<String, String>> {
        anyhow::bail!("AWS Bedrock SigV4 signing is not implemented")
    }

    /// Resolve AWS credentials from environment/config — not implemented.
    pub async fn resolve_credentials(&self) -> anyhow::Result<()> {
        anyhow::bail!("AWS Bedrock credential resolution is not implemented")
    }
}

// ── GCP Vertex AI (interface only) ──

#[allow(dead_code)]
pub struct VertexProvider {
    pub project_id: String,
    pub region: String,
}

#[allow(dead_code)]
impl VertexProvider {
    pub fn new(project_id: &str, region: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            region: region.to_string(),
        }
    }

    /// Build Vertex AI endpoint URL.
    pub fn endpoint_url(&self, model: &str) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:streamRawPredict",
            self.region, self.project_id, self.region, model
        )
    }

    /// Obtain GCP OAuth access token — not implemented.
    pub async fn get_access_token(&self) -> anyhow::Result<String> {
        anyhow::bail!("GCP Vertex AI authentication is not implemented")
    }

    /// Build auth headers with bearer token — not implemented.
    pub async fn auth_headers(&self) -> anyhow::Result<std::collections::HashMap<String, String>> {
        anyhow::bail!("GCP Vertex AI auth headers are not implemented")
    }
}
