//! `/install-github-app` command -- set up GitHub Actions integration.
//!
//! Provides a text-based setup guide for configuring GitHub Actions with
//! Claude Code. Checks for the `gh` CLI and shows step-by-step instructions.

use std::process::Command as ProcessCommand;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

/// Handler for the `/install-github-app` slash command.
pub struct InstallGithubAppHandler;

/// Check whether the GitHub CLI (`gh`) is available on PATH.
fn is_gh_available() -> bool {
    let cmd = if cfg!(target_os = "windows") {
        ProcessCommand::new("where").arg("gh").output()
    } else {
        ProcessCommand::new("which").arg("gh").output()
    };
    matches!(cmd, Ok(output) if output.status.success())
}

#[async_trait]
impl CommandHandler for InstallGithubAppHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        // Sub-command: check gh availability
        if arg == "check" {
            let available = is_gh_available();
            return if available {
                Ok(CommandResult::Output(
                    "[OK] GitHub CLI (gh) is available on PATH.".to_string(),
                ))
            } else {
                Ok(CommandResult::Output(
                    "[FAIL] GitHub CLI (gh) not found.\n\
                     Install it from: https://cli.github.com/"
                        .to_string(),
                ))
            };
        }

        // Full setup guide
        let gh_status = if is_gh_available() {
            "[OK] GitHub CLI (gh) detected on PATH."
        } else {
            "[WARN] GitHub CLI (gh) not found. Install from: https://cli.github.com/"
        };

        let guide = format!(
            "GitHub Actions Setup Guide\n\
             {separator}\n\
             \n\
             Step 1: Ensure GitHub CLI is installed and authenticated\n\
             {gh_status}\n\
             Run `gh auth status` to verify authentication.\n\
             \n\
             Step 2: Set ANTHROPIC_API_KEY as a repository secret\n\
             Run: gh secret set ANTHROPIC_API_KEY\n\
             This stores your API key securely for GitHub Actions.\n\
             \n\
             Step 3: Create workflow files\n\
             Create `.github/workflows/claude.yml` in your repository with\n\
             the Claude Code GitHub Action configuration.\n\
             \n\
             For more details, see:\n\
             https://docs.anthropic.com/en/docs/claude-code/github-actions",
            separator = "─".repeat(40),
            gh_status = gh_status,
        );

        Ok(CommandResult::Output(guide))
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
    async fn test_install_github_app_full_guide() {
        let handler = InstallGithubAppHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("GitHub Actions Setup Guide"));
                assert!(text.contains("Step 1"));
                assert!(text.contains("Step 2"));
                assert!(text.contains("Step 3"));
                assert!(text.contains("ANTHROPIC_API_KEY"));
                assert!(text.contains("claude.yml"));
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[tokio::test]
    async fn test_install_github_app_check() {
        let handler = InstallGithubAppHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("check", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Either gh is found or not -- both are valid.
                assert!(
                    text.contains("[OK] GitHub CLI") || text.contains("[FAIL] GitHub CLI"),
                    "Unexpected output: {}",
                    text
                );
            }
            _ => panic!("Expected Output result"),
        }
    }

    #[test]
    fn test_is_gh_available_returns_bool() {
        // Just verify the function executes without panicking.
        let result = is_gh_available();
        assert!(result == true || result == false);
    }
}
