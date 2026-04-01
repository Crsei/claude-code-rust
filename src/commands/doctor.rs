//! /doctor command -- runs diagnostics checks.

use std::process::Command as ProcessCommand;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct DoctorHandler;

/// Check whether a command-line program is available on PATH.
fn is_program_available(name: &str) -> bool {
    let cmd = if cfg!(target_os = "windows") {
        ProcessCommand::new("where").arg(name).output()
    } else {
        ProcessCommand::new("which").arg(name).output()
    };
    matches!(cmd, Ok(output) if output.status.success())
}

#[async_trait]
impl CommandHandler for DoctorHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let mut lines = Vec::new();
        lines.push("Diagnostics Report".to_string());
        lines.push("─".repeat(40));

        // 1. Git available
        let git_ok = git2::Repository::discover(".").is_ok();
        lines.push(format!(
            "[{}] Git repository detected",
            if git_ok { "OK" } else { "WARN" }
        ));

        let git_bin = is_program_available("git");
        lines.push(format!(
            "[{}] git binary on PATH",
            if git_bin { "OK" } else { "FAIL" }
        ));

        // 2. API key
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());
        lines.push(format!(
            "[{}] ANTHROPIC_API_KEY set",
            if api_key.is_some() { "OK" } else { "WARN" }
        ));

        // 3. Config directory
        let config_dir = dirs::home_dir()
            .map(|h| h.join(".cc-rust"))
            .unwrap_or_default();
        let config_exists = config_dir.is_dir();
        lines.push(format!(
            "[{}] Config directory (~/.cc-rust/)",
            if config_exists { "OK" } else { "WARN" }
        ));

        // 4. Ripgrep available
        let rg_ok = is_program_available("rg");
        lines.push(format!(
            "[{}] ripgrep (rg) on PATH",
            if rg_ok { "OK" } else { "WARN" }
        ));

        // Summary
        let all_ok = git_bin && api_key.is_some() && config_exists && rg_ok;
        lines.push(String::new());
        if all_ok {
            lines.push("All checks passed.".to_string());
        } else {
            lines.push("Some checks reported warnings. See above for details.".to_string());
        }

        Ok(CommandResult::Output(lines.join("\n")))
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
    async fn test_doctor_runs_without_error() {
        let handler = DoctorHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Diagnostics Report"));
                assert!(text.contains("Git"));
                assert!(text.contains("ANTHROPIC_API_KEY"));
                assert!(text.contains("ripgrep"));
            }
            _ => panic!("Expected Output"),
        }
    }
}
