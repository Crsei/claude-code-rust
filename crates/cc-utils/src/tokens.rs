#![allow(unused)]

use cc_types::message::{ContentBlock, Message, MessageContent, ToolResultContent};

/// Get the context window size for a given model.
/// Returns token count for the model's context window.
fn get_context_window_size(_model: &str) -> u64 {
    200_000
}

/// Average characters per token for English text.
/// This is a rough approximation; actual tokenization varies by model
/// and content type. Code tends to be ~3.5 chars/token, prose ~4-5.
const CHARS_PER_TOKEN: f64 = 4.0;

/// Overhead tokens per message (role metadata, formatting, etc.).
const MESSAGE_OVERHEAD_TOKENS: u64 = 4;

/// Estimate token count for a string using the ~4 chars per token heuristic.
pub fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    (text.len() as f64 / CHARS_PER_TOKEN).ceil() as u64
}

/// Estimate token count for a slice of messages.
/// Accounts for per-message overhead and content within each message.
pub fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_message_tokens).sum()
}

/// Estimate token count for a single message.
fn estimate_message_tokens(msg: &Message) -> u64 {
    let content_tokens = match msg {
        Message::User(user) => estimate_message_content_tokens(&user.content),
        Message::Assistant(assistant) => assistant
            .content
            .iter()
            .map(estimate_content_block_tokens)
            .sum(),
        Message::System(sys) => estimate_tokens(&sys.content),
        Message::Progress(prog) => estimate_tokens(&prog.data.to_string()),
        Message::Attachment(_) => 50, // rough fixed estimate for attachment metadata
    };

    content_tokens + MESSAGE_OVERHEAD_TOKENS
}

/// Estimate tokens for MessageContent (text or blocks).
fn estimate_message_content_tokens(content: &MessageContent) -> u64 {
    match content {
        MessageContent::Text(text) => estimate_tokens(text),
        MessageContent::Blocks(blocks) => blocks.iter().map(estimate_content_block_tokens).sum(),
    }
}

/// Estimate tokens for a single content block.
fn estimate_content_block_tokens(block: &ContentBlock) -> u64 {
    match block {
        ContentBlock::Text { text } => estimate_tokens(text),
        ContentBlock::ToolUse { id, name, input } => {
            // Tool use has metadata overhead plus the JSON input
            let input_str = input.to_string();
            estimate_tokens(id) + estimate_tokens(name) + estimate_tokens(&input_str) + 10
            // structural overhead
        }
        ContentBlock::ToolResult { content, .. } => estimate_tool_result_content_tokens(content),
        ContentBlock::Thinking { thinking, .. } => estimate_tokens(thinking),
        ContentBlock::RedactedThinking { data } => estimate_tokens(data),
        ContentBlock::Image { source } => {
            // Images are typically encoded as base64; the API counts them
            // differently. A rough estimate based on the encoded data size.
            // Most images use ~1600 tokens for a typical screenshot.
            let data_len = source.data.len();
            // Base64 encodes ~3 bytes into 4 chars, so raw size is ~75% of data_len.
            // Then ~750 bytes per token for images is a rough Anthropic estimate.
            let raw_bytes = (data_len as f64 * 0.75) as u64;
            (raw_bytes / 750).max(100)
        }
    }
}

/// Estimate tokens for tool result content.
fn estimate_tool_result_content_tokens(content: &ToolResultContent) -> u64 {
    match content {
        ToolResultContent::Text(text) => estimate_tokens(text),
        ToolResultContent::Blocks(blocks) => blocks.iter().map(estimate_content_block_tokens).sum(),
    }
}

/// Check if estimated tokens in the given messages exceed the model's
/// context window threshold (80% of context window).
///
/// This is used to determine if compaction should be triggered.
pub fn is_over_token_limit(messages: &[Message], model: &str) -> bool {
    let estimated = estimate_messages_tokens(messages);
    let context_window = get_context_window_size(model);
    let threshold = (context_window as f64 * 0.8) as u64;
    estimated > threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_types::message::{Message, MessageContent, UserMessage};
    use uuid::Uuid;

    fn make_user_message(text: &str, is_meta: bool) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text(text.to_string()),
            is_meta,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        // "hello" = 5 chars, ceil(5/4) = 2 tokens
        assert_eq!(estimate_tokens("hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_longer() {
        // 100 chars -> 25 tokens
        let text = "a".repeat(100);
        assert_eq!(estimate_tokens(&text), 25);
    }

    #[test]
    fn test_estimate_messages_tokens() {
        let messages = vec![
            make_user_message("hello world", false),
            make_user_message("how are you?", false),
        ];

        let tokens = estimate_messages_tokens(&messages);
        // Each message: ceil(len/4) + 4 overhead
        // "hello world" = 11 chars -> ceil(11/4) = 3 + 4 = 7
        // "how are you?" = 12 chars -> ceil(12/4) = 3 + 4 = 7
        // Total = 14
        assert_eq!(tokens, 14);
    }

    #[test]
    fn test_is_over_token_limit() {
        // Threshold for any model is 80% of 200k = 160k tokens
        // A message with enough text to exceed 160k tokens:
        // 160001 tokens * 4 chars = 640004 chars
        let large_text = "a".repeat(640_004);
        let messages = vec![make_user_message(&large_text, false)];
        assert!(is_over_token_limit(&messages, "claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_not_over_token_limit() {
        let messages = vec![make_user_message("hello", false)];
        assert!(!is_over_token_limit(&messages, "claude-sonnet-4-20250514"));
    }
}
