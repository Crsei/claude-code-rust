//! JSON-line transport parsing helpers.

use cc_ipc_protocol::{BackendMessage, FrontendMessage};

/// Parsed frontend line or an explicit diagnostic backend message.
#[derive(Debug)]
pub enum ParsedFrontendLine {
    Message(FrontendMessage),
    Diagnostic(BackendMessage),
}

/// Parse one newline-delimited frontend message.
pub fn parse_frontend_line(line: &str) -> ParsedFrontendLine {
    match serde_json::from_str::<FrontendMessage>(line) {
        Ok(msg) => ParsedFrontendLine::Message(msg),
        Err(err) => ParsedFrontendLine::Diagnostic(BackendMessage::Error {
            message: format!("invalid FrontendMessage: {err}; source_phase=client_transport"),
            recoverable: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frontend_message() {
        let parsed = parse_frontend_line(r#"{"type":"quit"}"#);
        assert!(matches!(
            parsed,
            ParsedFrontendLine::Message(FrontendMessage::Quit)
        ));
    }

    #[test]
    fn parse_failure_is_debuggable_diagnostic() {
        let parsed = parse_frontend_line("{bad json");
        let ParsedFrontendLine::Diagnostic(BackendMessage::Error {
            message,
            recoverable,
        }) = parsed
        else {
            panic!("expected diagnostic");
        };

        assert!(recoverable);
        assert!(message.contains("invalid FrontendMessage"));
        assert!(message.contains("source_phase=client_transport"));
    }
}
