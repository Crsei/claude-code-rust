//! Google Gemini provider — streamGenerateContent API.
//!
//! Auth: `GOOGLE_API_KEY` passed as `?key=` query parameter.
//! Endpoint: `{base}/models/{model}:streamGenerateContent?key=...&alt=sse`
//!
//! Response format differs from OpenAI — each SSE chunk is a full JSON object
//! (not delta-encoded), so we diff against the previously emitted text to
//! produce proper text deltas.
//!
//! Reference: code-iris/crates/iris-llm/src/google.rs

use std::pin::Pin;

use anyhow::{Context, Result};
use futures::Stream;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::client::MessagesRequest;
use crate::types::message::{ContentBlock, MessageDelta, StreamEvent, Usage};

// ---------------------------------------------------------------------------
// Gemini response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: u64,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: u64,
}

// ---------------------------------------------------------------------------
// Message format conversion (Anthropic → Gemini)
// ---------------------------------------------------------------------------

/// Convert MessagesRequest (Anthropic format) to Gemini request body.
///
/// Key differences:
/// - Gemini uses "model" role instead of "assistant"
/// - System prompt goes in `system_instruction` field
/// - Consecutive same-role messages must be merged (Gemini requires alternating turns)
fn build_gemini_request(request: &MessagesRequest) -> Value {
    let mut contents: Vec<Value> = Vec::new();

    for msg in &request.messages {
        let role_str = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let role = match role_str {
            "assistant" => "model",
            _ => "user",
        };

        let text = match msg.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(blocks)) => blocks
                .iter()
                .filter_map(|b| match b.get("type").and_then(|t| t.as_str()) {
                    Some("text") => b
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string()),
                    Some("tool_result") => b
                        .get("content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };

        if !text.is_empty() {
            contents.push(json!({
                "role": role,
                "parts": [{"text": text}]
            }));
        }
    }

    // Merge consecutive same-role messages (Gemini requires alternating turns)
    let mut merged: Vec<Value> = Vec::new();
    for item in contents {
        let role = item["role"].as_str().unwrap_or("").to_string();
        let text = item["parts"][0]["text"].as_str().unwrap_or("").to_string();
        if let Some(last) = merged.last_mut() {
            if last["role"].as_str() == Some(&role) {
                let prev = last["parts"][0]["text"].as_str().unwrap_or("").to_string();
                last["parts"][0]["text"] = json!(format!("{prev}\n{text}"));
                continue;
            }
        }
        merged.push(item);
    }

    let mut body = json!({
        "contents": merged,
        "generationConfig": {
            "maxOutputTokens": request.max_tokens,
        }
    });

    // System instruction
    if let Some(system) = &request.system {
        let system_text: String = system
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    block
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                } else {
                    block.as_str().map(|s| s.to_string())
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !system_text.is_empty() {
            body["system_instruction"] = json!({
                "parts": [{"text": system_text}]
            });
        }
    }

    body
}

// ---------------------------------------------------------------------------
// Streaming implementation
// ---------------------------------------------------------------------------

/// Send a streaming request to Google Gemini API.
pub(crate) async fn google_stream(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    request: &MessagesRequest,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
    let url = format!(
        "{}/models/{}:streamGenerateContent?key={}&alt=sse",
        base_url.trim_end_matches('/'),
        request.model,
        api_key,
    );

    let body = build_gemini_request(request);

    let response = http
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("failed to send Google Gemini request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        anyhow::bail!("Google Gemini error (HTTP {}): {}", status, error_body);
    }

    let stream = parse_gemini_sse_byte_stream(response.bytes_stream());
    Ok(Box::pin(stream))
}

/// Parse a Gemini SSE byte stream into StreamEvent values.
///
/// Gemini returns **cumulative** text per chunk (not delta-encoded), so we
/// diff against the previously emitted text to produce proper text deltas.
///
/// Emits Anthropic-style StreamEvent sequence:
///   MessageStart → ContentBlockStart → ContentBlockDelta* →
///   ContentBlockStop → MessageDelta → MessageStop
fn parse_gemini_sse_byte_stream<S>(byte_stream: S) -> impl Stream<Item = Result<StreamEvent>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures::StreamExt;

    async_stream::try_stream! {
        let mut byte_stream = std::pin::pin!(byte_stream);
        let mut buffer = String::new();
        let mut emitted_text = String::new();
        let mut header_emitted = false;

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result.context("error reading Gemini response chunk")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

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

                if data == "[DONE]" || data.is_empty() {
                    continue;
                }

                let gemini_resp: GeminiResponse = match serde_json::from_str(data) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                // Emit header events on first valid chunk
                if !header_emitted {
                    header_emitted = true;

                    let usage = if let Some(ref u) = gemini_resp.usage_metadata {
                        Usage {
                            input_tokens: u.prompt_token_count,
                            output_tokens: u.candidates_token_count,
                            ..Usage::default()
                        }
                    } else {
                        Usage::default()
                    };

                    yield StreamEvent::MessageStart { usage };
                    yield StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: ContentBlock::Text { text: String::new() },
                    };
                }

                // Emit usage updates
                if let Some(ref u) = gemini_resp.usage_metadata {
                    if u.candidates_token_count > 0 {
                        yield StreamEvent::MessageDelta {
                            delta: MessageDelta { stop_reason: None },
                            usage: Some(Usage {
                                input_tokens: u.prompt_token_count,
                                output_tokens: u.candidates_token_count,
                                ..Usage::default()
                            }),
                        };
                    }
                }

                // Process candidates
                for candidate in gemini_resp.candidates.unwrap_or_default() {
                    // Check finish reason first
                    if candidate.finish_reason.as_deref() == Some("STOP") {
                        yield StreamEvent::ContentBlockStop { index: 0 };
                        yield StreamEvent::MessageDelta {
                            delta: MessageDelta {
                                stop_reason: Some("end_turn".to_string()),
                            },
                            usage: None,
                        };
                        yield StreamEvent::MessageStop;
                        return;
                    }

                    // Extract text and compute delta
                    if let Some(content) = candidate.content {
                        for part in content.parts.unwrap_or_default() {
                            if let Some(full_text) = part.text {
                                // Gemini returns cumulative text — emit only the new part
                                if full_text.len() > emitted_text.len() {
                                    let delta_text = full_text[emitted_text.len()..].to_string();
                                    emitted_text = full_text;
                                    yield StreamEvent::ContentBlockDelta {
                                        index: 0,
                                        delta: json!({"type": "text_delta", "text": delta_text}),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        // Stream ended without explicit finish
        if header_emitted {
            yield StreamEvent::ContentBlockStop { index: 0 };
            yield StreamEvent::MessageDelta {
                delta: MessageDelta {
                    stop_reason: Some("end_turn".to_string()),
                },
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
    fn test_build_gemini_request_basic() {
        let req = MessagesRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![json!({"role": "user", "content": "Hello"})],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_gemini_request(&req);
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 1024);

        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn test_build_gemini_request_with_system() {
        let req = MessagesRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![json!({"role": "user", "content": "Hello"})],
            system: Some(vec![json!({"type": "text", "text": "Be helpful."})]),
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_gemini_request(&req);
        assert_eq!(
            body["system_instruction"]["parts"][0]["text"],
            "Be helpful."
        );
    }

    #[test]
    fn test_build_gemini_request_role_mapping() {
        let req = MessagesRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![
                json!({"role": "user", "content": "Hi"}),
                json!({"role": "assistant", "content": "Hello!"}),
                json!({"role": "user", "content": "How are you?"}),
            ],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_gemini_request(&req);
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model"); // "assistant" → "model"
        assert_eq!(contents[2]["role"], "user");
    }

    #[test]
    fn test_build_gemini_request_merges_consecutive_roles() {
        let req = MessagesRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![
                json!({"role": "user", "content": "Hello"}),
                json!({"role": "user", "content": "How are you?"}),
                json!({"role": "assistant", "content": "Fine."}),
            ],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_gemini_request(&req);
        let contents = body["contents"].as_array().unwrap();
        // Two user messages should be merged into one
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        let merged_text = contents[0]["parts"][0]["text"].as_str().unwrap();
        assert!(merged_text.contains("Hello"));
        assert!(merged_text.contains("How are you?"));
        assert_eq!(contents[1]["role"], "model");
    }

    #[test]
    fn test_build_gemini_request_content_blocks() {
        let req = MessagesRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![json!({
                "role": "user",
                "content": [
                    {"type": "text", "text": "Part A"},
                    {"type": "text", "text": "Part B"},
                ]
            })],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_gemini_request(&req);
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents[0]["parts"][0]["text"], "Part A\nPart B");
    }
}
