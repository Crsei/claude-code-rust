//! StreamProvider trait — unified provider dispatch for streaming API calls.
//!
//! Each LLM provider (Anthropic, OpenAI-compatible, Google Gemini) implements
//! this trait. The `ApiClient` stores a `Box<dyn StreamProvider>` and dispatches
//! through it, eliminating match-based routing in `messages_stream()`.

use std::pin::Pin;

use anyhow::{Context, Result};
use futures::Stream;

use crate::api::client::{parse_sse_byte_stream, MessagesRequest};
use crate::api::retry::categorize_api_error;
use crate::types::message::StreamEvent;

/// Trait for provider-specific streaming implementations.
#[async_trait::async_trait]
pub trait StreamProvider: Send + Sync {
    /// Send a streaming request and return a stream of unified `StreamEvent`s.
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}

// ---------------------------------------------------------------------------
// Anthropic / Azure (native Messages API SSE)
// ---------------------------------------------------------------------------

pub struct AnthropicStreamProvider {
    pub api_key: String,
    pub base_url: String,
}

#[async_trait::async_trait]
impl StreamProvider for AnthropicStreamProvider {
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_static("interleaved-thinking-2025-05-14,prompt-caching-2024-07-16"),
        );
        if let Ok(val) = HeaderValue::from_str(&self.api_key) {
            headers.insert("x-api-key", val);
        }

        let mut req_body = request.clone();
        req_body.stream = true;

        let body_json =
            serde_json::to_string(&req_body).context("failed to serialize request body")?;

        let response = http
            .post(&url)
            .headers(headers)
            .body(body_json)
            .send()
            .await
            .context("failed to send HTTP request")?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("(failed to read error body)"));
            let category = categorize_api_error(status, &error_body);
            anyhow::bail!(
                "API error (HTTP {}): {:?} — {}",
                status,
                category,
                error_body
            );
        }

        let byte_stream = response.bytes_stream();
        let sse_stream = parse_sse_byte_stream(byte_stream);

        Ok(Box::pin(sse_stream))
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible (DeepSeek, Groq, Qwen, Azure OpenAI, etc.)
// ---------------------------------------------------------------------------

pub struct OpenAiCompatStreamProvider {
    pub name: String,
    pub api_key: String,
    pub base_url: String,
}

#[async_trait::async_trait]
impl StreamProvider for OpenAiCompatStreamProvider {
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        crate::api::openai_compat::openai_compat_stream(
            http,
            &self.base_url,
            &self.api_key,
            &self.name,
            request,
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Google Gemini (streamGenerateContent)
// ---------------------------------------------------------------------------

pub struct GoogleStreamProvider {
    pub api_key: String,
    pub base_url: String,
}

#[async_trait::async_trait]
impl StreamProvider for GoogleStreamProvider {
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        crate::api::google_provider::google_stream(http, &self.base_url, &self.api_key, request)
            .await
    }
}
