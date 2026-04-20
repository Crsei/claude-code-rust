//! GCP Vertex AI provider — routes Claude requests to Google-managed Claude
//! endpoints.
//!
//! Like Bedrock, this is an adaptation layer, not a separate product. Vertex
//! natively streams SSE in the same format as the Anthropic API, so this
//! module can reuse the existing SSE parser directly.
//!
//! # Endpoint
//!
//! `POST https://{region}-aiplatform.googleapis.com/v1/projects/{project}
//!   /locations/{region}/publishers/anthropic/models/{model}:streamRawPredict`
//!
//! # Authentication (MVP)
//!
//! Access token resolved from first of:
//! 1. `CLAUDE_CODE_VERTEX_ACCESS_TOKEN` (explicit override, highest priority)
//! 2. `GOOGLE_OAUTH_ACCESS_TOKEN`
//! 3. `gcloud auth application-default print-access-token` subprocess
//!
//! Service-account JSON → JWT → token exchange is out of scope for MVP because
//! it requires an RSA signing crate. Users who rely on service accounts should
//! run `gcloud auth activate-service-account <sa>` first and this module will
//! pick up the resulting ADC token.
//!
//! # Project / region resolution
//!
//! Project ID (first non-empty wins):
//! - `ANTHROPIC_VERTEX_PROJECT_ID`
//! - `GOOGLE_CLOUD_PROJECT`
//! - `GCLOUD_PROJECT`
//!
//! Region (first non-empty wins):
//! - `CLOUD_ML_REGION`
//! - default `us-east5`

use std::pin::Pin;

use anyhow::{bail, Context, Result};
use futures::Stream;
use serde_json::{json, Value};

use crate::api::client::{parse_sse_byte_stream, MessagesRequest};
use crate::api::model_mapping::to_vertex_model_id;
use crate::api::retry::categorize_api_error;
use crate::types::message::StreamEvent;

pub const VERTEX_ANTHROPIC_VERSION: &str = "vertex-2023-10-16";
pub const DEFAULT_VERTEX_REGION: &str = "us-east5";

/// Pre-obtained OAuth access token used for Vertex calls.
#[derive(Debug, Clone)]
pub struct VertexAccessToken(pub String);

impl VertexAccessToken {
    /// Resolve an access token from the environment or `gcloud` CLI.
    ///
    /// Returns `None` if no source succeeds.
    pub fn from_env_or_gcloud() -> Option<Self> {
        if let Ok(t) = std::env::var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN") {
            if !t.is_empty() {
                return Some(Self(t));
            }
        }
        if let Ok(t) = std::env::var("GOOGLE_OAUTH_ACCESS_TOKEN") {
            if !t.is_empty() {
                return Some(Self(t));
            }
        }
        fetch_gcloud_token().map(Self)
    }
}

/// Invoke `gcloud auth application-default print-access-token` to obtain an
/// access token via Application Default Credentials.
fn fetch_gcloud_token() -> Option<String> {
    let output = std::process::Command::new("gcloud")
        .args([
            "auth",
            "application-default",
            "print-access-token",
            "--quiet",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Resolve the project ID from environment.
pub fn resolve_project_id() -> Option<String> {
    for var in [
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "GCLOUD_PROJECT",
    ] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// Resolve the region from environment, defaulting to `us-east5`.
pub fn resolve_region() -> String {
    std::env::var("CLOUD_ML_REGION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_VERTEX_REGION.to_string())
}

/// Build the Vertex `:streamRawPredict` URL for a given model.
pub fn build_stream_url(region: &str, project_id: &str, model_id: &str) -> String {
    format!(
        "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/anthropic/models/{model}:streamRawPredict",
        region = region,
        project = project_id,
        model = model_id,
    )
}

/// Convert a `MessagesRequest` into the Vertex-specific JSON body.
///
/// Same as Bedrock: strip `model` and `stream`; add `anthropic_version`.
fn to_vertex_body(request: &MessagesRequest) -> Result<Vec<u8>> {
    let mut body = json!({
        "anthropic_version": VERTEX_ANTHROPIC_VERSION,
        "max_tokens": request.max_tokens,
        "messages": request.messages.clone(),
    });
    if let Some(system) = &request.system {
        body["system"] = Value::Array(system.clone());
    }
    if let Some(tools) = &request.tools {
        body["tools"] = Value::Array(tools.clone());
    }
    if let Some(thinking) = &request.thinking {
        body["thinking"] = thinking.clone();
    }
    if let Some(tool_choice) = &request.tool_choice {
        body["tool_choice"] = tool_choice.clone();
    }
    serde_json::to_vec(&body).context("failed to serialize Vertex request body")
}

/// Vertex stream provider (implements `StreamProvider`).
pub struct VertexStreamProvider {
    pub region: String,
    pub project_id: String,
    pub access_token: VertexAccessToken,
}

#[async_trait::async_trait]
impl crate::api::stream_provider::StreamProvider for VertexStreamProvider {
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let vertex_model = to_vertex_model_id(&request.model);
        let url = build_stream_url(&self.region, &self.project_id, &vertex_model);
        let body = to_vertex_body(request)?;

        let response = http
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", self.access_token.0))
            .body(body)
            .send()
            .await
            .context("failed to send Vertex streamRawPredict request")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("(failed to read error body)"));
            let category = categorize_api_error(status.as_u16(), &error_body);
            bail!(
                "Vertex AI error (HTTP {}): {:?} — {}",
                status.as_u16(),
                category,
                error_body
            );
        }

        let byte_stream = response.bytes_stream();
        let sse_stream = parse_sse_byte_stream(byte_stream);
        Ok(Box::pin(sse_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_defaults_to_us_east5() {
        let saved = std::env::var("CLOUD_ML_REGION").ok();
        std::env::remove_var("CLOUD_ML_REGION");

        assert_eq!(resolve_region(), "us-east5");

        if let Some(v) = saved {
            std::env::set_var("CLOUD_ML_REGION", v);
        }
    }

    #[test]
    fn region_uses_cloud_ml_region() {
        let saved = std::env::var("CLOUD_ML_REGION").ok();
        std::env::set_var("CLOUD_ML_REGION", "europe-west4");

        assert_eq!(resolve_region(), "europe-west4");

        match saved {
            Some(v) => std::env::set_var("CLOUD_ML_REGION", v),
            None => std::env::remove_var("CLOUD_ML_REGION"),
        }
    }

    #[test]
    fn url_construction() {
        let url = build_stream_url("us-east5", "my-proj", "claude-sonnet-4-5@20250929");
        assert_eq!(
            url,
            "https://us-east5-aiplatform.googleapis.com/v1/projects/my-proj/locations/us-east5/publishers/anthropic/models/claude-sonnet-4-5@20250929:streamRawPredict"
        );
    }

    #[test]
    fn project_id_priority() {
        let saved_an = std::env::var("ANTHROPIC_VERTEX_PROJECT_ID").ok();
        let saved_gc = std::env::var("GOOGLE_CLOUD_PROJECT").ok();
        let saved_gcl = std::env::var("GCLOUD_PROJECT").ok();
        std::env::remove_var("ANTHROPIC_VERTEX_PROJECT_ID");
        std::env::remove_var("GOOGLE_CLOUD_PROJECT");
        std::env::remove_var("GCLOUD_PROJECT");

        // None set → None
        assert_eq!(resolve_project_id(), None);

        // Only fallback set
        std::env::set_var("GCLOUD_PROJECT", "gcloud-proj");
        assert_eq!(resolve_project_id(), Some("gcloud-proj".to_string()));

        // Higher precedence wins
        std::env::set_var("GOOGLE_CLOUD_PROJECT", "gcp-proj");
        assert_eq!(resolve_project_id(), Some("gcp-proj".to_string()));

        std::env::set_var("ANTHROPIC_VERTEX_PROJECT_ID", "ant-proj");
        assert_eq!(resolve_project_id(), Some("ant-proj".to_string()));

        // restore
        std::env::remove_var("ANTHROPIC_VERTEX_PROJECT_ID");
        std::env::remove_var("GOOGLE_CLOUD_PROJECT");
        std::env::remove_var("GCLOUD_PROJECT");
        if let Some(v) = saved_an {
            std::env::set_var("ANTHROPIC_VERTEX_PROJECT_ID", v);
        }
        if let Some(v) = saved_gc {
            std::env::set_var("GOOGLE_CLOUD_PROJECT", v);
        }
        if let Some(v) = saved_gcl {
            std::env::set_var("GCLOUD_PROJECT", v);
        }
    }

    #[test]
    fn body_strips_stream_and_model_adds_anthropic_version() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![json!({"role":"user","content":"hi"})],
            system: None,
            max_tokens: 256,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };
        let raw = to_vertex_body(&req).unwrap();
        let v: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
        assert_eq!(v["max_tokens"], 256);
        assert!(v.get("model").is_none());
        assert!(v.get("stream").is_none());
    }

    #[test]
    fn access_token_env_var_priority() {
        let saved_cc = std::env::var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN").ok();
        let saved_go = std::env::var("GOOGLE_OAUTH_ACCESS_TOKEN").ok();
        std::env::set_var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", "cc-token");
        std::env::set_var("GOOGLE_OAUTH_ACCESS_TOKEN", "go-token");

        let t = VertexAccessToken::from_env_or_gcloud().unwrap();
        assert_eq!(t.0, "cc-token");

        std::env::remove_var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN");
        let t = VertexAccessToken::from_env_or_gcloud().unwrap();
        assert_eq!(t.0, "go-token");

        std::env::remove_var("GOOGLE_OAUTH_ACCESS_TOKEN");
        if let Some(v) = saved_cc {
            std::env::set_var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", v);
        }
        if let Some(v) = saved_go {
            std::env::set_var("GOOGLE_OAUTH_ACCESS_TOKEN", v);
        }
    }
}
