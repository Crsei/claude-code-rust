#![allow(unused)]
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
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            content_blocks: Vec::new(),
            usage: Usage::default(),
            stop_reason: None,
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
                self.content_blocks[*index] = content_block.clone();
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    if let ContentBlock::Text { ref mut text } = block {
                        if let Some(t) = delta.get("text").and_then(|v| v.as_str()) {
                            text.push_str(t);
                        }
                    }
                }
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

    /// Build the final AssistantMessage with cost calculated from model pricing.
    pub fn build(self, model: &str) -> AssistantMessage {
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
