//! Base protocol types shared across all domains.

use serde::{Deserialize, Serialize};

use crate::types::message::ContentBlock;

/// Lightweight description of a content block in a tool result,
/// suitable for forwarding to the frontend without embedding raw image data.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentInfo {
    /// Plain text content.
    Text { text: String },
    /// An image was returned (data is NOT forwarded — only metadata).
    Image {
        /// MIME type (e.g. "image/png").
        media_type: String,
        /// Approximate byte size of the base64-decoded image data.
        #[serde(skip_serializing_if = "Option::is_none")]
        size_bytes: Option<usize>,
    },
}

/// A single conversation message as seen by the frontend.
#[derive(Serialize, Debug, Clone)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<ContentBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::ContentBlock;

    #[test]
    fn conversation_message_serializes_content_blocks_when_present() {
        let message = ConversationMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            content: "summary".to_string(),
            timestamp: 1,
            content_blocks: Some(vec![
                ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "Read".to_string(),
                    input: serde_json::json!({ "file_path": "/tmp/a.ts" }),
                },
                ContentBlock::Text {
                    text: "summary".to_string(),
                },
            ]),
            cost_usd: Some(0.01),
            thinking: None,
            level: None,
        };

        let value = serde_json::to_value(&message).expect("serialize conversation message");
        let blocks = value
            .get("content_blocks")
            .and_then(|v| v.as_array())
            .expect("content_blocks array");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "tool_use");
        assert_eq!(blocks[1]["type"], "text");
    }

    #[test]
    fn conversation_message_omits_content_blocks_when_absent() {
        let message = ConversationMessage {
            id: "system-1".to_string(),
            role: "system".to_string(),
            content: "info".to_string(),
            timestamp: 1,
            content_blocks: None,
            cost_usd: None,
            thinking: None,
            level: Some("info".to_string()),
        };

        let value = serde_json::to_value(&message).expect("serialize conversation message");
        assert!(value.get("content_blocks").is_none());
    }

    #[test]
    fn backend_question_request_serializes() {
        use super::super::BackendMessage;
        let msg = BackendMessage::QuestionRequest {
            id: "q-1".into(),
            text: "Continue?".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "question_request");
        assert_eq!(json["id"], "q-1");
        assert_eq!(json["text"], "Continue?");
    }

    #[test]
    fn frontend_question_response_deserializes() {
        use super::super::FrontendMessage;
        let json = r#"{"type":"question_response","id":"q-1","text":"yes"}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        match msg {
            FrontendMessage::QuestionResponse { id, text } => {
                assert_eq!(id, "q-1");
                assert_eq!(text, "yes");
            }
            _ => panic!("expected QuestionResponse"),
        }
    }
}
