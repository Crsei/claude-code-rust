//! SSE (Server-Sent Events) stream parser for the Anthropic Messages API
use anyhow::Result;
use serde_json::Value;

use crate::types::message::{AssistantMessage, ContentBlock, MessageDelta, StreamEvent, Usage};

/// Parse a single SSE line into a StreamEvent
pub fn parse_sse_event(event_type: &str, data: &str) -> Result<Option<StreamEvent>> {
    let parsed: Value = serde_json::from_str(data)?;

    match event_type {
        "message_start" => {
            let usage_val = parsed.get("message").and_then(|m| m.get("usage"));
            let usage = if let Some(u) = usage_val {
                serde_json::from_value(u.clone()).unwrap_or_default()
            } else {
                Usage::default()
            };
            Ok(Some(StreamEvent::MessageStart { usage }))
        }
        "content_block_start" => {
            let index = parsed.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let block: ContentBlock =
                serde_json::from_value(parsed.get("content_block").cloned().unwrap_or_default())?;
            Ok(Some(StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            }))
        }
        "content_block_delta" => {
            let index = parsed.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let delta = parsed.get("delta").cloned().unwrap_or_default();
            Ok(Some(StreamEvent::ContentBlockDelta { index, delta }))
        }
        "content_block_stop" => {
            let index = parsed.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Ok(Some(StreamEvent::ContentBlockStop { index }))
        }
        "message_delta" => {
            let delta: MessageDelta =
                serde_json::from_value(parsed.get("delta").cloned().unwrap_or_default())
                    .unwrap_or(MessageDelta { stop_reason: None });
            let usage = parsed
                .get("usage")
                .and_then(|u| serde_json::from_value(u.clone()).ok());
            Ok(Some(StreamEvent::MessageDelta { delta, usage }))
        }
        "message_stop" => Ok(Some(StreamEvent::MessageStop)),
        "ping" | "error" => Ok(None),
        _ => Ok(None),
    }
}

/// Accumulate stream events into a complete AssistantMessage
pub struct StreamAccumulator {
    pub content_blocks: Vec<ContentBlock>,
    pub usage: Usage,
    pub stop_reason: Option<String>,
    tool_input_partials: Vec<String>,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            content_blocks: Vec::new(),
            usage: Usage::default(),
            stop_reason: None,
            tool_input_partials: Vec::new(),
        }
    }

    pub fn process_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::MessageStart { usage } => {
                self.usage = usage.clone();
            }
            StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                while self.content_blocks.len() <= *index {
                    self.content_blocks.push(ContentBlock::Text {
                        text: String::new(),
                    });
                }
                while self.tool_input_partials.len() <= *index {
                    self.tool_input_partials.push(String::new());
                }
                self.content_blocks[*index] = content_block.clone();
                self.tool_input_partials[*index].clear();
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    let handled = match block {
                        ContentBlock::Text { text } => {
                            if delta_type_matches(delta, "text_delta") {
                                if let Some(t) = delta.get("text").and_then(|v| v.as_str()) {
                                    text.push_str(t);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        ContentBlock::Thinking {
                            thinking,
                            signature,
                        } => {
                            let mut handled = false;
                            if delta_type_matches(delta, "thinking_delta") {
                                if let Some(t) = delta.get("thinking").and_then(|v| v.as_str()) {
                                    thinking.push_str(t);
                                    handled = true;
                                }
                            }
                            if delta_type_matches(delta, "signature_delta") {
                                if let Some(s) = delta.get("signature").and_then(|v| v.as_str()) {
                                    signature.get_or_insert_with(String::new).push_str(s);
                                    handled = true;
                                }
                            }
                            handled
                        }
                        ContentBlock::ToolUse { .. } => {
                            if delta_type_matches(delta, "input_json_delta") {
                                if let Some(partial_json) =
                                    delta.get("partial_json").and_then(|v| v.as_str())
                                {
                                    while self.tool_input_partials.len() <= *index {
                                        self.tool_input_partials.push(String::new());
                                    }
                                    self.tool_input_partials[*index].push_str(partial_json);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    if !handled {
                        log_unsupported_delta(*index, delta);
                    }
                } else {
                    tracing::warn!(
                        index = *index,
                        delta_type = delta_type_name(delta),
                        "received stream content block delta before content block start"
                    );
                }
            }
            StreamEvent::ContentBlockStop { index } => {
                self.finalize_tool_input(*index);
            }
            StreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason = delta.stop_reason.clone();
                if let Some(u) = usage {
                    // OpenAI-compatible providers report both input and output
                    // tokens in the final chunk; Anthropic only sends output here.
                    if u.input_tokens > 0 {
                        self.usage.input_tokens = u.input_tokens;
                    }
                    if u.output_tokens > 0 {
                        self.usage.output_tokens = u.output_tokens;
                    }
                }
            }
            _ => {}
        }
    }

    fn finalize_tool_input(&mut self, index: usize) {
        let Some(partial_json) = self.tool_input_partials.get_mut(index) else {
            return;
        };
        if partial_json.is_empty() {
            return;
        }

        let Ok(input) = serde_json::from_str::<Value>(partial_json) else {
            return;
        };

        if let Some(ContentBlock::ToolUse {
            input: block_input, ..
        }) = self.content_blocks.get_mut(index)
        {
            *block_input = input;
            partial_json.clear();
        }
    }

    /// Build the final AssistantMessage with cost calculated from model pricing.
    pub fn build(mut self, model: &str) -> AssistantMessage {
        for index in 0..self.content_blocks.len() {
            self.finalize_tool_input(index);
        }

        let cost_usd = crate::api::pricing::calculate_cost(model, &self.usage);
        AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp(),
            role: "assistant".to_string(),
            content: self.content_blocks,
            usage: Some(self.usage),
            stop_reason: self.stop_reason,
            is_api_error_message: false,
            api_error: None,
            cost_usd,
        }
    }
}

fn delta_type_matches(delta: &Value, expected: &str) -> bool {
    delta
        .get("type")
        .and_then(|v| v.as_str())
        .map_or(true, |actual| actual == expected)
}

fn delta_type_name(delta: &Value) -> &str {
    delta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
}

fn log_unsupported_delta(index: usize, delta: &Value) {
    tracing::warn!(
        index,
        delta_type = delta_type_name(delta),
        "ignoring unsupported stream content block delta"
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accumulates_tool_input_json_delta() {
        let mut accumulator = StreamAccumulator::new();

        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::ToolUse {
                id: "toolu_1".to_string(),
                name: "Bash".to_string(),
                input: json!({}),
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "input_json_delta",
                "partial_json": "{\"command\":\"echo"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "input_json_delta",
                "partial_json": " hi\",\"timeout\":1000}"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockStop { index: 0 });

        let message = accumulator.build("claude-sonnet-4-20250514");
        match &message.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_1");
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "echo hi");
                assert_eq!(input["timeout"], 1000);
            }
            other => panic!("expected tool_use block, got {other:?}"),
        }
    }

    #[test]
    fn accumulates_thinking_signature_delta() {
        let mut accumulator = StreamAccumulator::new();

        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::Thinking {
                thinking: String::new(),
                signature: None,
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "thinking_delta",
                "thinking": "first "
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "thinking_delta",
                "thinking": "second"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "signature_delta",
                "signature": "sig-a"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "signature_delta",
                "signature": "sig-b"
            }),
        });

        let message = accumulator.build("claude-sonnet-4-20250514");
        match &message.content[0] {
            ContentBlock::Thinking {
                thinking,
                signature,
            } => {
                assert_eq!(thinking, "first second");
                assert_eq!(signature.as_deref(), Some("sig-asig-b"));
            }
            other => panic!("expected thinking block, got {other:?}"),
        }
    }

    #[test]
    fn ignores_text_like_unsupported_delta() {
        let mut accumulator = StreamAccumulator::new();

        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::Text {
                text: "prefix".to_string(),
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "connector_text_delta",
                "text": " should not append"
            }),
        });

        let message = accumulator.build("claude-sonnet-4-20250514");
        match &message.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "prefix"),
            other => panic!("expected text block, got {other:?}"),
        }
    }

    #[test]
    fn accepts_legacy_delta_without_type() {
        let mut accumulator = StreamAccumulator::new();

        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::Text {
                text: String::new(),
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({ "text": "legacy" }),
        });

        let message = accumulator.build("claude-sonnet-4-20250514");
        match &message.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "legacy"),
            other => panic!("expected text block, got {other:?}"),
        }
    }

    #[test]
    fn accumulates_mixed_anthropic_stream_sequence() {
        let mut accumulator = StreamAccumulator::new();

        accumulator.process_event(&StreamEvent::MessageStart {
            usage: Usage {
                input_tokens: 42,
                output_tokens: 0,
                cache_read_input_tokens: 3,
                cache_creation_input_tokens: 2,
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlock::Text {
                text: String::new(),
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 0,
            delta: json!({
                "type": "text_delta",
                "text": "hello"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockStop { index: 0 });
        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 1,
            content_block: ContentBlock::Thinking {
                thinking: String::new(),
                signature: None,
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 1,
            delta: json!({
                "type": "thinking_delta",
                "thinking": "considering"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 1,
            delta: json!({
                "type": "signature_delta",
                "signature": "sig"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockStop { index: 1 });
        accumulator.process_event(&StreamEvent::ContentBlockStart {
            index: 2,
            content_block: ContentBlock::ToolUse {
                id: "toolu_2".to_string(),
                name: "Read".to_string(),
                input: json!({}),
            },
        });
        accumulator.process_event(&StreamEvent::ContentBlockDelta {
            index: 2,
            delta: json!({
                "type": "input_json_delta",
                "partial_json": "{\"file_path\":\"Cargo.toml\"}"
            }),
        });
        accumulator.process_event(&StreamEvent::ContentBlockStop { index: 2 });
        accumulator.process_event(&StreamEvent::MessageDelta {
            delta: MessageDelta {
                stop_reason: Some("tool_use".to_string()),
            },
            usage: Some(Usage {
                input_tokens: 0,
                output_tokens: 9,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
        });
        accumulator.process_event(&StreamEvent::MessageStop);

        let message = accumulator.build("claude-sonnet-4-20250514");
        assert_eq!(message.stop_reason.as_deref(), Some("tool_use"));
        let usage = message.usage.as_ref().expect("usage");
        assert_eq!(usage.input_tokens, 42);
        assert_eq!(usage.output_tokens, 9);
        assert_eq!(usage.cache_read_input_tokens, 3);
        assert_eq!(usage.cache_creation_input_tokens, 2);

        match &message.content[..] {
            [ContentBlock::Text { text }, ContentBlock::Thinking {
                thinking,
                signature,
            }, ContentBlock::ToolUse { id, name, input }] => {
                assert_eq!(text, "hello");
                assert_eq!(thinking, "considering");
                assert_eq!(signature.as_deref(), Some("sig"));
                assert_eq!(id, "toolu_2");
                assert_eq!(name, "Read");
                assert_eq!(input["file_path"], "Cargo.toml");
            }
            other => panic!("unexpected content blocks: {other:?}"),
        }
    }
}
