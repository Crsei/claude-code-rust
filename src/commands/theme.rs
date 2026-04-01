//! /theme command -- switch UI theme.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};

pub struct ThemeHandler;

const AVAILABLE_THEMES: &[&str] = &[
    "dark",
    "light",
    "solarized",
    "monokai",
    "nord",
    "dracula",
    "gruvbox",
    "catppuccin",
    "tokyo-night",
    "one-dark",
    "material",
    "everforest",
];

#[async_trait]
impl CommandHandler for ThemeHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim().to_lowercase();

        if arg.is_empty() || arg == "list" {
            let current = ctx
                .app_state
                .settings
                .theme
                .as_deref()
                .unwrap_or("(not set)");

            let theme_list: Vec<String> = AVAILABLE_THEMES
                .iter()
                .map(|t| {
                    if Some(*t) == ctx.app_state.settings.theme.as_deref() {
                        format!("  * {} (current)", t)
                    } else {
                        format!("  - {}", t)
                    }
                })
                .collect();

            return Ok(CommandResult::Output(format!(
                "Current theme: {}\n\nAvailable themes:\n{}\n\nUsage: /theme <name>",
                current,
                theme_list.join("\n")
            )));
        }

        if AVAILABLE_THEMES.contains(&arg.as_str()) {
            ctx.app_state.settings.theme = Some(arg.clone());
            Ok(CommandResult::Output(format!(
                "Theme set to: {}",
                arg
            )))
        } else {
            Ok(CommandResult::Output(format!(
                "Unknown theme: '{}'\n\nAvailable themes: {}",
                arg,
                AVAILABLE_THEMES.join(", ")
            )))
        }
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
    async fn test_list_themes() {
        let handler = ThemeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("list", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(text.contains("Available themes"));
                assert!(text.contains("dark"));
                assert!(text.contains("nord"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_set_theme() {
        let handler = ThemeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("dracula", &mut ctx).await.unwrap();
        assert_eq!(ctx.app_state.settings.theme, Some("dracula".to_string()));
        match result {
            CommandResult::Output(text) => assert!(text.contains("dracula")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_unknown_theme() {
        let handler = ThemeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("neon", &mut ctx).await.unwrap();
        assert!(ctx.app_state.settings.theme.is_none());
        match result {
            CommandResult::Output(text) => assert!(text.contains("Unknown theme")),
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_empty_args_shows_list() {
        let handler = ThemeHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("Available themes")),
            _ => panic!("Expected Output"),
        }
    }
}
