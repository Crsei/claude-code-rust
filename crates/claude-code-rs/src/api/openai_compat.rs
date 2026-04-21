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

use crate::api::client::{build_openai_compat_url, MessagesRequest, OPENAI_CODEX_PROVIDER_NAME};
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
                block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
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
                Some("text") => block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()),
                Some("tool_result") => block
                    .get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Convert a MessagesRequest (Anthropic format) to OpenAI chat completions body.
///
/// `provider_name` is used to select the correct token-limit parameter:
/// Azure OpenAI and OpenAI newer models require `max_completion_tokens`
/// instead of the legacy `max_tokens`.
fn build_openai_request(request: &MessagesRequest, provider_name: &str) -> Value {
    let is_codex_provider = provider_name.eq_ignore_ascii_case(OPENAI_CODEX_PROVIDER_NAME);
    let mut oai_messages: Vec<Value> = Vec::new();

    // System prompt → system message
    if let Some(system) = &request.system {
        let text = extract_system_text(system);
        if !text.is_empty() {
            oai_messages.push(json!({"role": "system", "content": text}));
        }
    }

    // User/assistant messages — convert to OpenAI format.
    // Handles: text messages, assistant tool_use blocks, user tool_result blocks.
    for msg in &request.messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = msg.get("content");

        if role == "assistant" {
            // Check if assistant message contains tool_use blocks
            if let Some(Value::Array(blocks)) = content {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls_out: Vec<Value> = Vec::new();

                for block in blocks {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                if !t.is_empty() {
                                    text_parts.push(t.to_string());
                                }
                            }
                        }
                        Some("tool_use") => {
                            let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let empty_obj = json!({});
                            let input = block.get("input").unwrap_or(&empty_obj);
                            tool_calls_out.push(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string(),
                                }
                            }));
                        }
                        _ => {}
                    }
                }

                let content_val = if text_parts.is_empty() {
                    Value::Null
                } else {
                    Value::String(text_parts.join("\n"))
                };

                if tool_calls_out.is_empty() {
                    if !text_parts.is_empty() {
                        oai_messages.push(json!({"role": "assistant", "content": content_val}));
                    }
                } else {
                    let mut assistant_msg = json!({"role": "assistant"});
                    if !text_parts.is_empty() {
                        assistant_msg["content"] = content_val;
                    }
                    assistant_msg["tool_calls"] = json!(tool_calls_out);
                    oai_messages.push(assistant_msg);
                }
            } else {
                let text = flatten_content(content);
                if !text.is_empty() {
                    oai_messages.push(json!({"role": "assistant", "content": text}));
                }
            }
        } else if role == "user" {
            // Check for tool_result blocks → convert to OpenAI "tool" role messages
            if let Some(Value::Array(blocks)) = content {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_results: Vec<(String, String)> = Vec::new();

                for block in blocks {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("tool_result") => {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("")
                                .to_string();
                            let result_content = block
                                .get("content")
                                .map(|c| match c {
                                    Value::String(s) => s.clone(),
                                    Value::Array(arr) => arr
                                        .iter()
                                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    other => other.to_string(),
                                })
                                .unwrap_or_default();
                            tool_results.push((tool_use_id, result_content));
                        }
                        Some("text") => {
                            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                if !t.is_empty() {
                                    text_parts.push(t.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Emit tool result messages first
                for (tool_use_id, result) in tool_results {
                    oai_messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": result,
                    }));
                }
                // Then any text parts as a regular user message
                if !text_parts.is_empty() {
                    oai_messages.push(json!({"role": "user", "content": text_parts.join("\n")}));
                }
            } else {
                let text = flatten_content(content);
                if !text.is_empty() {
                    oai_messages.push(json!({"role": "user", "content": text}));
                }
            }
        } else {
            let text = flatten_content(content);
            if !text.is_empty() {
                oai_messages.push(json!({"role": role, "content": text}));
            }
        }
    }

    let mut body = json!({
        "model": request.model,
        "messages": oai_messages,
        "stream": true,
    });
    if !is_codex_provider {
        body["stream_options"] = json!({ "include_usage": true });
    }

    if request.max_tokens > 0 && !is_codex_provider {
        // Azure OpenAI and OpenAI newer models (gpt-4o, o1, o3, gpt-5, etc.)
        // require `max_completion_tokens`; legacy `max_tokens` is rejected.
        let uses_new_param = provider_name.eq_ignore_ascii_case("azure")
            || provider_name.eq_ignore_ascii_case("openai")
            || request.model.starts_with("gpt-4o")
            || request.model.starts_with("gpt-5")
            || request.model.starts_with("o1")
            || request.model.starts_with("o3")
            || request.model.starts_with("o4");
        let key = if uses_new_param {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body[key] = json!(request.max_tokens);
    }

    // Convert Anthropic-format tools to OpenAI function-calling format.
    // Anthropic: {"name": "X", "description": "...", "input_schema": {...}}
    // OpenAI:    {"type": "function", "function": {"name": "X", "description": "...", "parameters": {...}}}
    if let Some(tools) = &request.tools {
        let oai_tools: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?;
                let description = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let parameters = t
                    .get("input_schema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters,
                    }
                }))
            })
            .collect();
        if !oai_tools.is_empty() {
            body["tools"] = json!(oai_tools);
        }
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
    let url = build_openai_compat_url(base_url, provider_name);
    let body = build_openai_request(request, provider_name);

    tracing::debug!(
        provider = provider_name,
        url = %url,
        body = %serde_json::to_string_pretty(&body).unwrap_or_default(),
        "OpenAI-compat request"
    );

    let mut req_builder = http.post(&url).header("Content-Type", "application/json");

    // Azure OpenAI uses `api-key` header; others use `Authorization: Bearer`
    if provider_name.eq_ignore_ascii_case("azure") {
        req_builder = req_builder.header("api-key", api_key);
    } else {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = match req_builder.json(&body).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!(
                provider = provider_name,
                url = %url,
                error = %e,
                error_debug = ?e,
                "HTTP request failed"
            );
            anyhow::bail!("failed to send request to {}: {}", provider_name, e);
        }
    };

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

        // Track the next content_block index (text = 0, tool_calls start at 1+).
        let mut block_index: usize = 0;
        // Track whether we are inside a text content block.
        let mut _text_block_open = false;
        // Track active tool calls: index → (id, name, accumulated arguments).
        let mut tool_calls: std::collections::HashMap<u64, (String, String, String)> =
            std::collections::HashMap::new();

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
                    // Close any open text block
                    if _text_block_open {
                        yield StreamEvent::ContentBlockStop { index: block_index };
                        _text_block_open = false;
                    }
                    if header_emitted {
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
                }

                // Process choices array
                if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        let delta = match choice.get("delta") {
                            Some(d) => d,
                            None => continue,
                        };

                        // ── Text content delta ──────────────────────────
                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                if !_text_block_open {
                                    yield StreamEvent::ContentBlockStart {
                                        index: block_index,
                                        content_block: ContentBlock::Text { text: String::new() },
                                    };
                                    _text_block_open = true;
                                }
                                yield StreamEvent::ContentBlockDelta {
                                    index: block_index,
                                    delta: json!({"type": "text_delta", "text": content}),
                                };
                            }
                        }

                        // ── Tool call deltas ────────────────────────────
                        // OpenAI streams tool_calls as:
                        //   delta.tool_calls: [{"index":0, "id":"call_xxx", "type":"function",
                        //                       "function":{"name":"Read","arguments":""}}]
                        // Subsequent chunks only have:
                        //   delta.tool_calls: [{"index":0, "function":{"arguments":"{\"fi"}}]
                        if let Some(tc_arr) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                            for tc in tc_arr {
                                let tc_index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0);

                                // New tool call: has "id" and "function.name"
                                if let Some(tc_id) = tc.get("id").and_then(|i| i.as_str()) {
                                    let fn_name = tc
                                        .get("function")
                                        .and_then(|f| f.get("name"))
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let initial_args = tc
                                        .get("function")
                                        .and_then(|f| f.get("arguments"))
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    tool_calls.insert(tc_index, (tc_id.to_string(), fn_name, initial_args));
                                } else if let Some(entry) = tool_calls.get_mut(&tc_index) {
                                    // Continuation: accumulate arguments
                                    if let Some(args_chunk) = tc
                                        .get("function")
                                        .and_then(|f| f.get("arguments"))
                                        .and_then(|a| a.as_str())
                                    {
                                        entry.2.push_str(args_chunk);
                                    }
                                }
                            }
                        }

                        // ── Finish reason ────────────────────────────────
                        if let Some(reason) = choice
                            .get("finish_reason")
                            .and_then(|r| r.as_str())
                        {
                            // Close any open text block
                            if _text_block_open {
                                yield StreamEvent::ContentBlockStop { index: block_index };
                                block_index += 1;
                                _text_block_open = false;
                            }

                            // Emit accumulated tool_calls as Anthropic-style ToolUse blocks
                            let mut sorted_tc: Vec<_> = tool_calls.drain().collect();
                            sorted_tc.sort_by_key(|(idx, _)| *idx);
                            for (_tc_idx, (tc_id, tc_name, tc_args)) in sorted_tc {
                                let input: Value = serde_json::from_str(&tc_args)
                                    .unwrap_or_else(|_| json!({}));
                                yield StreamEvent::ContentBlockStart {
                                    index: block_index,
                                    content_block: ContentBlock::ToolUse {
                                        id: tc_id,
                                        name: tc_name,
                                        input,
                                    },
                                };
                                yield StreamEvent::ContentBlockStop { index: block_index };
                                block_index += 1;
                            }

                            let stop_reason = match reason {
                                "stop" => "end_turn",
                                "length" => "max_tokens",
                                "tool_calls" => "tool_use",
                                other => other,
                            };
                            yield StreamEvent::MessageDelta {
                                delta: MessageDelta {
                                    stop_reason: Some(stop_reason.to_string()),
                                },
                                usage: None,
                            };
                            // Don't return yet — the usage-only chunk follows
                            // before [DONE] and we need to process it.
                        }
                    }
                }

                // Usage info — Azure/OpenAI send a final chunk with usage after finish_reason.
                // The chunk has `"usage": null` for content chunks, and a real object for the last.
                if let Some(usage) = v.get("usage").filter(|u| !u.is_null()) {
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
        if _text_block_open {
            yield StreamEvent::ContentBlockStop { index: block_index };
        }
        if header_emitted {
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
        assert_eq!(
            extract_system_text(&system),
            "You are helpful.\nBe concise."
        );
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
            advisor_model: None,
        };
        let body = build_openai_request(&req, "openai");
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], true);
        // OpenAI/Azure use max_completion_tokens for newer models
        assert_eq!(body["max_completion_tokens"], 1024);
        assert!(body.get("max_tokens").is_none() || body["max_tokens"].is_null());

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "Be helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Hello");
    }

    #[test]
    fn test_build_openai_request_codex_compatible_shape() {
        let req = MessagesRequest {
            model: "gpt-5.4".to_string(),
            messages: vec![json!({"role": "user", "content": "Hello"})],
            system: None,
            max_tokens: 4096,
            tools: None,
            stream: true,
            thinking: None,
            tool_choice: None,
            advisor_model: None,
        };
        let body = build_openai_request(&req, OPENAI_CODEX_PROVIDER_NAME);
        assert_eq!(body["model"], "gpt-5.4");
        assert_eq!(body["stream"], true);
        assert!(body.get("stream_options").is_none());
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("max_completion_tokens").is_none());
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
            advisor_model: None,
        };
        let body = build_openai_request(&req, "deepseek");
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        // DeepSeek uses legacy max_tokens
        assert_eq!(body["max_tokens"], 512);
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
            advisor_model: None,
        };
        let body = build_openai_request(&req, "openai");
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["content"], "Hello\nWorld");
    }
}
