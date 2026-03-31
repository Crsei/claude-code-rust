#![allow(unused)]

/// 消息工具函数
///
/// 对应 TypeScript 中散布在多处的消息处理辅助函数

use crate::types::message::{ContentBlock, Message, MessageContent};

/// 从消息中提取纯文本内容
pub fn get_text_content(message: &Message) -> String {
    match message {
        Message::User(m) => match &m.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        },
        Message::Assistant(m) => m
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Message::System(m) => m.content.clone(),
        Message::Progress(_) => String::new(),
        Message::Attachment(_) => String::new(),
    }
}

/// 估算消息的 token 数 (粗略: 4 chars ≈ 1 token)
pub fn estimate_tokens(message: &Message) -> u64 {
    let text = get_text_content(message);
    (text.len() as u64 / 4).max(1)
}

/// Count the number of tool calls (ToolUse blocks) with the given tool name
/// across all assistant messages.
pub fn count_tool_calls(messages: &[Message], tool_name: &str) -> usize {
    messages
        .iter()
        .filter_map(|m| match m {
            Message::Assistant(a) => Some(&a.content),
            _ => None,
        })
        .flat_map(|content| content.iter())
        .filter(|block| matches!(block, ContentBlock::ToolUse { name, .. } if name == tool_name))
        .count()
}

/// Check if the last assistant message contains any tool use blocks.
///
/// Returns `false` if there are no assistant messages.
pub fn last_message_has_tool_use(messages: &[Message]) -> bool {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::Assistant(a) => Some(a),
            _ => None,
        })
        .map(|a| {
            a.content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
        })
        .unwrap_or(false)
}

/// Extract text content from the last assistant message.
///
/// Concatenates all Text blocks in the last assistant message.
/// Returns `None` if there are no assistant messages or if the last
/// assistant message contains no text blocks.
pub fn get_last_assistant_text(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::Assistant(a) => Some(a),
            _ => None,
        })
        .and_then(|a| {
            let text_parts: Vec<&str> = a
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();

            if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join(""))
            }
        })
}

/// Get all tool use IDs from the last assistant message.
///
/// Returns an empty vec if there are no assistant messages.
pub fn get_last_assistant_tool_use_ids(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::Assistant(a) => Some(a),
            _ => None,
        })
        .map(|a| {
            a.content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Count total assistant messages.
pub fn count_assistant_messages(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, Message::Assistant(_)))
        .count()
}

/// Count total user messages (excluding meta/system-injected ones).
pub fn count_user_messages(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, Message::User(u) if !u.is_meta))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::AssistantMessage;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_assistant_with_tool_use(tool_name: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "Let me run that.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: format!("tu_{}", tool_name),
                    name: tool_name.to_string(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    fn make_assistant_text_only(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            usage: None,
            stop_reason: Some("end_turn".to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    #[test]
    fn test_count_tool_calls() {
        let messages = vec![
            make_assistant_with_tool_use("bash"),
            make_assistant_with_tool_use("bash"),
            make_assistant_with_tool_use("file_read"),
        ];

        assert_eq!(count_tool_calls(&messages, "bash"), 2);
        assert_eq!(count_tool_calls(&messages, "file_read"), 1);
        assert_eq!(count_tool_calls(&messages, "file_write"), 0);
    }

    #[test]
    fn test_last_message_has_tool_use() {
        let messages = vec![
            make_assistant_with_tool_use("bash"),
            make_assistant_text_only("Done!"),
        ];
        // Last assistant message is text-only
        assert!(!last_message_has_tool_use(&messages));

        let messages2 = vec![
            make_assistant_text_only("Let me check"),
            make_assistant_with_tool_use("bash"),
        ];
        // Last assistant message has tool use
        assert!(last_message_has_tool_use(&messages2));
    }

    #[test]
    fn test_last_message_has_tool_use_empty() {
        let messages: Vec<Message> = vec![];
        assert!(!last_message_has_tool_use(&messages));
    }

    #[test]
    fn test_get_last_assistant_text() {
        let messages = vec![
            make_assistant_with_tool_use("bash"),
            make_assistant_text_only("The result is 42."),
        ];
        assert_eq!(
            get_last_assistant_text(&messages),
            Some("The result is 42.".to_string())
        );
    }

    #[test]
    fn test_get_last_assistant_text_none() {
        let messages: Vec<Message> = vec![];
        assert_eq!(get_last_assistant_text(&messages), None);
    }

    #[test]
    fn test_get_last_assistant_tool_use_ids() {
        let messages = vec![make_assistant_with_tool_use("bash")];
        let ids = get_last_assistant_tool_use_ids(&messages);
        assert_eq!(ids, vec!["tu_bash".to_string()]);
    }

    #[test]
    fn test_count_user_messages_excludes_meta() {
        use crate::compact::messages::create_user_message;

        let messages = vec![
            create_user_message("real input", false),
            create_user_message("system injected", true),
            create_user_message("another real", false),
        ];

        assert_eq!(count_user_messages(&messages), 2);
    }

    #[test]
    fn test_get_text_content_user() {
        use crate::compact::messages::create_user_message;
        let msg = create_user_message("hello world", false);
        assert_eq!(get_text_content(&msg), "hello world");
    }

    #[test]
    fn test_get_text_content_assistant() {
        let msg = make_assistant_text_only("response text");
        assert_eq!(get_text_content(&msg), "response text");
    }
}
