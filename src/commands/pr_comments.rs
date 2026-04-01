//! `/pr-comments` command -- review PR comments.
//!
//! Returns a Query message asking the model to review and respond to
//! pull request comments.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

pub struct PrCommentsHandler;

#[async_trait]
impl CommandHandler for PrCommentsHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let pr_ref = args.trim();

        let prompt = if pr_ref.is_empty() {
            "Please review the comments on the current pull request.\n\n\
             Steps:\n\
             1. Use `gh pr view` to find the current PR\n\
             2. Use `gh pr view --comments` to read all comments\n\
             3. Summarize the feedback and outstanding action items\n\
             4. Suggest code changes to address the review comments\n\
             5. If there are unresolved conversations, provide suggested responses"
                .to_string()
        } else {
            format!(
                "Please review the comments on pull request {}.\n\n\
                 Steps:\n\
                 1. Use `gh pr view {}` to read the PR description\n\
                 2. Use `gh pr view {} --comments` to read all comments\n\
                 3. Summarize the feedback and outstanding action items\n\
                 4. Suggest code changes to address the review comments\n\
                 5. If there are unresolved conversations, provide suggested responses",
                pr_ref, pr_ref, pr_ref
            )
        };

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
    async fn test_pr_comments_no_args() {
        let handler = PrCommentsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                assert_eq!(msgs.len(), 1);
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("current pull request"));
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
    async fn test_pr_comments_with_number() {
        let handler = PrCommentsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("123", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("123"));
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
