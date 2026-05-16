//! User input processing: slash-command detection, message construction.
//!
//! Corresponds to the input-handling portion of `submitMessage()` in
//! TypeScript's QueryEngine.ts.

use uuid::Uuid;

use cc_types::commands::{CommandDispatcher, ParsedCommand};

use crate::types::message::{Message, MessageContent, UserMessage};

// ---------------------------------------------------------------------------
// ProcessedInput
// ---------------------------------------------------------------------------

/// Result of processing raw user input.
#[derive(Debug, Clone)]
pub struct ProcessedInput {
    /// Messages to append to the conversation (user message + any attachments).
    pub messages: Vec<Message>,
    /// Whether to send the updated conversation to the model.
    /// `false` for purely local slash commands (e.g. `/help`, `/clear`).
    pub should_query: bool,
    /// Tool allow-list overridden by a slash command (e.g. `/allowed-tools`).
    pub allowed_tools: Option<Vec<String>>,
    /// Model override from a slash command.
    pub model: Option<String>,
    /// Text result for local commands (displayed without querying the model).
    pub result_text: Option<String>,
    /// Parsed slash command to execute in the async lifecycle layer.
    pub parsed_command: Option<ParsedCommand>,
}

// ---------------------------------------------------------------------------
// process_user_input
// ---------------------------------------------------------------------------

/// Process raw user input: detect slash commands, build user message.
///
/// 1. If the input starts with `/`, try to match a registered command.
///    - Known commands return a parsed command with `should_query = false`.
///      The async lifecycle layer executes the handler and decides whether the
///      command yields local output or injects messages for a model query.
/// 2. Otherwise, wrap the input in a plain `UserMessage` with
///    `should_query = true`.
///
/// `dispatcher` is the command dispatcher used to parse slash commands.
/// Handler execution stays out of this pure parsing layer because handlers may
/// need async I/O and mutable engine state.
pub fn process_user_input(
    input: &str,
    _messages: &[Message],
    _cwd: &str,
    dispatcher: &dyn CommandDispatcher,
) -> ProcessedInput {
    let trimmed = input.trim();

    // -- Slash-command path ---------------------------------------------------
    if trimmed.starts_with('/') {
        if let Some(parsed) = dispatcher.parse_command_input(trimmed) {
            return ProcessedInput {
                messages: Vec::new(),
                should_query: false,
                allowed_tools: None,
                model: None,
                result_text: None,
                parsed_command: Some(parsed),
            };
        }

        // Unrecognised slash command -- fall through to treat as regular
        // user text so the model can see it (matches TypeScript behaviour).
    }

    // -- Regular user text ----------------------------------------------------
    let user_message = UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        role: "user".to_string(),
        content: MessageContent::Text(trimmed.to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    };

    ProcessedInput {
        messages: vec![Message::User(user_message)],
        should_query: true,
        allowed_tools: None,
        model: None,
        result_text: None,
        parsed_command: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cc_types::commands::{CommandDispatcher, ParsedCommand};

    /// Minimal dispatcher used only in tests.  Recognises `/help` and
    /// `/config`; everything else is treated as regular text.
    struct TestDispatcher;

    impl CommandDispatcher for TestDispatcher {
        fn parse_command_input(&self, input: &str) -> Option<ParsedCommand> {
            let trimmed = input.trim();
            if !trimmed.starts_with('/') {
                return None;
            }
            let without_slash = &trimmed[1..];
            let name = without_slash.split_whitespace().next().unwrap_or("");
            let args = without_slash
                .strip_prefix(name)
                .unwrap_or("")
                .trim()
                .to_string();
            let index = match name {
                "help" => 0,
                "config" => 1,
                _ => return None,
            };
            Some(ParsedCommand { index, args })
        }

        fn command_name(&self, index: usize) -> Option<String> {
            match index {
                0 => Some("help".to_string()),
                1 => Some("config".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn test_regular_text() {
        let d = TestDispatcher;
        let result = process_user_input("Hello, Claude!", &[], "/tmp", &d);
        assert!(result.should_query);
        assert_eq!(result.messages.len(), 1);
        assert!(result.allowed_tools.is_none());
        assert!(result.model.is_none());
        assert!(result.result_text.is_none());
    }

    #[test]
    fn test_slash_command_known() {
        let d = TestDispatcher;
        let result = process_user_input("/help", &[], "/tmp", &d);
        assert!(!result.should_query);
        assert!(result.messages.is_empty());
        assert!(result.allowed_tools.is_none());
        assert!(result.model.is_none());
        assert!(result.result_text.is_none());
        let parsed = result.parsed_command.expect("parsed command");
        assert_eq!(parsed.index, 0);
        assert_eq!(parsed.args, "");
    }

    #[test]
    fn test_slash_command_with_args() {
        let d = TestDispatcher;
        let result = process_user_input("/config set model SOTA", &[], "/tmp", &d);
        assert!(!result.should_query);
        let parsed = result.parsed_command.expect("parsed command");
        assert_eq!(parsed.index, 1);
        assert_eq!(parsed.args, "set model SOTA");
    }

    #[test]
    fn test_unknown_slash_command() {
        let d = TestDispatcher;
        let result = process_user_input("/nonexistent_command", &[], "/tmp", &d);
        // Unknown commands are treated as regular text.
        assert!(result.should_query);
        assert_eq!(result.messages.len(), 1);
    }

    #[test]
    fn test_empty_input() {
        let d = TestDispatcher;
        let result = process_user_input("", &[], "/tmp", &d);
        assert!(result.should_query);
        assert_eq!(result.messages.len(), 1);
    }

    #[test]
    fn test_whitespace_only() {
        let d = TestDispatcher;
        let result = process_user_input("   ", &[], "/tmp", &d);
        assert!(result.should_query);
        assert_eq!(result.messages.len(), 1);
    }
}
