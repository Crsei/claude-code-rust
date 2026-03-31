#![allow(unused)]
//! API provider implementations
//!
//! Each provider transforms the generic MessagesRequest into
//! provider-specific HTTP requests.

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

// ── AWS Bedrock ──

pub struct BedrockProvider {
    pub region: String,
}

impl Provider for BedrockProvider {
    fn name(&self) -> &str { "bedrock" }
    fn base_url(&self) -> String {
        format!("https://bedrock-runtime.{}.amazonaws.com", self.region)
    }
    fn model_id(&self, model: &str) -> String {
        // Bedrock uses ARN-style model IDs
        format!("anthropic.{}", model)
    }
}

// ── GCP Vertex AI ──

pub struct VertexProvider {
    pub project_id: String,
    pub region: String,
}

impl Provider for VertexProvider {
    fn name(&self) -> &str { "vertex" }
    fn base_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models",
            self.region, self.project_id, self.region
        )
    }
    fn model_id(&self, model: &str) -> String { model.to_string() }
}
