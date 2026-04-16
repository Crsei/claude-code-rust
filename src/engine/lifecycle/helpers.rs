//! Helper functions for the QueryEngine lifecycle.

use crate::types::message::Message;

/// Convert a conversation history into a text representation suitable for
/// model-based summarization. Extracts user messages, assistant text, and
/// tool use/result information.
pub(crate) fn format_conversation_for_summary(messages: &[Message]) -> String {
    let mut parts = Vec::new();

    for msg in messages {
        match msg {
            Message::User(u) => {
                let text = match &u.content {
                    crate::types::message::MessageContent::Text(t) => t.clone(),
                    crate::types::message::MessageContent::Blocks(blocks) => blocks
                        .iter()
                        .filter_map(|b| match b {
                            crate::types::message::ContentBlock::Text { text } => {
                                Some(text.clone())
                            }
                            crate::types::message::ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                is_error,
                            } => {
                                let result_text = match content {
                                    crate::types::message::ToolResultContent::Text(t) => {
                                        if t.len() > 500 {
                                            format!("{}...[truncated]", &t[..500])
                                        } else {
                                            t.clone()
                                        }
                                    }
                                    crate::types::message::ToolResultContent::Blocks(_) => {
                                        "[complex result]".to_string()
                                    }
                                };
                                Some(format!(
                                    "[Tool Result ({}{}): {}]",
                                    tool_use_id,
                                    if *is_error { ", ERROR" } else { "" },
                                    result_text
                                ))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                if !text.is_empty() && !u.is_meta {
                    parts.push(format!("User: {}", text));
                }
            }
            Message::Assistant(a) => {
                for block in &a.content {
                    match block {
                        crate::types::message::ContentBlock::Text { text } => {
                            if text.len() > 1000 {
                                parts.push(format!("Assistant: {}...[truncated]", &text[..1000]));
                            } else {
                                parts.push(format!("Assistant: {}", text));
                            }
                        }
                        crate::types::message::ContentBlock::ToolUse { name, input, .. } => {
                            let input_preview = {
                                let s = input.to_string();
                                if s.len() > 200 {
                                    format!("{}...", &s[..200])
                                } else {
                                    s
                                }
                            };
                            parts.push(format!("[Tool Use: {} ({})]", name, input_preview));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // Cap total length to avoid exceeding context for the summarization call
    let joined = parts.join("\n\n");
    if joined.len() > 400_000 {
        format!(
            "{}...\n\n[conversation truncated for summarization]",
            &joined[..400_000]
        )
    } else {
        joined
    }
}

/// Build a `MessagesRequest` from the generic `ModelCallParams`.
///
/// This translates the engine's internal representation into the wire format
/// expected by `api::client::ApiClient`.
pub(crate) fn build_messages_request(
    params: &crate::query::deps::ModelCallParams,
) -> crate::api::client::MessagesRequest {
    use crate::types::message::{Message, MessageContent};

    // Convert Message list to API JSON format
    let api_messages: Vec<serde_json::Value> = params
        .messages
        .iter()
        .filter_map(|msg| match msg {
            Message::User(u) => {
                let content = match &u.content {
                    MessageContent::Text(t) => serde_json::json!(t),
                    MessageContent::Blocks(blocks) => {
                        serde_json::to_value(blocks).unwrap_or_default()
                    }
                };
                Some(serde_json::json!({
                    "role": "user",
                    "content": content,
                }))
            }
            Message::Assistant(a) => {
                let content = serde_json::to_value(&a.content).unwrap_or_default();
                Some(serde_json::json!({
                    "role": "assistant",
                    "content": content,
                }))
            }
            // System, Progress, Attachment messages are not sent to the API
            _ => None,
        })
        .collect();

    // Convert system prompt parts into API format
    let system = if params.system_prompt.is_empty() {
        None
    } else {
        Some(
            params
                .system_prompt
                .iter()
                .map(|s| serde_json::json!({"type": "text", "text": s}))
                .collect(),
        )
    };

    // Convert tools to API JSON format.
    let tools: Option<Vec<serde_json::Value>> = if params.tools.is_empty() {
        None
    } else {
        Some(
            params
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name(),
                        "description": "",
                        "input_schema": t.input_json_schema(),
                    })
                })
                .collect(),
        )
    };

    // Build thinking config
    let thinking = params.thinking_enabled.and_then(|enabled| {
        if enabled {
            Some(serde_json::json!({
                "type": "enabled",
                "budget_tokens": params.max_output_tokens.unwrap_or(16384)
            }))
        } else {
            None
        }
    });

    crate::api::client::MessagesRequest {
        model: params
            .model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        messages: api_messages,
        system,
        max_tokens: params.max_output_tokens.unwrap_or(16384),
        tools,
        stream: true,
        thinking,
        tool_choice: None,
    }
}
