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

    // Build thinking config.
    //
    // The budget is resolved with `effort_value` taking priority over
    // `max_output_tokens`, so /effort low|medium|high (or a numeric override)
    // controls reasoning depth without also capping the response length.
    let thinking = params.thinking_enabled.and_then(|enabled| {
        if enabled {
            let max_tokens_fallback = params
                .max_output_tokens
                .map(|n| n.min(u32::MAX as usize) as u32);
            let budget = crate::engine::effort::resolve_thinking_budget(
                params.effort_value.as_deref(),
                max_tokens_fallback,
            );
            Some(serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget,
            }))
        } else {
            None
        }
    });

    let resolved_model = params
        .model
        .clone()
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    crate::api::client::MessagesRequest {
        max_tokens: clamp_max_tokens_for_model(
            params.max_output_tokens.unwrap_or(16384),
            &resolved_model,
        ),
        model: resolved_model,
        messages: api_messages,
        system,
        tools,
        stream: true,
        thinking,
        tool_choice: None,
        advisor_model: params.advisor_model.clone(),
    }
}

/// Some OpenAI-compatible providers cap `max_tokens` below cc-rust's default
/// 16384. Rather than rely on provider-side errors surfacing as a blown
/// response, clamp at build time so the first request also succeeds.
pub(crate) fn clamp_max_tokens_for_model(requested: usize, model: &str) -> usize {
    // Keep the rules here narrow and documented; only add an entry when a
    // provider has confirmed, consistent behaviour.
    let lower = model.to_ascii_lowercase();
    let cap: Option<usize> = if lower.starts_with("deepseek") {
        // Confirmed via https://api.deepseek.com: valid range is [1, 8192].
        Some(8192)
    } else {
        None
    };
    match cap {
        Some(c) => requested.min(c),
        None => requested,
    }
}

#[cfg(test)]
mod clamp_tests {
    use super::clamp_max_tokens_for_model;

    #[test]
    fn deepseek_is_capped_at_8192() {
        assert_eq!(clamp_max_tokens_for_model(16384, "deepseek-chat"), 8192);
        assert_eq!(clamp_max_tokens_for_model(20000, "DeepSeek-Reasoner"), 8192);
    }

    #[test]
    fn deepseek_below_cap_unchanged() {
        assert_eq!(clamp_max_tokens_for_model(4096, "deepseek-chat"), 4096);
    }

    #[test]
    fn non_deepseek_unchanged() {
        assert_eq!(
            clamp_max_tokens_for_model(16384, "claude-sonnet-4-20250514"),
            16384
        );
        assert_eq!(clamp_max_tokens_for_model(32000, "gpt-4o"), 32000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::deps::ModelCallParams;

    fn base_params() -> ModelCallParams {
        ModelCallParams {
            messages: vec![],
            system_prompt: vec!["sys".into()],
            tools: vec![],
            model: Some("claude-sonnet-4-20250514".into()),
            max_output_tokens: Some(16_384),
            skip_cache_write: None,
            thinking_enabled: Some(true),
            effort_value: None,
            advisor_model: None,
        }
    }

    #[test]
    fn thinking_budget_uses_effort_high_over_max_tokens() {
        let mut p = base_params();
        p.effort_value = Some("high".into());
        p.max_output_tokens = Some(99_999);

        let req = build_messages_request(&p);
        let thinking = req.thinking.expect("thinking config present");
        assert_eq!(thinking["type"], "enabled");
        assert_eq!(thinking["budget_tokens"], 24_576);
    }

    #[test]
    fn thinking_budget_falls_back_to_max_tokens_when_effort_missing() {
        let mut p = base_params();
        p.effort_value = None;
        p.max_output_tokens = Some(8_000);

        let req = build_messages_request(&p);
        let thinking = req.thinking.expect("thinking config present");
        assert_eq!(thinking["budget_tokens"], 8_000);
    }

    #[test]
    fn thinking_budget_accepts_numeric_effort_override() {
        let mut p = base_params();
        p.effort_value = Some("12345".into());
        p.max_output_tokens = Some(8_000);

        let req = build_messages_request(&p);
        let thinking = req.thinking.expect("thinking config present");
        assert_eq!(thinking["budget_tokens"], 12_345);
    }

    #[test]
    fn thinking_omitted_when_disabled() {
        let mut p = base_params();
        p.thinking_enabled = Some(false);
        p.effort_value = Some("high".into());

        let req = build_messages_request(&p);
        assert!(req.thinking.is_none());
    }

    #[test]
    fn thinking_omitted_when_unset() {
        let mut p = base_params();
        p.thinking_enabled = None;
        p.effort_value = Some("high".into());

        let req = build_messages_request(&p);
        assert!(req.thinking.is_none());
    }
}
