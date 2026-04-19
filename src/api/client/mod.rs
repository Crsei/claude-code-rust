//! API client — creates provider-specific HTTP clients and drives the
//! Anthropic Messages API (streaming + non-streaming).
use std::pin::Pin;

use anyhow::Result;
use futures::Stream;
use serde_json::Value;

use crate::types::message::{AssistantMessage, StreamEvent};

// Re-export siblings for convenience within this module's tests.
use crate::api::providers::{ProviderInfo, ProviderProtocol};
use crate::api::streaming::StreamAccumulator;

mod stream;
#[cfg(test)]
mod tests;

pub(crate) use stream::parse_sse_byte_stream;
#[cfg(test)]
use stream::parse_sse_text;

pub const OPENAI_CODEX_PROVIDER_NAME: &str = "openai-codex";
pub const OPENAI_CODEX_TOKEN_ENV: &str = "OPENAI_CODEX_AUTH_TOKEN";
pub const OPENAI_CODEX_BASE_URL_ENV: &str = "OPENAI_CODEX_BASE_URL";
pub const OPENAI_CODEX_MODEL_ENV: &str = "OPENAI_CODEX_MODEL";

pub(crate) fn build_openai_compat_url(base_url: &str, provider_name: &str) -> String {
    let endpoint = if provider_name.eq_ignore_ascii_case(OPENAI_CODEX_PROVIDER_NAME) {
        "/conversation"
    } else {
        "/chat/completions"
    };
    format!("{}{}", base_url.trim_end_matches('/'), endpoint)
}

/// API provider enum — determines wire protocol and auth method.
#[derive(Debug, Clone)]
pub enum ApiProvider {
    /// Direct Anthropic API (native Messages API)
    Anthropic {
        api_key: String,
        base_url: Option<String>,
    },
    /// Azure Foundry (Anthropic-compatible)
    #[allow(dead_code)]
    Azure { endpoint: String, api_key: String },
    /// OpenAI-compatible provider (OpenAI, DeepSeek, Groq, Qwen, etc.)
    OpenAiCompat {
        name: String,
        api_key: String,
        base_url: String,
        #[allow(dead_code)]
        default_model: String,
    },
    /// Google Gemini (streamGenerateContent API)
    Google { api_key: String, base_url: String },
    /// AWS Bedrock — Claude via AWS-managed endpoints.
    ///
    /// `base_url_override` is read from `ANTHROPIC_BEDROCK_BASE_URL` when set.
    Bedrock {
        region: String,
        auth: crate::api::bedrock::BedrockAuth,
        base_url_override: Option<String>,
    },
    /// GCP Vertex AI — Claude via Google-managed endpoints.
    Vertex {
        project_id: String,
        region: String,
        access_token: crate::api::vertex::VertexAccessToken,
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
    #[allow(dead_code)]
    pub max_retries: usize,
    pub timeout_secs: u64,
}

/// The API client — uses reqwest under the hood.
pub struct ApiClient {
    config: ApiClientConfig,
    http: reqwest::Client,
    stream_provider: Box<dyn crate::api::stream_provider::StreamProvider>,
}

/// Build the appropriate `StreamProvider` from an `ApiProvider`.
fn make_stream_provider(
    provider: &ApiProvider,
) -> Box<dyn crate::api::stream_provider::StreamProvider> {
    use crate::api::stream_provider::*;
    match provider {
        ApiProvider::OpenAiCompat {
            name,
            api_key,
            base_url,
            ..
        } => Box::new(OpenAiCompatStreamProvider {
            name: name.clone(),
            api_key: api_key.clone(),
            base_url: base_url.clone(),
        }),
        ApiProvider::Google { api_key, base_url } => Box::new(GoogleStreamProvider {
            api_key: api_key.clone(),
            base_url: base_url.clone(),
        }),
        ApiProvider::Anthropic { api_key, base_url } => Box::new(AnthropicStreamProvider {
            api_key: api_key.clone(),
            base_url: base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        }),
        ApiProvider::Azure { api_key, endpoint } => Box::new(AnthropicStreamProvider {
            api_key: api_key.clone(),
            base_url: endpoint.clone(),
        }),
        ApiProvider::Bedrock {
            region,
            auth,
            base_url_override,
        } => Box::new(crate::api::bedrock::BedrockStreamProvider {
            region: region.clone(),
            auth: auth.clone(),
            base_url_override: base_url_override.clone(),
        }),
        ApiProvider::Vertex {
            project_id,
            region,
            access_token,
        } => Box::new(crate::api::vertex::VertexStreamProvider {
            region: region.clone(),
            project_id: project_id.clone(),
            access_token: access_token.clone(),
        }),
    }
}

impl ApiClient {
    pub fn new(config: ApiClientConfig) -> Self {
        let stream_provider = make_stream_provider(&config.provider);
        Self {
            http: {
                let mut builder = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(config.timeout_secs));

                // Honor HTTPS_PROXY/HTTP_PROXY/ALL_PROXY explicitly so the
                // client works under TUN/fake-ip DNS hijacking (e.g. Clash TUN)
                // where the system DNS resolves API hosts to private IPs and
                // direct TLS handshakes fail with "unexpected EOF".
                if let Ok(proxy_url) = std::env::var("HTTPS_PROXY")
                    .or_else(|_| std::env::var("https_proxy"))
                    .or_else(|_| std::env::var("HTTP_PROXY"))
                    .or_else(|_| std::env::var("http_proxy"))
                    .or_else(|_| std::env::var("ALL_PROXY"))
                {
                    if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                        tracing::info!(proxy = %proxy_url, "using explicit HTTP proxy");
                        builder = builder.proxy(proxy);
                    }
                }

                builder.build().unwrap_or_else(|_| reqwest::Client::new())
            },
            stream_provider,
            config,
        }
    }

    /// Build the messages endpoint URL based on provider.
    ///
    /// Only used for Anthropic-format providers (Anthropic, Azure).
    /// OpenAI-compat and Google providers build their URLs internally.
    #[allow(dead_code)]
    pub fn build_url(&self) -> String {
        match &self.config.provider {
            ApiProvider::Anthropic { base_url, .. } => {
                let base = base_url.as_deref().unwrap_or("https://api.anthropic.com");
                let base = base.trim_end_matches('/');
                format!("{}/v1/messages", base)
            }
            ApiProvider::Azure { endpoint, .. } => {
                let endpoint = endpoint.trim_end_matches('/');
                format!("{}/v1/messages", endpoint)
            }
            ApiProvider::OpenAiCompat { name, base_url, .. } => {
                build_openai_compat_url(base_url, name)
            }
            ApiProvider::Google { base_url, .. } => base_url.clone(),
            ApiProvider::Bedrock {
                region,
                base_url_override,
                ..
            } => crate::api::bedrock::build_invoke_url(
                region,
                &crate::api::model_mapping::to_bedrock_model_id(&self.config.default_model),
                base_url_override.as_deref(),
            ),
            ApiProvider::Vertex {
                project_id, region, ..
            } => crate::api::vertex::build_stream_url(
                region,
                project_id,
                &crate::api::model_mapping::to_vertex_model_id(&self.config.default_model),
            ),
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
    /// Priority:
    /// 1. `CLAUDE_CODE_USE_BEDROCK=1` → AWS Bedrock (Claude)
    /// 2. `CLAUDE_CODE_USE_VERTEX=1`  → GCP Vertex AI (Claude)
    /// 3. First of the registered API-key providers (Anthropic, Azure, OpenAI, …)
    ///    that has its env var set.
    ///
    /// For Azure OpenAI, the base URL is read from `AZURE_BASE_URL` since it is
    /// deployment-specific (e.g. `https://<resource>.openai.azure.com/openai/v1/`).
    ///
    /// Returns `None` if no provider is configured.
    pub fn from_env() -> Option<Self> {
        // 1. CLAUDE_CODE_USE_BEDROCK / _VERTEX — third-party cloud providers
        //    checked BEFORE API-key providers, matching claude-code-bun.
        if is_env_truthy("CLAUDE_CODE_USE_BEDROCK") {
            return Self::from_bedrock_env();
        }
        if is_env_truthy("CLAUDE_CODE_USE_VERTEX") {
            return Self::from_vertex_env();
        }

        let info = crate::api::providers::detect_provider()?;
        let api_key = std::env::var(info.env_key).ok()?;

        // Azure OpenAI: override the placeholder base_url with AZURE_BASE_URL
        if info.name == "azure" {
            let base_url = std::env::var("AZURE_BASE_URL")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| info.base_url.to_string());
            let base_url = base_url.trim_end_matches('/').to_string();

            let provider = ApiProvider::OpenAiCompat {
                name: "azure".to_string(),
                api_key,
                base_url,
                default_model: info.default_model.to_string(),
            };
            return Some(Self::new(ApiClientConfig {
                provider,
                default_model: info.default_model.to_string(),
                max_retries: 3,
                timeout_secs: 120,
            }));
        }

        // OpenAI Codex: allow runtime base_url/model overrides.
        if info.name == OPENAI_CODEX_PROVIDER_NAME {
            let base_url = std::env::var(OPENAI_CODEX_BASE_URL_ENV)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| info.base_url.to_string())
                .trim_end_matches('/')
                .to_string();
            let default_model = std::env::var(OPENAI_CODEX_MODEL_ENV)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| info.default_model.to_string());

            let provider = ApiProvider::OpenAiCompat {
                name: info.name.to_string(),
                api_key,
                base_url,
                default_model: default_model.clone(),
            };
            return Some(Self::new(ApiClientConfig {
                provider,
                default_model,
                max_retries: 3,
                timeout_secs: 120,
            }));
        }

        Some(Self::from_provider_info(info, &api_key))
    }

    /// Construct an `ApiClient` for AWS Bedrock using environment variables.
    ///
    /// Honors (matching claude-code-bun):
    /// - `AWS_REGION` / `AWS_DEFAULT_REGION` — region selection
    /// - `AWS_BEARER_TOKEN_BEDROCK` — preferred auth (Bedrock API key)
    /// - `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN` — SigV4
    /// - `ANTHROPIC_BEDROCK_BASE_URL` — override the default endpoint
    ///
    /// Returns `None` if neither auth mode is available.
    pub fn from_bedrock_env() -> Option<Self> {
        let auth = crate::api::bedrock::BedrockAuth::from_env()?;
        let region = crate::api::bedrock::resolve_region();
        let base_url_override = std::env::var("ANTHROPIC_BEDROCK_BASE_URL")
            .ok()
            .filter(|v| !v.is_empty());
        let default_model = std::env::var("ANTHROPIC_MODEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());
        Some(Self::new(ApiClientConfig {
            provider: ApiProvider::Bedrock {
                region,
                auth,
                base_url_override,
            },
            default_model,
            max_retries: 3,
            timeout_secs: 120,
        }))
    }

    /// Construct an `ApiClient` for GCP Vertex AI using environment variables.
    ///
    /// Honors:
    /// - `CLOUD_ML_REGION` — region (default: `us-east5`)
    /// - `ANTHROPIC_VERTEX_PROJECT_ID` / `GOOGLE_CLOUD_PROJECT` / `GCLOUD_PROJECT` — project ID
    /// - `CLAUDE_CODE_VERTEX_ACCESS_TOKEN` / `GOOGLE_OAUTH_ACCESS_TOKEN` — access token
    ///   (falls back to `gcloud auth application-default print-access-token` subprocess)
    ///
    /// Returns `None` if project ID or access token can't be resolved.
    pub fn from_vertex_env() -> Option<Self> {
        let project_id = crate::api::vertex::resolve_project_id()?;
        let region = crate::api::vertex::resolve_region();
        let access_token = crate::api::vertex::VertexAccessToken::from_env_or_gcloud()?;
        let default_model = std::env::var("ANTHROPIC_MODEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());
        Some(Self::new(ApiClientConfig {
            provider: ApiProvider::Vertex {
                project_id,
                region,
                access_token,
            },
            default_model,
            max_retries: 3,
            timeout_secs: 120,
        }))
    }

    /// Construct an `ApiClient` for the OpenAI Codex provider.
    ///
    /// Auth source (in priority order):
    /// - `OPENAI_CODEX_AUTH_TOKEN`
    /// - OAuth token saved by `/login 4`
    ///
    /// Optional:
    /// - `OPENAI_CODEX_BASE_URL` (default: https://chatgpt.com/backend-api)
    /// - `OPENAI_CODEX_MODEL` (default: gpt-5.4)
    pub fn from_codex_auth() -> Option<Self> {
        let info = crate::api::providers::get_provider(OPENAI_CODEX_PROVIDER_NAME)?;
        let api_key = crate::auth::resolve_codex_auth_token()?;

        let base_url = std::env::var(OPENAI_CODEX_BASE_URL_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| info.base_url.to_string())
            .trim_end_matches('/')
            .to_string();
        let default_model = std::env::var(OPENAI_CODEX_MODEL_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| info.default_model.to_string());

        Some(Self::new(ApiClientConfig {
            provider: ApiProvider::OpenAiCompat {
                name: info.name.to_string(),
                api_key,
                base_url,
                default_model: default_model.clone(),
            },
            default_model,
            max_retries: 3,
            timeout_secs: 120,
        }))
    }

    /// Construct an `ApiClient` for a specific backend.
    ///
    /// - `codex` backend: force the OpenAI Codex auth path.
    /// - other backends: use the standard auth chain.
    pub fn from_backend(backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(crate::engine::codex_exec::is_codex_backend) {
            return Self::from_codex_auth();
        }
        Self::from_auth()
    }

    /// Construct an `ApiClient` using the full auth resolution chain.
    ///
    /// Resolution order:
    /// 1. Multi-provider environment variable detection (Anthropic, OpenAI, Google, etc.)
    /// 2. `ANTHROPIC_AUTH_TOKEN` environment variable
    /// 3. API key from system keychain
    ///
    /// Returns `None` if no authentication is available.
    pub fn from_auth() -> Option<Self> {
        // 1. Try multi-provider env detection
        if let Some(client) = Self::from_env() {
            return Some(client);
        }

        // 2. Fall back to auth::resolve_auth() (keychain, external token, OAuth)
        let auth = crate::auth::resolve_auth();
        let api_key = auth
            .api_key()
            .or_else(|| auth.bearer_token())
            .map(|s| s.to_string())?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
        Some(Self::new(ApiClientConfig {
            provider: ApiProvider::Anthropic { api_key, base_url },
            default_model: "claude-sonnet-4-20250514".to_string(),
            max_retries: 3,
            timeout_secs: 120,
        }))
    }

    /// Build the required HTTP headers for Anthropic-format providers.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    /// Delegates to the provider-specific `StreamProvider` implementation
    /// (Anthropic, OpenAI-compat, or Google Gemini).
    pub async fn messages_stream(
        &self,
        request: MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        self.stream_provider.stream(&self.http, &request).await
    }

    /// Send a non-streaming messages request.
    ///
    /// Internally uses the streaming endpoint and collects all events via
    /// `StreamAccumulator`.
    #[allow(dead_code)]
    pub async fn messages(&self, request: MessagesRequest) -> Result<AssistantMessage> {
        use futures::StreamExt;

        let model = request.model.clone();
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

        Ok(accumulator.build(&model))
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }
}

/// Return true if env var `name` is set to a truthy value (`1`, `true`, `yes`,
/// `on`). Matches claude-code-bun's `isEnvTruthy` semantics.
pub(crate) fn is_env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}
