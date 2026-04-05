//! /init command -- initializes project config.

use std::fs;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::utils::cwd::get_cwd;

pub struct InitHandler;

#[async_trait]
impl CommandHandler for InitHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let cwd = get_cwd();
        let config_dir = cwd.join(".cc-rust");
        let settings_file = config_dir.join("settings.json");

        if settings_file.exists() {
            return Ok(CommandResult::Output(format!(
                "Project already initialized. Config at: {}",
                settings_file.display()
            )));
        }

        fs::create_dir_all(&config_dir)?;
        fs::write(
            &settings_file,
            r#"{"model": null, "theme": null}"#,
        )?;

        Ok(CommandResult::Output(format!(
            "Project initialized. Created {}",
            settings_file.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;
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

        crate::utils::cwd::set_cwd(&tmp.to_string_lossy());

        let handler = InitHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("initialized")),
            _ => panic!("Expected Output"),
        }

        let settings = tmp.join(".cc-rust").join("settings.json");
        assert!(settings.exists());

        // Second call should say already initialized
        let result2 = handler.execute("", &mut ctx).await.unwrap();
        match result2 {
            CommandResult::Output(text) => assert!(text.contains("already")),
            _ => panic!("Expected Output"),
        }

        let _ = fs::remove_dir_all(&tmp);
        crate::utils::cwd::reset_cwd();
    }
}
