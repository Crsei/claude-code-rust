//! SdkMessage → BackendMessage mapping.
//!
//! Pure mapping layer extracted from `headless.rs`.  Each [`SdkMessage`] variant
//! is translated into one or more [`BackendMessage`]s and written via the
//! [`FrontendSink`].  This module has **no** runtime state — it only depends on
//! the protocol types, the engine (for message/suggestion reads), and the
//! suggestion service.

use std::sync::Arc;

use parking_lot::Mutex;
use tracing::debug;

use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::message::{ContentBlock, Message, StreamEvent, ToolResultContent};

use super::protocol::{BackendMessage, ToolResultContentInfo};
use super::sink::FrontendSink;

// ---------------------------------------------------------------------------
// SdkMessage → BackendMessage mapping
// ---------------------------------------------------------------------------

/// Map a single [`SdkMessage`] to the appropriate [`BackendMessage`](s) and
/// send them to the frontend.  This is the central dispatch for the headless
/// protocol — every `SdkMessage` variant is handled here.
pub fn handle_sdk_message(
    sdk_msg: &SdkMessage,
    message_id: &str,
    engine: &Arc<QueryEngine>,
    suggestion_svc: &Arc<Mutex<PromptSuggestionService>>,
    sink: &FrontendSink,
) -> std::io::Result<()> {
    match sdk_msg {
        // ── SystemInit ──────────────────────────────────────────
        SdkMessage::SystemInit(init) => sink.send(&BackendMessage::SystemInfo {
            text: format!(
                "Permission: {}, {} tools",
                init.permission_mode,
                init.tools.len(),
            ),
            level: "info".to_string(),
        }),

        // ── StreamEvent ─────────────────────────────────────────
        SdkMessage::StreamEvent(evt) => handle_stream_event(&evt.event, message_id, sink),

        // ── Assistant message ───────────────────────────────────
        SdkMessage::Assistant(a) => {
            // First send individual ToolUse messages for each tool call
            // so the frontend can render them immediately.
            for block in &a.message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    let _ = sink.send(&BackendMessage::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
            }

            // Then send the full assistant message
            let content =
                serde_json::to_value(&a.message.content).unwrap_or(serde_json::Value::Null);
            sink.send(&BackendMessage::AssistantMessage {
                id: a.message.uuid.to_string(),
                content,
                cost_usd: a.message.cost_usd,
            })
        }

        // ── UserReplay (includes tool results) ──────────────────
        SdkMessage::UserReplay(replay) => {
            if replay.is_synthetic {
                debug!("headless: user replay (synthetic): {}", replay.content);
            }

            // Extract and forward tool results from content blocks
            if let Some(ref blocks) = replay.content_blocks {
                for block in blocks {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = block
                    {
                        let (output, content_infos) = match content {
                            ToolResultContent::Text(t) => (t.clone(), None),
                            ToolResultContent::Blocks(inner) => extract_tool_result_output(inner),
                        };
                        let _ = sink.send(&BackendMessage::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            output,
                            is_error: *is_error,
                            content_blocks: content_infos,
                        });
                    }
                }
            }

            Ok(())
        }

        // ── CompactBoundary ─────────────────────────────────────
        SdkMessage::CompactBoundary(boundary) => {
            let text = if let Some(ref meta) = boundary.compact_metadata {
                format!(
                    "Context compacted: {} → {} tokens",
                    meta.pre_compact_token_count, meta.post_compact_token_count
                )
            } else {
                "Context compacted".to_string()
            };
            sink.send(&BackendMessage::SystemInfo {
                text,
                level: "info".to_string(),
            })
        }

        // ── ApiRetry ────────────────────────────────────────────
        SdkMessage::ApiRetry(retry) => sink.send(&BackendMessage::Error {
            message: format!(
                "API retry {}/{}: {} (waiting {}ms)",
                retry.attempt, retry.max_retries, retry.error, retry.retry_delay_ms
            ),
            recoverable: true,
        }),

        // ── ToolUseSummary ──────────────────────────────────────
        SdkMessage::ToolUseSummary(summary) => sink.send(&BackendMessage::SystemInfo {
            text: summary.summary.clone(),
            level: "info".to_string(),
        }),

        // ── Result ──────────────────────────────────────────────
        SdkMessage::Result(r) => {
            // Always send StreamEnd to clear UI streaming state
            let _ = sink.send(&BackendMessage::StreamEnd {
                message_id: message_id.to_string(),
            });

            let _ = sink.send(&BackendMessage::UsageUpdate {
                input_tokens: r.usage.total_input_tokens,
                output_tokens: r.usage.total_output_tokens,
                cost_usd: r.usage.total_cost_usd,
            });

            if r.is_error {
                let _ = sink.send(&BackendMessage::Error {
                    message: r.result.clone(),
                    recoverable: true,
                });
            }

            // Generate prompt suggestions after query completion
            generate_and_send_suggestions(engine, suggestion_svc, sink);

            // Try to extract session memory insights
            engine.try_extract_session_memory();

            // Fire Notification hook (sound/alert when query finishes)
            {
                let hooks_map = engine.app_state().hooks;
                tokio::spawn(async move {
                    crate::tools::hooks::fire_notification_hook(
                        "Claude Code",
                        "Response ready",
                        &hooks_map,
                    )
                    .await;
                });
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// StreamEvent mapping
// ---------------------------------------------------------------------------

/// Map a [`StreamEvent`] to the appropriate [`BackendMessage`] and send it.
fn handle_stream_event(
    event: &StreamEvent,
    message_id: &str,
    sink: &FrontendSink,
) -> std::io::Result<()> {
    match event {
        StreamEvent::MessageStart { .. } => sink.send(&BackendMessage::StreamStart {
            message_id: message_id.to_string(),
        }),
        StreamEvent::ContentBlockStart { .. } => Ok(()),
        StreamEvent::ContentBlockDelta { ref delta, .. } => {
            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                sink.send(&BackendMessage::StreamDelta {
                    message_id: message_id.to_string(),
                    text: text.to_string(),
                })
            } else if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                sink.send(&BackendMessage::ThinkingDelta {
                    message_id: message_id.to_string(),
                    thinking: thinking.to_string(),
                })
            } else {
                Ok(())
            }
        }
        StreamEvent::MessageStop => sink.send(&BackendMessage::StreamEnd {
            message_id: message_id.to_string(),
        }),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Tool result content extraction
// ---------------------------------------------------------------------------

/// Extract human-readable output text and optional structured content info
/// from a `ToolResultContent::Blocks(...)`.
///
/// For text blocks: concatenated into the output string.
/// For image blocks: represented as `[image: mime_type]` in the output text,
///   with metadata forwarded in the content_infos vec.
pub fn extract_tool_result_output(
    blocks: &[ContentBlock],
) -> (String, Option<Vec<ToolResultContentInfo>>) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut infos: Vec<ToolResultContentInfo> = Vec::new();
    let mut has_non_text = false;

    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
                infos.push(ToolResultContentInfo::Text { text: text.clone() });
            }
            ContentBlock::Image { source } => {
                has_non_text = true;
                let media_type = source.media_type.clone();
                let size_bytes = Some(source.data.len() * 3 / 4); // approx decoded size
                text_parts.push(format!("[image: {}]", media_type));
                infos.push(ToolResultContentInfo::Image {
                    media_type,
                    size_bytes,
                });
            }
            _ => {
                // Other block types (ToolUse, Thinking, etc.) — just note them
                text_parts.push("[...]".to_string());
            }
        }
    }

    let output = if text_parts.is_empty() {
        "(no output)".to_string()
    } else {
        text_parts.join("\n")
    };

    let content_infos = if has_non_text { Some(infos) } else { None };
    (output, content_infos)
}

// ---------------------------------------------------------------------------
// Prompt suggestions
// ---------------------------------------------------------------------------

/// Generate prompt suggestions from the last assistant message and send them.
pub fn generate_and_send_suggestions(
    engine: &Arc<QueryEngine>,
    svc: &Arc<Mutex<PromptSuggestionService>>,
    sink: &FrontendSink,
) {
    let messages = engine.messages();

    let mut svc = svc.lock();

    // Check suppression (too few messages, rate-limited, etc.)
    if svc.get_suppression_reason(messages.len(), false).is_some() {
        return;
    }

    // Find last assistant message
    let last_assistant = messages.iter().rev().find_map(|msg| match msg {
        Message::Assistant(a) => Some(a),
        _ => None,
    });
    let Some(assistant) = last_assistant else {
        return;
    };

    // Extract tool names and text summary
    let tool_names: Vec<String> = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let summary: String = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(suggestions) = svc.try_generate(&summary, &tool_names) {
        let items: Vec<String> = suggestions
            .into_iter()
            .take(3)
            .map(|s| format!("{} {}", s.category.icon(), s.text))
            .collect();

        if !items.is_empty() {
            let _ = sink.send(&BackendMessage::Suggestions { items });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::ImageSource;

    #[test]
    fn test_extract_text_only_blocks() {
        let blocks = vec![
            ContentBlock::Text {
                text: "line 1".into(),
            },
            ContentBlock::Text {
                text: "line 2".into(),
            },
        ];
        let (output, infos) = extract_tool_result_output(&blocks);
        assert_eq!(output, "line 1\nline 2");
        assert!(infos.is_none(), "no non-text blocks → None");
    }

    #[test]
    fn test_extract_image_block_shows_placeholder() {
        let blocks = vec![ContentBlock::Image {
            source: ImageSource {
                source_type: "base64".into(),
                media_type: "image/png".into(),
                data: "aGVsbG8=".into(),
            },
        }];
        let (output, infos) = extract_tool_result_output(&blocks);
        assert_eq!(output, "[image: image/png]");
        let infos = infos.expect("should have content_infos");
        assert_eq!(infos.len(), 1);
        match &infos[0] {
            ToolResultContentInfo::Image { media_type, .. } => {
                assert_eq!(media_type, "image/png");
            }
            _ => panic!("expected Image info"),
        }
    }

    #[test]
    fn test_extract_mixed_text_and_image() {
        let blocks = vec![
            ContentBlock::Text {
                text: "screenshot taken".into(),
            },
            ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/jpeg".into(),
                    data: "AAAA".into(),
                },
            },
        ];
        let (output, infos) = extract_tool_result_output(&blocks);
        assert!(output.contains("screenshot taken"));
        assert!(output.contains("[image: image/jpeg]"));
        let infos = infos.expect("has image → Some");
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn test_extract_empty_blocks() {
        let blocks: Vec<ContentBlock> = vec![];
        let (output, infos) = extract_tool_result_output(&blocks);
        assert_eq!(output, "(no output)");
        assert!(infos.is_none());
    }
}
