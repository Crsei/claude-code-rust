#![allow(unused)]
//! OpenAI-compatible provider — handles all providers using the
//! OpenAI chat/completions API format.
//!
//! Covers: OpenAI, DeepSeek, Groq, OpenRouter, Qwen, Zhipu, Moonshot,
//! Baichuan, MiniMax, Yi, SiliconFlow, StepFun, Spark.
//!
//! Converts our internal MessagesRequest (Anthropic format) to OpenAI format,
//! sends the streaming request, and parses the SSE response back into our
//! StreamEvent type so the rest of the system (StreamAccumulator, etc.) works
//! unchanged.
//!
//! Reference: code-iris/crates/iris-llm/src/openai.rs

use std::pin::Pin;

use anyhow::{Context, Result};
use futures::Stream;
use serde_json::{json, Value};

use crate::api::client::MessagesRequest;
use crate::types::message::{ContentBlock, MessageDelta, StreamEvent, Usage};

// ---------------------------------------------------------------------------
// Message format conversion (Anthropic → OpenAI)
// ---------------------------------------------------------------------------

/// Extract text from Anthropic system prompt blocks.
///
/// System blocks look like: `[{"type": "text", "text": "Be helpful."}]`
fn extract_system_text(system: &[Value]) -> String {
    system
        .iter()
        .filter_map(|block| {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
            } else {
                block.as_str().map(|s| s.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Flatten Anthropic content (array of blocks or string) to a single string.
///
/// Anthropic: `{"content": [{"type":"text","text":"Hello"}]}` or `{"content": "Hello"}`
/// OpenAI:    `{"content": "Hello"}`
fn flatten_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()),
                Some("tool_result") => block.get("content").and_then(|c| c.as_str()).map(|s| s.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Convert a MessagesRequest (Anthropic format) to OpenAI chat completions body.
fn build_openai_request(request: &MessagesRequest) -> Value {
    let mut oai_messages: Vec<Value> = Vec::new();

    // System prompt → system message
    if let Some(system) = &request.system {
        let text = extract_system_text(system);
        if !text.is_empty() {
            oai_messages.push(json!({"role": "system", "content": text}));
        }
    }

    // User/assistant messages — flatten content blocks to plain text
    for msg in &request.messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = flatten_content(msg.get("content"));
        if !content.is_empty() {
            oai_messages.push(json!({"role": role, "content": content}));
        }
    }

    let mut body = json!({
        "model": request.model,
        "messages": oai_messages,
        "stream": true,
    });

    if request.max_tokens > 0 {
        body["max_tokens"] = json!(request.max_tokens);
    }

    body
}

// ---------------------------------------------------------------------------
// Streaming implementation (network feature required)
// ---------------------------------------------------------------------------

/// Send a streaming request to an OpenAI-compatible provider and return
/// a stream of `StreamEvent` compatible with `StreamAccumulator`.
pub(crate) async fn openai_compat_stream(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    provider_name: &str,
    request: &MessagesRequest,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = build_openai_request(request);

    tracing::debug!(
        provider = provider_name,
        url = %url,
        body = %serde_json::to_string_pretty(&body).unwrap_or_default(),
        "OpenAI-compat request"
    );

    let response = http
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to send request to {}", provider_name))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Provider {} error (HTTP {}): {}",
            provider_name,
            status,
            error_body
        );
    }

    let stream = parse_openai_sse_byte_stream(response.bytes_stream());
    Ok(Box::pin(stream))
}

/// Parse an OpenAI-format SSE byte stream into StreamEvent values.
///
/// OpenAI SSE format (no `event:` field, just `data:` lines):
/// ```text
/// data: {"choices":[{"delta":{"content":"Hello"}}]}
///
/// data: {"choices":[{"delta":{},"finish_reason":"stop"}]}
///
/// data: [DONE]
/// ```
///
/// Emits Anthropic-style StreamEvent sequence for compatibility with the
/// existing StreamAccumulator:
///
///   MessageStart → ContentBlockStart → ContentBlockDelta* →
///   ContentBlockStop → MessageDelta → MessageStop
fn parse_openai_sse_byte_stream<S>(byte_stream: S) -> impl Stream<Item = Result<StreamEvent>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures::StreamExt;

    async_stream::try_stream! {
        let mut byte_stream = std::pin::pin!(byte_stream);
        let mut buffer = String::new();
        let mut header_emitted = false;

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result.context("error reading OpenAI response chunk")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                let data = if let Some(rest) = line.strip_prefix("data:") {
                    rest.trim()
                } else {
                    continue;
                };

                // [DONE] marker
                if data == "[DONE]" {
                    if header_emitted {
                        yield StreamEvent::ContentBlockStop { index: 0 };
                        yield StreamEvent::MessageDelta {
                            delta: MessageDelta { stop_reason: Some("end_turn".to_string()) },
                            usage: None,
                        };
                    }
                    yield StreamEvent::MessageStop;
                    return;
                }

                let v: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Emit Anthropic-style header on first data chunk
                if !header_emitted {
                    header_emitted = true;
                    yield StreamEvent::MessageStart { usage: Usage::default() };
                    yield StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: ContentBlock::Text { text: String::new() },
                    };
                }

                // Process choices array
                if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        // Text delta
                        if let Some(content) = choice
                            .get("delta")
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            if !content.is_empty() {
                                yield StreamEvent::ContentBlockDelta {
                                    index: 0,
                                    delta: json!({"type": "text_delta", "text": content}),
                                };
                            }
                        }

                        // Finish reason
                        if let Some(reason) = choice
                            .get("finish_reason")
                            .and_then(|r| r.as_str())
                        {
                            let stop_reason = match reason {
                                "stop" => "end_turn",
                                "length" => "max_tokens",
                                other => other,
                            };
                            yield StreamEvent::ContentBlockStop { index: 0 };
                            yield StreamEvent::MessageDelta {
                                delta: MessageDelta {
                                    stop_reason: Some(stop_reason.to_string()),
                                },
                                usage: None,
                            };
                            yield StreamEvent::MessageStop;
                            return;
                        }
                    }
                }

                // Usage info (some providers include this in streamed chunks)
                if let Some(usage) = v.get("usage") {
                    let input = usage
                        .get("prompt_tokens")
                        .or_else(|| usage.get("input_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let output = usage
                        .get("completion_tokens")
                        .or_else(|| usage.get("output_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    if input > 0 || output > 0 {
                        yield StreamEvent::MessageDelta {
                            delta: MessageDelta { stop_reason: None },
                            usage: Some(Usage {
                                input_tokens: input,
                                output_tokens: output,
                                ..Usage::default()
                            }),
                        };
                    }
                }
            }
        }

        // Stream ended without [DONE] or finish_reason — still close properly
        if header_emitted {
            yield StreamEvent::ContentBlockStop { index: 0 };
            yield StreamEvent::MessageDelta {
                delta: MessageDelta { stop_reason: Some("end_turn".to_string()) },
                usage: None,
            };
        }
        yield StreamEvent::MessageStop;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_system_text_blocks() {
        let system = vec![
            json!({"type": "text", "text": "You are helpful."}),
            json!({"type": "text", "text": "Be concise."}),
        ];
        assert_eq!(extract_system_text(&system), "You are helpful.\nBe concise.");
    }

    #[test]
    fn test_extract_system_text_empty() {
        let system: Vec<Value> = vec![];
        assert_eq!(extract_system_text(&system), "");
    }

    #[test]
    fn test_flatten_content_string() {
        let content = Value::String("Hello world".to_string());
        assert_eq!(flatten_content(Some(&content)), "Hello world");
    }

    #[test]
    fn test_flatten_content_text_blocks() {
        let content = json!([
            {"type": "text", "text": "Hello"},
            {"type": "text", "text": " world"},
        ]);
        assert_eq!(flatten_content(Some(&content)), "Hello\n world");
    }

    #[test]
    fn test_flatten_content_none() {
        assert_eq!(flatten_content(None), "");
    }

    #[test]
    fn test_flatten_content_tool_result() {
        let content = json!([
            {"type": "tool_result", "tool_use_id": "id1", "content": "result text"},
        ]);
        assert_eq!(flatten_content(Some(&content)), "result text");
    }

    #[test]
    fn test_build_openai_request_basic() {
        let req = MessagesRequest {
            model: "gpt-4o".to_string(),
            messages: vec![json!({"role": "user", "content": "Hello"})],
            system: Some(vec![json!({"type": "text", "text": "Be helpful."})]),
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };
        let body = build_openai_request(&req);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 1024);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "Be helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Hello");
    }

    #[test]
    fn test_build_openai_request_no_system() {
        let req = MessagesRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![json!({"role": "user", "content": "Hi"})],
            system: None,
            max_tokens: 512,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };
        let body = build_openai_request(&req);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn test_build_openai_request_content_blocks() {
        let req = MessagesRequest {
            model: "gpt-4o".to_string(),
            messages: vec![json!({
                "role": "user",
                "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "text", "text": "World"},
                ]
            })],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
        };
        let body = build_openai_request(&req);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["content"], "Hello\nWorld");
    }
}
