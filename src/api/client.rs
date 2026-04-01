#![allow(unused)]
//! API client — creates provider-specific HTTP clients and drives the
//! Anthropic Messages API (streaming + non-streaming).
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::Stream;
use serde_json::Value;

use crate::types::message::{AssistantMessage, Message, StreamEvent, Usage};

// Re-export siblings for convenience within this module's tests.
use crate::api::providers::{ProviderInfo, ProviderProtocol};
use crate::api::retry::{categorize_api_error, retry_delay, ApiErrorCategory, RetryConfig};
use crate::api::streaming::{parse_sse_event, StreamAccumulator};

/// API provider enum — determines wire protocol and auth method.
#[derive(Debug, Clone)]
pub enum ApiProvider {
    /// Direct Anthropic API (native Messages API)
    Anthropic {
        api_key: String,
        base_url: Option<String>,
    },
    /// Azure Foundry (Anthropic-compatible)
    Azure {
        endpoint: String,
        api_key: String,
    },
    /// OpenAI-compatible provider (OpenAI, DeepSeek, Groq, Qwen, etc.)
    OpenAiCompat {
        name: String,
        api_key: String,
        base_url: String,
        default_model: String,
    },
    /// Google Gemini (streamGenerateContent API)
    Google {
        api_key: String,
        base_url: String,
    },
    /// AWS Bedrock (interface only — not implemented)
    #[allow(dead_code)]
    Bedrock {
        region: String,
        model_id: String,
    },
    /// GCP Vertex AI (interface only — not implemented)
    #[allow(dead_code)]
    Vertex {
        project_id: String,
        region: String,
    },
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

/// The API client — uses reqwest under the hood.
pub struct ApiClient {
    config: ApiClientConfig,
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(config: ApiClientConfig) -> Self {
        Self {
            http: {
                let builder = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(config.timeout_secs));
                builder.build().unwrap_or_else(|_| reqwest::Client::new())
            },
            config,
        }
    }

    /// Build the messages endpoint URL based on provider.
    ///
    /// Only used for Anthropic-format providers (Anthropic, Azure).
    /// OpenAI-compat and Google providers build their URLs internally.
    pub fn build_url(&self) -> String {
        match &self.config.provider {
            ApiProvider::Anthropic { base_url, .. } => {
                let base = base_url
                    .as_deref()
                    .unwrap_or("https://api.anthropic.com");
                let base = base.trim_end_matches('/');
                format!("{}/v1/messages", base)
            }
            ApiProvider::Azure { endpoint, .. } => {
                let endpoint = endpoint.trim_end_matches('/');
                format!("{}/v1/messages", endpoint)
            }
            ApiProvider::OpenAiCompat { base_url, .. } => {
                let base = base_url.trim_end_matches('/');
                format!("{}/chat/completions", base)
            }
            ApiProvider::Google { base_url, .. } => {
                base_url.clone()
            }
            ApiProvider::Bedrock { .. } => {
                unimplemented!("AWS Bedrock provider is not implemented")
            }
            ApiProvider::Vertex { .. } => {
                unimplemented!("GCP Vertex AI provider is not implemented")
            }
        }
    }

    /// Construct an `ApiClient` from a `ProviderInfo` and API key.
    pub fn from_provider_info(info: &ProviderInfo, api_key: &str) -> Self {
        let provider = match info.protocol {
            ProviderProtocol::Anthropic => ApiProvider::Anthropic {
                api_key: api_key.to_string(),
                base_url: Some(info.base_url.to_string()),
            },
            ProviderProtocol::OpenAiCompat => ApiProvider::OpenAiCompat {
                name: info.name.to_string(),
                api_key: api_key.to_string(),
                base_url: info.base_url.to_string(),
                default_model: info.default_model.to_string(),
            },
            ProviderProtocol::Google => ApiProvider::Google {
                api_key: api_key.to_string(),
                base_url: info.base_url.to_string(),
            },
        };
        Self::new(ApiClientConfig {
            provider,
            default_model: info.default_model.to_string(),
            max_retries: 3,
            timeout_secs: 120,
        })
    }

    /// Auto-detect provider from environment variables and construct an `ApiClient`.
    ///
    /// Returns `None` if no provider has an API key set.
    pub fn from_env() -> Option<Self> {
        let info = crate::api::providers::detect_provider()?;
        let api_key = std::env::var(info.env_key).ok()?;
        Some(Self::from_provider_info(info, &api_key))
    }

    /// Build the required HTTP headers for Anthropic-format providers.
    pub fn build_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        match &self.config.provider {
            ApiProvider::Anthropic { api_key, .. } | ApiProvider::Azure { api_key, .. } => {
                headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
                headers.insert(
                    "anthropic-beta",
                    HeaderValue::from_static(
                        "interleaved-thinking-2025-05-14,prompt-caching-2024-07-16",
                    ),
                );
                if let Ok(val) = HeaderValue::from_str(api_key) {
                    headers.insert("x-api-key", val);
                }
            }
            ApiProvider::OpenAiCompat { api_key, .. } => {
                let bearer = format!("Bearer {}", api_key);
                if let Ok(val) = HeaderValue::from_str(&bearer) {
                    headers.insert("Authorization", val);
                }
            }
            ApiProvider::Google { .. } => {
                // Google uses API key in URL query param, no auth header needed
            }
            _ => {}
        }

        headers
    }

    /// Header accessor as a simple map (works without network feature, for tests).
    pub fn build_headers_map(&self) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        map.insert("content-type".to_string(), "application/json".to_string());

        match &self.config.provider {
            ApiProvider::Anthropic { api_key, .. } | ApiProvider::Azure { api_key, .. } => {
                map.insert("anthropic-version".to_string(), "2023-06-01".to_string());
                map.insert(
                    "anthropic-beta".to_string(),
                    "interleaved-thinking-2025-05-14,prompt-caching-2024-07-16".to_string(),
                );
                map.insert("x-api-key".to_string(), api_key.clone());
            }
            ApiProvider::OpenAiCompat { api_key, .. } => {
                map.insert("Authorization".to_string(), format!("Bearer {}", api_key));
            }
            ApiProvider::Google { .. } => {}
            _ => {}
        }

        map
    }

    /// Send a messages request and return the response as a stream of events.
    ///
    /// Routes to provider-specific implementations:
    /// - Anthropic/Azure → Anthropic SSE format
    /// - OpenAI-compat  → OpenAI chat/completions SSE format
    /// - Google         → Gemini streamGenerateContent SSE format
    pub async fn messages_stream(
        &self,
        request: MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        use futures::StreamExt;

        // Route to provider-specific streaming implementation
        match &self.config.provider {
            ApiProvider::OpenAiCompat {
                name,
                api_key,
                base_url,
                ..
            } => {
                return crate::api::openai_compat::openai_compat_stream(
                    &self.http, base_url, api_key, name, &request,
                )
                .await;
            }
            ApiProvider::Google { api_key, base_url } => {
                return crate::api::google_provider::google_stream(
                    &self.http, base_url, api_key, &request,
                )
                .await;
            }
            _ => {} // Fall through to Anthropic-format logic
        }

        // ── Anthropic / Azure native SSE format ──

        let url = self.build_url();
        let headers = self.build_headers();

        let mut req_body = request;
        req_body.stream = true;

        let body_json =
            serde_json::to_string(&req_body).context("failed to serialize request body")?;

        let response = self
            .http
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

    /// Send a non-streaming messages request.
    ///
    /// Internally uses the streaming endpoint and collects all events via
    /// `StreamAccumulator`.
    pub async fn messages(&self, request: MessagesRequest) -> Result<AssistantMessage> {
        use futures::StreamExt;

        let stream = self.messages_stream(request).await?;
        let mut stream = std::pin::pin!(stream);

        let mut accumulator = StreamAccumulator::new();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    accumulator.process_event(&event);
                }
                Err(e) => {
                    // If an error occurs mid-stream, return what we have with
                    // an error marker — but first, if we have no content at all,
                    // propagate the error directly.
                    if accumulator.content_blocks.is_empty() {
                        return Err(e);
                    }
                    tracing::warn!("stream error mid-accumulation: {}", e);
                    break;
                }
            }
        }

        Ok(accumulator.build())
    }

    /// Send a streaming request with automatic retry on retryable errors.
    ///
    /// Retries with exponential backoff per `RetryConfig`. Returns the first
    /// successful stream, or the last error after exhausting retries.
    pub async fn messages_stream_with_retry(
        &self,
        request: MessagesRequest,
        retry_config: &RetryConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..=retry_config.max_retries {
            match self.messages_stream(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::warn!(
                        attempt = attempt,
                        max = retry_config.max_retries,
                        error = %err_msg,
                        "API request failed"
                    );

                    // Check if this error is retryable by looking for HTTP status
                    // in the error message. This is a heuristic — the structured
                    // error category is embedded in the bail! message.
                    let is_retryable = err_msg.contains("RateLimit")
                        || err_msg.contains("Overloaded")
                        || err_msg.contains("ServerError")
                        || err_msg.contains("HTTP 429")
                        || err_msg.contains("HTTP 500")
                        || err_msg.contains("HTTP 502")
                        || err_msg.contains("HTTP 503")
                        || err_msg.contains("HTTP 529");

                    if !is_retryable || attempt >= retry_config.max_retries {
                        return Err(e);
                    }

                    let delay = retry_delay(retry_config, attempt);
                    tracing::info!(
                        delay_ms = delay.as_millis() as u64,
                        "retrying after delay"
                    );
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("retry loop exhausted")))
    }

    /// Send a non-streaming request with automatic retry.
    pub async fn messages_with_retry(
        &self,
        request: MessagesRequest,
        retry_config: &RetryConfig,
    ) -> Result<AssistantMessage> {
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..=retry_config.max_retries {
            match self.messages(request.clone()).await {
                Ok(msg) => return Ok(msg),
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::warn!(
                        attempt = attempt,
                        max = retry_config.max_retries,
                        error = %err_msg,
                        "API request failed"
                    );

                    let is_retryable = err_msg.contains("RateLimit")
                        || err_msg.contains("Overloaded")
                        || err_msg.contains("ServerError")
                        || err_msg.contains("HTTP 429")
                        || err_msg.contains("HTTP 500")
                        || err_msg.contains("HTTP 502")
                        || err_msg.contains("HTTP 503")
                        || err_msg.contains("HTTP 529");

                    if !is_retryable || attempt >= retry_config.max_retries {
                        return Err(e);
                    }

                    let delay = retry_delay(retry_config, attempt);
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("retry loop exhausted")))
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// SSE byte-stream parser
// ---------------------------------------------------------------------------

/// Parse an incoming HTTP byte stream (from `reqwest`) into a stream of
/// `StreamEvent` values.
///
/// The SSE wire format looks like:
/// ```text
/// event: message_start
/// data: {"type":"message_start","message":{...}}
///
/// event: content_block_start
/// data: {"type":"content_block_start","index":0,"content_block":{...}}
///
/// ```
///
/// Events are separated by blank lines (`\n\n`). Each event may have
/// `event:` and `data:` fields. We buffer incoming bytes and split on
/// line boundaries, accumulating `event` and `data` fields until a blank
/// line triggers parsing.
fn parse_sse_byte_stream<S>(byte_stream: S) -> impl Stream<Item = Result<StreamEvent>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures::StreamExt;

    async_stream::try_stream! {
        let mut byte_stream = std::pin::pin!(byte_stream);
        let mut buffer = String::new();
        let mut current_event_type = String::new();
        let mut current_data = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result.context("error reading response chunk")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines from the buffer
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    // Blank line => dispatch accumulated event
                    if !current_event_type.is_empty() && !current_data.is_empty() {
                        if let Some(event) =
                            parse_sse_event(&current_event_type, &current_data)?
                        {
                            yield event;
                        }
                    }
                    current_event_type.clear();
                    current_data.clear();
                } else if let Some(rest) = line.strip_prefix("event:") {
                    current_event_type = rest.trim().to_string();
                } else if let Some(rest) = line.strip_prefix("data:") {
                    let data_part = rest.trim();
                    if !current_data.is_empty() {
                        current_data.push('\n');
                    }
                    current_data.push_str(data_part);
                }
                // Ignore other fields (id:, retry:, comments starting with ':')
            }
        }

        // Flush any remaining buffered event (in case the stream ends without
        // a trailing blank line)
        if !current_event_type.is_empty() && !current_data.is_empty() {
            if let Some(event) = parse_sse_event(&current_event_type, &current_data)? {
                yield event;
            }
        }
    }
}

/// Parse SSE-formatted text (for testing without network). Returns all events
/// found in the text.
pub fn parse_sse_text(text: &str) -> Result<Vec<StreamEvent>> {
    let mut events = Vec::new();
    let mut current_event_type = String::new();
    let mut current_data = String::new();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            // Blank line => dispatch
            if !current_event_type.is_empty() && !current_data.is_empty() {
                if let Some(event) = parse_sse_event(&current_event_type, &current_data)? {
                    events.push(event);
                }
            }
            current_event_type.clear();
            current_data.clear();
        } else if let Some(rest) = line.strip_prefix("event:") {
            current_event_type = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            let data_part = rest.trim();
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(data_part);
        }
    }

    // Flush trailing event
    if !current_event_type.is_empty() && !current_data.is_empty() {
        if let Some(event) = parse_sse_event(&current_event_type, &current_data)? {
            events.push(event);
        }
    }

    Ok(events)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn anthropic_config() -> ApiClientConfig {
        ApiClientConfig {
            provider: ApiProvider::Anthropic {
                api_key: "sk-test-key-123".to_string(),
                base_url: None,
            },
            default_model: "claude-sonnet-4-20250514".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        }
    }

    fn anthropic_config_custom_url() -> ApiClientConfig {
        ApiClientConfig {
            provider: ApiProvider::Anthropic {
                api_key: "sk-test-key-456".to_string(),
                base_url: Some("https://custom.api.example.com".to_string()),
            },
            default_model: "claude-sonnet-4-20250514".to_string(),
            max_retries: 2,
            timeout_secs: 30,
        }
    }

    // -----------------------------------------------------------------------
    // URL building
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_url_anthropic() {
        let client = ApiClient::new(anthropic_config());
        let url = client.build_url();
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_build_url_anthropic_custom_base() {
        let client = ApiClient::new(anthropic_config_custom_url());
        let url = client.build_url();
        assert_eq!(url, "https://custom.api.example.com/v1/messages");
    }

    #[test]
    fn test_build_url_anthropic_trailing_slash() {
        let config = ApiClientConfig {
            provider: ApiProvider::Anthropic {
                api_key: "key".to_string(),
                base_url: Some("https://example.com/".to_string()),
            },
            default_model: "model".to_string(),
            max_retries: 1,
            timeout_secs: 30,
        };
        let client = ApiClient::new(config);
        let url = client.build_url();
        assert_eq!(url, "https://example.com/v1/messages");
    }

    #[test]
    #[should_panic(expected = "AWS Bedrock provider is not implemented")]
    fn test_build_url_bedrock_not_implemented() {
        let config = ApiClientConfig {
            provider: ApiProvider::Bedrock {
                region: "us-east-1".to_string(),
                model_id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            },
            default_model: "claude-sonnet-4-20250514".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        };
        let client = ApiClient::new(config);
        let _ = client.build_url(); // should panic
    }

    #[test]
    #[should_panic(expected = "GCP Vertex AI provider is not implemented")]
    fn test_build_url_vertex_not_implemented() {
        let config = ApiClientConfig {
            provider: ApiProvider::Vertex {
                project_id: "my-project".to_string(),
                region: "us-central1".to_string(),
            },
            default_model: "claude-sonnet-4-20250514".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        };
        let client = ApiClient::new(config);
        let _ = client.build_url(); // should panic
    }

    #[test]
    fn test_build_url_azure() {
        let config = ApiClientConfig {
            provider: ApiProvider::Azure {
                endpoint: "https://my-azure-endpoint.com".to_string(),
                api_key: "az-key".to_string(),
            },
            default_model: "model".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        };
        let client = ApiClient::new(config);
        let url = client.build_url();
        assert_eq!(url, "https://my-azure-endpoint.com/v1/messages");
    }

    #[test]
    fn test_build_url_openai_compat() {
        let config = ApiClientConfig {
            provider: ApiProvider::OpenAiCompat {
                name: "deepseek".to_string(),
                api_key: "sk-test".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                default_model: "deepseek-chat".to_string(),
            },
            default_model: "deepseek-chat".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        };
        let client = ApiClient::new(config);
        let url = client.build_url();
        assert_eq!(url, "https://api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn test_build_url_openai_compat_trailing_slash() {
        let config = ApiClientConfig {
            provider: ApiProvider::OpenAiCompat {
                name: "openai".to_string(),
                api_key: "sk-test".to_string(),
                base_url: "https://api.openai.com/v1/".to_string(),
                default_model: "gpt-4o".to_string(),
            },
            default_model: "gpt-4o".to_string(),
            max_retries: 3,
            timeout_secs: 60,
        };
        let client = ApiClient::new(config);
        let url = client.build_url();
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    }

    // -----------------------------------------------------------------------
    // Header building
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_headers_has_required() {
        let client = ApiClient::new(anthropic_config());
        let headers = client.build_headers_map();

        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("x-api-key").unwrap(), "sk-test-key-123");
        assert!(headers.get("anthropic-beta").unwrap().contains("interleaved-thinking"));
        assert!(headers.get("anthropic-beta").unwrap().contains("prompt-caching"));
    }

    #[test]
    fn test_build_headers_azure_has_api_key() {
        let config = ApiClientConfig {
            provider: ApiProvider::Azure {
                endpoint: "https://azure.example.com".to_string(),
                api_key: "az-secret".to_string(),
            },
            default_model: "model".to_string(),
            max_retries: 1,
            timeout_secs: 30,
        };
        let client = ApiClient::new(config);
        let headers = client.build_headers_map();
        assert_eq!(headers.get("x-api-key").unwrap(), "az-secret");
    }

    #[test]
    fn test_build_headers_openai_compat_bearer() {
        let config = ApiClientConfig {
            provider: ApiProvider::OpenAiCompat {
                name: "openai".to_string(),
                api_key: "sk-my-key".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                default_model: "gpt-4o".to_string(),
            },
            default_model: "gpt-4o".to_string(),
            max_retries: 1,
            timeout_secs: 30,
        };
        let client = ApiClient::new(config);
        let headers = client.build_headers_map();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer sk-my-key");
        assert!(headers.get("x-api-key").is_none());
        assert!(headers.get("anthropic-version").is_none());
    }

    #[test]
    fn test_build_headers_google_no_auth_header() {
        let config = ApiClientConfig {
            provider: ApiProvider::Google {
                api_key: "AIza-test".to_string(),
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            },
            default_model: "gemini-2.0-flash".to_string(),
            max_retries: 1,
            timeout_secs: 30,
        };
        let client = ApiClient::new(config);
        let headers = client.build_headers_map();
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert!(headers.get("x-api-key").is_none());
        assert!(headers.get("Authorization").is_none());
    }

    #[test]
    fn test_build_headers_bedrock_no_api_key() {
        let config = ApiClientConfig {
            provider: ApiProvider::Bedrock {
                region: "us-east-1".to_string(),
                model_id: "model-id".to_string(),
            },
            default_model: "model".to_string(),
            max_retries: 1,
            timeout_secs: 30,
        };
        let client = ApiClient::new(config);
        let headers = client.build_headers_map();
        assert!(headers.get("x-api-key").is_none());
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
    }

    // -----------------------------------------------------------------------
    // from_provider_info
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_provider_info_anthropic() {
        use crate::api::providers::get_provider;
        let info = get_provider("anthropic").unwrap();
        let client = ApiClient::from_provider_info(info, "sk-test");
        assert!(matches!(client.config().provider, ApiProvider::Anthropic { .. }));
        assert_eq!(client.config().default_model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_from_provider_info_deepseek() {
        use crate::api::providers::get_provider;
        let info = get_provider("deepseek").unwrap();
        let client = ApiClient::from_provider_info(info, "sk-ds-key");
        match &client.config().provider {
            ApiProvider::OpenAiCompat { name, base_url, .. } => {
                assert_eq!(name, "deepseek");
                assert_eq!(base_url, "https://api.deepseek.com/v1");
            }
            _ => panic!("expected OpenAiCompat"),
        }
    }

    #[test]
    fn test_from_provider_info_google() {
        use crate::api::providers::get_provider;
        let info = get_provider("google").unwrap();
        let client = ApiClient::from_provider_info(info, "AIza-test");
        assert!(matches!(client.config().provider, ApiProvider::Google { .. }));
        assert_eq!(client.config().default_model, "gemini-2.0-flash");
    }

    // -----------------------------------------------------------------------
    // SSE line parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_sse_line_parsing_message_start() {
        let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}\n\
\n";

        let events = parse_sse_text(sse_text).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::MessageStart { usage } => {
                assert_eq!(usage.input_tokens, 100);
                assert_eq!(usage.output_tokens, 0);
            }
            other => panic!("expected MessageStart, got {:?}", other),
        }
    }

    #[test]
    fn test_sse_line_parsing_content_block_start() {
        let sse_text = "\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n";

        let events = parse_sse_text(sse_text).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(*index, 0);
            }
            other => panic!("expected ContentBlockStart, got {:?}", other),
        }
    }

    #[test]
    fn test_sse_line_parsing_multiple_events() {
        let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":50,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":10}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

        let events = parse_sse_text(sse_text).unwrap();
        assert_eq!(events.len(), 6);

        assert!(matches!(events[0], StreamEvent::MessageStart { .. }));
        assert!(matches!(events[1], StreamEvent::ContentBlockStart { .. }));
        assert!(matches!(events[2], StreamEvent::ContentBlockDelta { .. }));
        assert!(matches!(events[3], StreamEvent::ContentBlockStop { .. }));
        assert!(matches!(events[4], StreamEvent::MessageDelta { .. }));
        assert!(matches!(events[5], StreamEvent::MessageStop));
    }

    #[test]
    fn test_sse_line_parsing_ping_ignored() {
        let sse_text = "\
event: ping\n\
data: {}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

        let events = parse_sse_text(sse_text).unwrap();
        // ping should be ignored, only message_stop should come through
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], StreamEvent::MessageStop));
    }

    #[test]
    fn test_sse_line_parsing_accumulator_integration() {
        let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":42,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello, world!\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

        let events = parse_sse_text(sse_text).unwrap();
        let mut acc = StreamAccumulator::new();
        for event in &events {
            acc.process_event(event);
        }

        let msg = acc.build();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content.len(), 1);
        assert_eq!(msg.stop_reason.as_deref(), Some("end_turn"));

        if let crate::types::message::ContentBlock::Text { text } = &msg.content[0] {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("expected Text content block");
        }

        assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 42);
        assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 5);
    }

    #[test]
    fn test_sse_line_parsing_no_trailing_newline() {
        // SSE text without a trailing blank line should still parse
        let sse_text = "\
event: message_stop\n\
data: {\"type\":\"message_stop\"}";

        let events = parse_sse_text(sse_text).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], StreamEvent::MessageStop));
    }

    #[test]
    fn test_sse_line_parsing_empty_text() {
        let events = parse_sse_text("").unwrap();
        assert!(events.is_empty());
    }

    // -----------------------------------------------------------------------
    // MessagesRequest serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_messages_request_serialization() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![serde_json::json!({"role": "user", "content": "Hello"})],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4-20250514");
        assert_eq!(json["max_tokens"], 1024);
        assert_eq!(json["stream"], true);
        // thinking and tool_choice should be omitted when None
        assert!(json.get("thinking").is_none());
        assert!(json.get("tool_choice").is_none());
    }

    #[test]
    fn test_messages_request_with_thinking() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![],
            system: Some(vec![serde_json::json!({"type": "text", "text": "You are helpful."})]),
            max_tokens: 4096,
            tools: None,
            stream: true,
            thinking: Some(serde_json::json!({"type": "enabled", "budget_tokens": 2048})),
            tool_choice: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("thinking").is_some());
        assert_eq!(json["thinking"]["type"], "enabled");
        assert!(json.get("system").is_some());
    }
}
