//! `/install-slack-app` command -- install the Claude Slack app.
//!
//! Shows the Slack marketplace URL and attempts to open it in the
//! default browser.

use std::process::Command as ProcessCommand;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// The Slack marketplace URL for the Claude app.
const SLACK_URL: &str = "https://slack.com/marketplace/A08SF47R6P4-claude";

/// Handler for the `/install-slack-app` slash command.
pub struct InstallSlackAppHandler;

/// Attempt to open a URL in the default browser.
///
/// Returns `true` if the browser command launched successfully.
fn open_browser(url: &str) -> bool {
    let result = if cfg!(target_os = "windows") {
        ProcessCommand::new("cmd")
            .args(["/C", "start", "", url])
            .output()
    } else if cfg!(target_os = "macos") {
        ProcessCommand::new("open").arg(url).output()
    } else {
        ProcessCommand::new("xdg-open").arg(url).output()
    };

    matches!(result, Ok(output) if output.status.success())
}

#[async_trait]
impl CommandHandler for InstallSlackAppHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let opened = open_browser(SLACK_URL);

        let mut lines = Vec::new();
        lines.push("Claude Slack App".to_string());
        lines.push(format!("Install from: {}", SLACK_URL));

        if opened {
            lines.push("Opening in your default browser...".to_string());
        } else {
            lines.push("Could not open browser automatically. Please visit the URL above.".to_string());
        }

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    async fn test_install_slack_app_shows_url() {
        let handler = InstallSlackAppHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains(SLACK_URL));
                assert!(text.contains("Claude Slack App"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_install_slack_app_browser_message() {
        let handler = InstallSlackAppHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // The output should mention either opening the browser or
                // asking the user to visit the URL manually.
                assert!(
                    text.contains("Opening in your default browser")
                        || text.contains("Could not open browser"),
                    "Unexpected output: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[test]
    fn test_slack_url_constant() {
        assert!(SLACK_URL.starts_with("https://"));
        assert!(SLACK_URL.contains("slack.com/marketplace"));
    }
}
