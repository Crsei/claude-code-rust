//! `/security-review` command -- request a security review of recent changes.
//!
//! Returns a Query message asking the model to perform a security-focused
//! review of the current git changes.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

pub struct SecurityReviewHandler;

#[async_trait]
impl CommandHandler for SecurityReviewHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let focus = if args.trim().is_empty() {
            String::new()
        } else {
            format!("\n\nSpecific focus: {}", args.trim())
        };

        let prompt = format!(
            "Please perform a security review of the recent changes in this repository.\n\n\
             Use `git diff` and `git diff --cached` to examine the changes, then analyze them for:\n\
             - Input validation and sanitization issues\n\
             - Authentication and authorization flaws\n\
             - Injection vulnerabilities (SQL, command, path traversal)\n\
             - Sensitive data exposure (API keys, credentials, PII)\n\
             - Insecure dependencies or configurations\n\
             - Race conditions and TOCTOU vulnerabilities\n\
             - Error handling that leaks internal details\n\n\
             Provide a severity rating for each finding (Critical / High / Medium / Low / Info) \
             and suggest specific fixes.{}",
            focus
        );

        let msg = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            role: "user".to_string(),
            content: MessageContent::Text(prompt),
            timestamp: chrono::Utc::now().timestamp(),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });

        Ok(CommandResult::Query(vec![msg]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
        }
    }

    #[tokio::test]
    async fn test_security_review_returns_query() {
        let handler = SecurityReviewHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                assert_eq!(msgs.len(), 1);
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("security review"));
                        } else {
                            panic!("Expected text content");
                        }
                    }
                    _ => panic!("Expected User message"),
                }
            }
            _ => panic!("Expected Query result"),
        }
    }

    #[tokio::test]
    async fn test_security_review_with_focus() {
        let handler = SecurityReviewHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("auth module", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("auth module"));
                        } else {
                            panic!("Expected text content");
                        }
                    }
                    _ => panic!("Expected User message"),
                }
            }
            _ => panic!("Expected Query result"),
        }
    }
}
