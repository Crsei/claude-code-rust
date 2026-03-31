#![allow(unused)]
//! API client base — creates provider-specific HTTP clients
use std::pin::Pin;
use std::sync::Arc;
use anyhow::Result;
use futures::Stream;
use serde_json::Value;

use crate::types::message::{AssistantMessage, Message, StreamEvent, Usage};

/// API provider enum
#[derive(Debug, Clone)]
pub enum ApiProvider {
    /// Direct Anthropic API
    Anthropic { api_key: String, base_url: Option<String> },
    /// AWS Bedrock
    Bedrock { region: String, model_id: String },
    /// GCP Vertex AI
    Vertex { project_id: String, region: String },
    /// Azure Foundry
    Azure { endpoint: String, api_key: String },
}

/// Request body for the Messages API
#[derive(Debug, Clone, serde::Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Value>,
    pub system: Option<Vec<Value>>,
    pub max_tokens: usize,
    pub tools: Option<Vec<Value>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
}

/// API client configuration
#[derive(Debug, Clone)]
pub struct ApiClientConfig {
    pub provider: ApiProvider,
    pub default_model: String,
    pub max_retries: usize,
    pub timeout_secs: u64,
}

/// The API client — uses reqwest under the hood when 'network' feature is enabled
pub struct ApiClient {
    config: ApiClientConfig,
    #[cfg(feature = "network")]
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(config: ApiClientConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "network")]
            http: reqwest::Client::new(),
        }
    }

    /// Send a messages request and return the response as a stream of events
    ///
    /// When network feature is not enabled, returns an error
    pub async fn messages_stream(
        &self,
        request: MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        #[cfg(not(feature = "network"))]
        {
            anyhow::bail!("API client requires 'network' feature to be enabled")
        }

        #[cfg(feature = "network")]
        {
            // TODO: Implement actual HTTP streaming with SSE parsing
            anyhow::bail!("Streaming not yet implemented")
        }
    }

    /// Send a non-streaming messages request
    pub async fn messages(&self, request: MessagesRequest) -> Result<AssistantMessage> {
        #[cfg(not(feature = "network"))]
        {
            anyhow::bail!("API client requires 'network' feature to be enabled")
        }

        #[cfg(feature = "network")]
        {
            anyhow::bail!("Non-streaming messages not yet implemented")
        }
    }
}
