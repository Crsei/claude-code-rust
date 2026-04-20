//! AWS Bedrock provider — routes Claude requests to AWS-managed Claude endpoints.
//!
//! Bedrock is one of several "third-party cloud" providers for Claude; this
//! module is a thin adaptation layer that reuses the existing streaming
//! architecture. It is NOT a separate product — it's an alternative transport
//! for the same Claude conversation loop.
//!
//! # Endpoint
//!
//! `POST https://bedrock-runtime.{region}.amazonaws.com/model/{model_id}/invoke`
//!
//! With `ANTHROPIC_BEDROCK_BASE_URL` set, the base is overridden (useful for
//! proxies / mock servers).
//!
//! # Authentication (MVP)
//!
//! Two modes supported:
//! - `AWS_BEARER_TOKEN_BEDROCK` (Bedrock API key) → `Authorization: Bearer ...`
//! - `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` (+ optional `AWS_SESSION_TOKEN`)
//!   → SigV4 request signing
//!
//! # Request body
//!
//! Bedrock expects the Anthropic Messages body with two differences:
//! - The `model` field is removed (model is in the URL).
//! - `anthropic_version: "bedrock-2023-05-31"` is required.
//! - `stream` is removed (endpoint suffix determines streaming).
//!
//! # Streaming (MVP note)
//!
//! MVP uses Bedrock's non-streaming `/invoke` endpoint and synthesizes a
//! stream of `StreamEvent`s from the single JSON response. True server-side
//! streaming via `invoke-with-response-stream` (AWS EventStream binary format)
//! is a Phase-2 enhancement. This satisfies the MVP goal of "basic Claude
//! conversation works" with the existing accumulator/stream pipeline
//! unchanged.

use std::pin::Pin;

use anyhow::{bail, Context, Result};
use futures::Stream;
use serde_json::{json, Value};

use crate::api::client::MessagesRequest;
use crate::api::model_mapping::to_bedrock_model_id;
use crate::api::sigv4::{self, AwsCredentials, SignRequest};
use crate::types::message::{ContentBlock, MessageDelta, StreamEvent, Usage};

pub const BEDROCK_ANTHROPIC_VERSION: &str = "bedrock-2023-05-31";

/// How the caller authenticates to Bedrock.
#[derive(Debug, Clone)]
pub enum BedrockAuth {
    /// Pre-issued Bedrock API key (`AWS_BEARER_TOKEN_BEDROCK`).
    BearerToken(String),
    /// Standard AWS credentials that will be used to SigV4-sign each request.
    AwsCredentials(AwsCredentials),
}

impl BedrockAuth {
    /// Resolve auth from the environment.
    ///
    /// Priority:
    /// 1. `AWS_BEARER_TOKEN_BEDROCK` (simpler; matches claude-code-bun).
    /// 2. `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` (SigV4).
    pub fn from_env() -> Option<Self> {
        if let Ok(tok) = std::env::var("AWS_BEARER_TOKEN_BEDROCK") {
            if !tok.is_empty() {
                return Some(Self::BearerToken(tok));
            }
        }
        AwsCredentials::from_env().map(Self::AwsCredentials)
    }
}

/// Resolve the AWS region for Bedrock.
///
/// Matches claude-code-bun: `AWS_REGION` → `AWS_DEFAULT_REGION`
/// → default `us-east-1`.
pub fn resolve_region() -> String {
    std::env::var("AWS_REGION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var("AWS_DEFAULT_REGION")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| "us-east-1".to_string())
}

/// Build the Bedrock invoke URL for a given model ID.
pub fn build_invoke_url(region: &str, model_id: &str, base_url_override: Option<&str>) -> String {
    let model_id_encoded = urlencoding::encode(model_id);
    if let Some(base) = base_url_override {
        let base = base.trim_end_matches('/');
        return format!("{}/model/{}/invoke", base, model_id_encoded);
    }
    format!(
        "https://bedrock-runtime.{}.amazonaws.com/model/{}/invoke",
        region, model_id_encoded
    )
}

/// Convert a `MessagesRequest` into the Bedrock-specific JSON body.
fn to_bedrock_body(request: &MessagesRequest) -> Result<Vec<u8>> {
    let mut body = json!({
        "anthropic_version": BEDROCK_ANTHROPIC_VERSION,
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
    serde_json::to_vec(&body).context("failed to serialize Bedrock request body")
}

/// Parsed `/invoke` response used to synthesize stream events.
struct InvokeResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: Usage,
}

fn parse_invoke_response(json_str: &str) -> Result<InvokeResponse> {
    let v: Value =
        serde_json::from_str(json_str).context("failed to parse Bedrock JSON response")?;

    let stop_reason = v
        .get("stop_reason")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let content: Vec<ContentBlock> = v
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|block| serde_json::from_value(block.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let usage: Usage = v
        .get("usage")
        .and_then(|u| serde_json::from_value(u.clone()).ok())
        .unwrap_or_default();

    Ok(InvokeResponse {
        content,
        stop_reason,
        usage,
    })
}

/// Turn a single invoke response into the equivalent sequence of
/// `StreamEvent`s, so downstream consumers see Bedrock as a streaming
/// provider even though MVP uses the non-streaming `/invoke` endpoint.
fn synthesize_stream_events(resp: InvokeResponse) -> Vec<StreamEvent> {
    let mut events = Vec::new();

    // Input tokens go on MessageStart; output tokens on MessageDelta.
    let mut start_usage = resp.usage.clone();
    start_usage.output_tokens = 0;
    events.push(StreamEvent::MessageStart { usage: start_usage });

    for (idx, block) in resp.content.iter().enumerate() {
        match block {
            ContentBlock::Text { text } => {
                events.push(StreamEvent::ContentBlockStart {
                    index: idx,
                    content_block: ContentBlock::Text {
                        text: String::new(),
                    },
                });
                if !text.is_empty() {
                    events.push(StreamEvent::ContentBlockDelta {
                        index: idx,
                        delta: json!({"type": "text_delta", "text": text}),
                    });
                }
                events.push(StreamEvent::ContentBlockStop { index: idx });
            }
            ContentBlock::Thinking {
                thinking,
                signature,
            } => {
                events.push(StreamEvent::ContentBlockStart {
                    index: idx,
                    content_block: ContentBlock::Thinking {
                        thinking: String::new(),
                        signature: signature.clone(),
                    },
                });
                if !thinking.is_empty() {
                    events.push(StreamEvent::ContentBlockDelta {
                        index: idx,
                        delta: json!({"type": "thinking_delta", "thinking": thinking}),
                    });
                }
                events.push(StreamEvent::ContentBlockStop { index: idx });
            }
            ContentBlock::ToolUse { id, name, input } => {
                events.push(StreamEvent::ContentBlockStart {
                    index: idx,
                    content_block: ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: json!({}),
                    },
                });
                events.push(StreamEvent::ContentBlockDelta {
                    index: idx,
                    delta: json!({
                        "type": "input_json_delta",
                        "partial_json": serde_json::to_string(input).unwrap_or_default(),
                    }),
                });
                events.push(StreamEvent::ContentBlockStop { index: idx });
            }
            other => {
                events.push(StreamEvent::ContentBlockStart {
                    index: idx,
                    content_block: other.clone(),
                });
                events.push(StreamEvent::ContentBlockStop { index: idx });
            }
        }
    }

    let mut end_usage = Usage::default();
    end_usage.output_tokens = resp.usage.output_tokens;
    end_usage.cache_creation_input_tokens = resp.usage.cache_creation_input_tokens;
    end_usage.cache_read_input_tokens = resp.usage.cache_read_input_tokens;
    events.push(StreamEvent::MessageDelta {
        delta: MessageDelta {
            stop_reason: resp.stop_reason,
        },
        usage: Some(end_usage),
    });
    events.push(StreamEvent::MessageStop);

    events
}

/// Bedrock stream provider (implements `StreamProvider`).
pub struct BedrockStreamProvider {
    pub region: String,
    pub auth: BedrockAuth,
    pub base_url_override: Option<String>,
}

#[async_trait::async_trait]
impl crate::api::stream_provider::StreamProvider for BedrockStreamProvider {
    async fn stream(
        &self,
        http: &reqwest::Client,
        request: &MessagesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let bedrock_model = to_bedrock_model_id(&request.model);
        let url = build_invoke_url(
            &self.region,
            &bedrock_model,
            self.base_url_override.as_deref(),
        );
        let body = to_bedrock_body(request)?;

        let mut builder = http
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .body(body.clone());

        match &self.auth {
            BedrockAuth::BearerToken(tok) => {
                builder = builder.header("authorization", format!("Bearer {}", tok));
            }
            BedrockAuth::AwsCredentials(creds) => {
                let parsed = url::Url::parse(&url).context("invalid Bedrock URL")?;
                let host = parsed.host_str().context("Bedrock URL missing host")?;
                let path = parsed.path().to_string();
                let (amz_date, date_stamp) = sigv4::current_timestamps();
                let signed = sigv4::sign(
                    &SignRequest {
                        method: "POST",
                        host,
                        path: &path,
                        region: &self.region,
                        service: "bedrock",
                        body: &body,
                        content_type: "application/json",
                        amz_date,
                        date_stamp,
                    },
                    creds,
                )?;
                builder = builder
                    .header("authorization", signed.authorization)
                    .header("x-amz-date", signed.x_amz_date)
                    .header("x-amz-content-sha256", signed.x_amz_content_sha256);
                if let Some(tok) = signed.x_amz_security_token {
                    builder = builder.header("x-amz-security-token", tok);
                }
            }
        }

        let response = builder
            .send()
            .await
            .context("failed to send Bedrock invoke request")?;
        let status = response.status();
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("(failed to read Bedrock response body)"));

        if !status.is_success() {
            bail!(
                "Bedrock invoke error (HTTP {}): {}",
                status.as_u16(),
                body_text
            );
        }

        let parsed = parse_invoke_response(&body_text)?;
        let events = synthesize_stream_events(parsed);

        let stream = futures::stream::iter(events.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_defaults_to_us_east_1() {
        let saved_r = std::env::var("AWS_REGION").ok();
        let saved_dr = std::env::var("AWS_DEFAULT_REGION").ok();
        std::env::remove_var("AWS_REGION");
        std::env::remove_var("AWS_DEFAULT_REGION");

        assert_eq!(resolve_region(), "us-east-1");

        if let Some(v) = saved_r {
            std::env::set_var("AWS_REGION", v);
        }
        if let Some(v) = saved_dr {
            std::env::set_var("AWS_DEFAULT_REGION", v);
        }
    }

    #[test]
    fn url_uses_region_and_model() {
        let url = build_invoke_url(
            "eu-west-1",
            "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
            None,
        );
        assert_eq!(
            url,
            "https://bedrock-runtime.eu-west-1.amazonaws.com/model/us.anthropic.claude-sonnet-4-5-20250929-v1%3A0/invoke"
        );
    }

    #[test]
    fn url_override_from_base_url() {
        let url = build_invoke_url("us-east-1", "foo", Some("https://proxy.example.com"));
        assert_eq!(url, "https://proxy.example.com/model/foo/invoke");
    }

    #[test]
    fn body_strips_stream_and_model_adds_anthropic_version() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![json!({"role":"user","content":"hi"})],
            system: None,
            max_tokens: 128,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };
        let raw = to_bedrock_body(&req).unwrap();
        let v: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["anthropic_version"], BEDROCK_ANTHROPIC_VERSION);
        assert_eq!(v["max_tokens"], 128);
        assert!(v.get("model").is_none(), "model must not be in body");
        assert!(v.get("stream").is_none(), "stream must not be in body");
    }

    #[test]
    fn parse_invoke_response_extracts_content() {
        let json = r#"{
            "id":"msg_123",
            "role":"assistant",
            "model":"claude-sonnet-4-5",
            "content":[{"type":"text","text":"Hello!"}],
            "stop_reason":"end_turn",
            "usage":{"input_tokens":10,"output_tokens":3}
        }"#;
        let resp = parse_invoke_response(json).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 3);
    }

    #[test]
    fn synthesize_events_yields_complete_stream() {
        let resp = InvokeResponse {
            content: vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 1,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        };
        let events = synthesize_stream_events(resp);
        assert!(matches!(
            events.first(),
            Some(StreamEvent::MessageStart { .. })
        ));
        assert!(matches!(events.last(), Some(StreamEvent::MessageStop)));
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ContentBlockDelta { .. })));
    }

    #[test]
    fn synthesized_stream_accumulates_to_text() {
        let resp = InvokeResponse {
            content: vec![ContentBlock::Text {
                text: "Hello, world!".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: Usage {
                input_tokens: 5,
                output_tokens: 3,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        };
        let events = synthesize_stream_events(resp);

        let mut acc = crate::api::streaming::StreamAccumulator::new();
        for ev in &events {
            acc.process_event(ev);
        }
        let msg = acc.build("claude-sonnet-4-5-20250929");
        assert_eq!(msg.content.len(), 1);
        if let ContentBlock::Text { text } = &msg.content[0] {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("expected Text block");
        }
        assert_eq!(msg.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 5);
        assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 3);
    }

    #[test]
    fn auth_from_env_prefers_bearer_token() {
        let saved_bearer = std::env::var("AWS_BEARER_TOKEN_BEDROCK").ok();
        let saved_ak = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let saved_sk = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
        std::env::set_var("AWS_BEARER_TOKEN_BEDROCK", "bedrock-key-123");
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIA");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "secret");

        match BedrockAuth::from_env() {
            Some(BedrockAuth::BearerToken(t)) => assert_eq!(t, "bedrock-key-123"),
            other => panic!("expected BearerToken, got {:?}", other),
        }

        std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
        std::env::remove_var("AWS_ACCESS_KEY_ID");
        std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        if let Some(v) = saved_bearer {
            std::env::set_var("AWS_BEARER_TOKEN_BEDROCK", v);
        }
        if let Some(v) = saved_ak {
            std::env::set_var("AWS_ACCESS_KEY_ID", v);
        }
        if let Some(v) = saved_sk {
            std::env::set_var("AWS_SECRET_ACCESS_KEY", v);
        }
    }
}
