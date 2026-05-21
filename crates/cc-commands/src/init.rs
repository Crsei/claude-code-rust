//! /init command -- initializes project config.

use std::fs;

use anyhow::Result;
use async_trait::async_trait;

use crate::{CommandContext, CommandHandler, CommandResult};

pub struct InitHandler;

const CLAUDE_MD_TEMPLATE: &str = r#"# CLAUDE.md

Project instructions for cc-rust.

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

        if claude_md.exists() {
            return Ok(CommandResult::Output(
                "CLAUDE.md already exists here. Skipping /init to avoid overwriting it."
                    .to_string(),
            ));
        }

        fs::create_dir_all(&config_dir)?;
        let mut created = Vec::new();
        let mut skipped = Vec::new();

        if !settings_file.exists() {
            fs::write(&settings_file, r#"{"model": null, "theme": null}"#)?;
            created.push(settings_file.display().to_string());
        } else {
            skipped.push(settings_file.display().to_string());
        }

        if !claude_md.exists() {
            fs::write(&claude_md, CLAUDE_MD_TEMPLATE)?;
            created.push(claude_md.display().to_string());
        }

        let mut lines = vec!["Project initialization complete.".to_string()];
        if !created.is_empty() {
            lines.push(format!("Created: {}", created.join(", ")));
        }
        if !skipped.is_empty() {
            lines.push(format!("Already present: {}", skipped.join(", ")));
        }

        Ok(CommandResult::Output(lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_bootstrap::SessionId;
    use std::path::PathBuf;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: Default::default(),
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
            CommandResult::Output(text) => assert!(text.contains("Created")),
            _ => panic!("Expected Output"),
        }

        let settings = tmp.join(".cc-rust").join("settings.json");
        let claude_md = tmp.join("CLAUDE.md");
        assert!(settings.exists());
        assert!(claude_md.exists());
        assert!(fs::read_to_string(&claude_md)
            .unwrap()
            .contains("Project instructions"));
        assert!(fs::read_to_string(&claude_md).unwrap().contains("cc-rust"));

        fs::write(&claude_md, "# Existing instructions\n").unwrap();

        // Existing instructions are user-owned; /init must not rewrite them.
        let result2 = handler.execute("", &mut ctx).await.unwrap();
        match result2 {
            CommandResult::Output(text) => {
                assert!(text.contains("CLAUDE.md already exists here"));
                assert!(text.contains("Skipping /init"));
            }
            _ => panic!("Expected Output"),
        }
        assert_eq!(
            fs::read_to_string(&claude_md).unwrap(),
            "# Existing instructions\n"
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
