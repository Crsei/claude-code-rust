//! `/commit-push-pr` command -- commit, push, and create a PR.
//!
//! Returns a Query message asking the model to commit all changes,
//! push the branch, and create a pull request.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::types::message::{Message, MessageContent, UserMessage};

pub struct CommitPushPrHandler;

#[async_trait]
impl CommandHandler for CommitPushPrHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let extra_instructions = if args.trim().is_empty() {
            String::new()
        } else {
            format!("\n\nAdditional instructions: {}", args.trim())
        };

        let prompt = format!(
            "Please commit all current changes, push to the remote, and create a pull request.\n\n\
             Steps:\n\
             1. Run `git status` to see all changes\n\
             2. Run `git diff` and `git diff --cached` to review the changes\n\
             3. Stage the relevant files (use `git add` for specific files, avoid adding \
                sensitive files like .env or credentials)\n\
             4. Create a commit with a clear, descriptive message\n\
             5. Push the current branch to the remote with `git push -u origin HEAD`\n\
             6. Create a pull request using `gh pr create` with a descriptive title and body\n\
             7. Report the PR URL when done{}",
            extra_instructions
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
    async fn test_commit_push_pr_returns_query() {
        let handler = CommitPushPrHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                assert_eq!(msgs.len(), 1);
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("commit"));
                            assert!(text.contains("push"));
                            assert!(text.contains("pull request"));
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
    async fn test_commit_push_pr_with_instructions() {
        let handler = CommitPushPrHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("target main branch", &mut ctx).await.unwrap();
        match result {
            CommandResult::Query(msgs) => {
                match &msgs[0] {
                    Message::User(um) => {
                        if let MessageContent::Text(text) = &um.content {
                            assert!(text.contains("target main branch"));
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
