//! /skills command -- list available skills.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::skills;

pub struct SkillsHandler;

#[async_trait]
impl CommandHandler for SkillsHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let arg = args.trim();

        let all = skills::get_all_skills();

        // Detail view for a specific skill
        if !arg.is_empty() && arg != "list" {
            if let Some(skill) = all.iter().find(|s| s.name == arg) {
                let mut lines = Vec::new();
                lines.push(format!("Skill: {}", skill.display_name()));
                lines.push(format!("Source: {:?}", skill.source));
                lines.push(format!("Description: {}", skill.frontmatter.description));
                if let Some(ref when) = skill.frontmatter.when_to_use {
                    lines.push(format!("When to use: {}", when));
                }
                if !skill.frontmatter.allowed_tools.is_empty() {
                    lines.push(format!(
                        "Allowed tools: {}",
                        skill.frontmatter.allowed_tools.join(", ")
                    ));
                }
                lines.push(format!(
                    "User invocable: {}",
                    skill.frontmatter.user_invocable
                ));
                lines.push(format!("Model invocable: {}", skill.is_model_invocable()));
                return Ok(CommandResult::Output(lines.join("\n")));
            } else {
                return Ok(CommandResult::Output(format!(
                    "Skill '{}' not found. Use /skills to list all available skills.",
                    arg
                )));
            }
        }

        if all.is_empty() {
            return Ok(CommandResult::Output(
                "No skills loaded.\n\n\
                 Bundled skills: simplify, remember, debug, stuck, update-config\n\
                 Place custom skills in ~/.cc-rust/skills/<name>/SKILL.md"
                    .to_string(),
            ));
        }

        let mut lines = Vec::new();
        lines.push(format!("Available Skills ({} total)", all.len()));
        lines.push("─".repeat(50));

        for skill in &all {
            let source_tag = match &skill.source {
                skills::SkillSource::Bundled => "[bundled]",
                skills::SkillSource::User => "[user]",
                skills::SkillSource::Project => "[project]",
                skills::SkillSource::Plugin(_) => "[plugin]",
                skills::SkillSource::Mcp(_) => "[mcp]",
            };
            lines.push(format!(
                "  {} {} -- {}",
                source_tag,
                skill.display_name(),
                skill.frontmatter.description
            ));
        }

        lines.push(String::new());
        lines.push("Use /skills <name> for details on a specific skill.".to_string());

        Ok(CommandResult::Output(lines.join("\n")))
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
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_skills_list() {
        let handler = SkillsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                // Either shows skills or the "no skills loaded" message
                assert!(text.contains("Available Skills") || text.contains("No skills loaded"));
            }
            _ => panic!("Expected Output"),
        }
    }

    #[tokio::test]
    async fn test_unknown_skill() {
        let handler = SkillsHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("nonexistent", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => assert!(text.contains("not found")),
            _ => panic!("Expected Output"),
        }
    }
}
