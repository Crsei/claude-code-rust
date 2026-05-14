//! /init command -- initializes project config.

use std::fs;

use anyhow::Result;
use async_trait::async_trait;

use cc_commands::{CommandContext, CommandHandler, CommandResult};

pub struct InitHandler;

const CLAUDE_MD_TEMPLATE: &str = r#"# CLAUDE.md

Project instructions for Claude Code.

## Build And Test

- Add project-specific commands here.

## Project Notes

- Add coding conventions, architecture notes, and review expectations here.
"#;

#[async_trait]
impl CommandHandler for InitHandler {
    async fn execute(&self, _args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let config_dir = ctx.cwd.join(".cc-rust");
        let settings_file = config_dir.join("settings.json");
        let claude_md = ctx.cwd.join("CLAUDE.md");

        if settings_file.exists() && claude_md.exists() {
            return Ok(CommandResult::Output(format!(
                "Project already initialized. Config at: {}; instructions at: {}",
                settings_file.display(),
                claude_md.display()
            )));
        }

        fs::create_dir_all(&config_dir)?;
        let mut created = Vec::new();

        if !settings_file.exists() {
            fs::write(&settings_file, r#"{"model": null, "theme": null}"#)?;
            created.push(settings_file.display().to_string());
        }

        if !claude_md.exists() {
            fs::write(&claude_md, CLAUDE_MD_TEMPLATE)?;
            created.push(claude_md.display().to_string());
        }

        Ok(CommandResult::Output(format!(
            "Project initialized. Created {}",
            created.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_bootstrap::SessionId;
    use cc_engine::types::app_state::AppState;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_init_creates_config() {
        let tmp = std::env::temp_dir().join("cc_rust_init_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let handler = InitHandler;
        let mut ctx = test_ctx();
        ctx.cwd = tmp.clone();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("initialized")),
            _ => panic!("Expected Output"),
        }

        let settings = tmp.join(".cc-rust").join("settings.json");
        let claude_md = tmp.join("CLAUDE.md");
        assert!(settings.exists());
        assert!(claude_md.exists());
        assert!(fs::read_to_string(&claude_md)
            .unwrap()
            .contains("Project instructions"));

        fs::write(&claude_md, "# Existing instructions\n").unwrap();

        // Second call should say already initialized
        let result2 = handler.execute("", &mut ctx).await.unwrap();
        match result2 {
            CommandResult::Output(text) => assert!(text.contains("already")),
            _ => panic!("Expected Output"),
        }
        assert_eq!(
            fs::read_to_string(&claude_md).unwrap(),
            "# Existing instructions\n"
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
