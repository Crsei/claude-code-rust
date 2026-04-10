//! Skill tool — invoke user-defined and bundled skills by name.
//!
//! Corresponds to TypeScript: src/tools/SkillTool/SkillTool.ts
//!
//! The Skill tool looks up a skill definition from the global registry,
//! expands its prompt body with argument substitution, and injects the
//! expanded prompt into the conversation as new messages.
//!
//! Execution modes:
//! - **Inline** (default): skill prompt is added as a user message to the
//!   current conversation; the model processes it directly.
//! - **Fork**: skill runs in a separate sub-agent context (not yet implemented;
//!   falls back to inline).

#[allow(unused_imports)]
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};
use uuid::Uuid;

use crate::skills::{self, SkillContext, SkillDefinition};
#[allow(unused_imports)]
use crate::types::message::{AssistantMessage, ContentBlock, Message, MessageContent, UserMessage};
use crate::types::tool::*;

/// SkillTool — invoke skills (slash command wrappers) by name.
pub struct SkillTool;

#[derive(Deserialize)]
struct SkillInput {
    /// The skill name (e.g. "commit", "simplify", "remember").
    skill: String,
    /// Optional arguments to pass to the skill.
    #[serde(default)]
    args: Option<String>,
}

/// Build the prompt listing available skills for the system prompt.
#[allow(dead_code)]
fn build_skills_listing() -> String {
    let skills = skills::get_model_invocable_skills();
    if skills.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("Available skills:".to_string());
    for skill in &skills {
        let desc = &skill.frontmatter.description;
        let when = skill.frontmatter.when_to_use.as_deref().unwrap_or("");
        if !when.is_empty() {
            lines.push(format!("- {}: {} (use when: {})", skill.name, desc, when));
        } else {
            lines.push(format!("- {}: {}", skill.name, desc));
        }
    }
    lines.join("\n")
}

/// Create a user message containing the expanded skill prompt.
fn make_skill_message(skill: &SkillDefinition, args: &str) -> Message {
    let expanded = skill.expand_prompt(args, None);

    Message::User(UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp(),
        role: "user".to_string(),
        content: MessageContent::Text(expanded),
        is_meta: true,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    async fn description(&self, _input: &Value) -> String {
        "Execute a skill within the conversation.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The skill name (e.g. 'commit', 'simplify')"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"],
            "additionalProperties": false
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        // Depends on the skill — conservative default
        false
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let skill_name = match input.get("skill").and_then(|v| v.as_str()) {
            Some(name) => name.trim_start_matches('/'),
            None => {
                return ValidationResult::Error {
                    message: "Missing required parameter 'skill'.".to_string(),
                    error_code: 1,
                };
            }
        };

        if skill_name.is_empty() {
            return ValidationResult::Error {
                message: "Skill name cannot be empty.".to_string(),
                error_code: 2,
            };
        }

        // Check skill exists and is model-invocable
        match skills::find_skill(skill_name) {
            None => ValidationResult::Error {
                message: format!(
                    "Skill '{}' not found. Available skills: {}",
                    skill_name,
                    skills::get_model_invocable_skills()
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                error_code: 3,
            },
            Some(skill) if !skill.is_model_invocable() => ValidationResult::Error {
                message: format!(
                    "Skill '{}' cannot be invoked by the model (model invocation disabled).",
                    skill_name,
                ),
                error_code: 4,
            },
            Some(_) => ValidationResult::Ok,
        }
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: SkillInput = serde_json::from_value(input)?;
        let skill_name = params.skill.trim_start_matches('/');
        let args = params.args.as_deref().unwrap_or("");

        let skill = skills::find_skill(skill_name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

        info!(
            skill = %skill.name,
            source = ?skill.source,
            context = ?skill.frontmatter.context,
            "invoking skill"
        );

        match skill.frontmatter.context {
            SkillContext::Inline => {
                // Inject skill prompt as a user message into the conversation
                let skill_message = make_skill_message(&skill, args);

                debug!(
                    skill = %skill.name,
                    prompt_len = skill.prompt_body.len(),
                    "inline skill prompt injected"
                );

                let mut result_data = json!({
                    "success": true,
                    "skill": skill.name,
                    "message": format!("Skill '{}' invoked. Follow the instructions in the injected prompt.", skill.name),
                });

                // Pass allowed_tools and model info for context modification
                if !skill.frontmatter.allowed_tools.is_empty() {
                    result_data["allowed_tools"] = json!(skill.frontmatter.allowed_tools);
                }
                if let Some(ref model) = skill.frontmatter.model {
                    result_data["model"] = json!(model);
                }

                Ok(ToolResult {
                    data: result_data,
                    new_messages: vec![skill_message],
                })
            }
            SkillContext::Fork => {
                // Fork context: ideally runs in a sub-agent.
                // For now, fall back to inline execution with a note.
                let skill_message = make_skill_message(&skill, args);

                debug!(
                    skill = %skill.name,
                    "fork skill falling back to inline (sub-agent fork not yet implemented)"
                );

                Ok(ToolResult {
                    data: json!({
                        "success": true,
                        "skill": skill.name,
                        "context": "fork (inline fallback)",
                        "message": format!(
                            "Skill '{}' invoked (fork context, running inline). \
                             Follow the instructions in the injected prompt.",
                            skill.name,
                        ),
                    }),
                    new_messages: vec![skill_message],
                })
            }
        }
    }

    async fn prompt(&self) -> String {
        let listing = build_skills_listing();
        let base = concat!(
            "Execute a skill by name. Skills are user-defined or bundled extensions ",
            "that provide specialized capabilities. When a skill matches the user's ",
            "request, invoke it with the Skill tool before generating other responses.",
        );

        if listing.is_empty() {
            base.to_string()
        } else {
            format!("{}\n\n{}", base, listing)
        }
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(s) = input.and_then(|v| v.get("skill")).and_then(|v| v.as_str()) {
            format!("Skill({})", s)
        } else {
            "Skill".to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_tool_name() {
        let tool = SkillTool;
        assert_eq!(tool.name(), "Skill");
    }

    #[test]
    fn test_skill_tool_schema() {
        let tool = SkillTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("skill"));
        assert!(props.contains_key("args"));
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("skill")));
    }

    #[test]
    fn test_skill_user_facing_name() {
        let tool = SkillTool;
        assert_eq!(tool.user_facing_name(None), "Skill");
        assert_eq!(
            tool.user_facing_name(Some(&json!({"skill": "commit"}))),
            "Skill(commit)"
        );
    }

    #[test]
    fn test_make_skill_message() {
        let skill = SkillDefinition {
            name: "test".to_string(),
            source: skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: skills::SkillFrontmatter {
                description: "Test".to_string(),
                ..Default::default()
            },
            prompt_body: "Hello $ARGUMENTS!".to_string(),
        };

        let msg = make_skill_message(&skill, "world");
        match msg {
            Message::User(u) => {
                assert!(u.is_meta);
                match u.content {
                    MessageContent::Text(text) => assert_eq!(text, "Hello world!"),
                    _ => panic!("expected text content"),
                }
            }
            _ => panic!("expected user message"),
        }
    }

    #[test]
    fn test_build_skills_listing() {
        // Clear and register a test skill
        skills::clear_skills();
        skills::register_skill(SkillDefinition {
            name: "test-skill".to_string(),
            source: skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: skills::SkillFrontmatter {
                description: "A test skill".to_string(),
                when_to_use: Some("When testing".to_string()),
                ..Default::default()
            },
            prompt_body: "Do test things.".to_string(),
        });

        let listing = build_skills_listing();
        assert!(listing.contains("test-skill"));
        assert!(listing.contains("A test skill"));
        assert!(listing.contains("When testing"));

        // Cleanup
        skills::clear_skills();
    }

    #[tokio::test]
    async fn test_validate_missing_skill_name() {
        let tool = SkillTool;
        let state = std::sync::Arc::new(parking_lot::RwLock::new(
            crate::types::app_state::AppState::default(),
        ));
        let state_r = state.clone();
        let state_w = state.clone();

        let ctx = ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".to_string(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: std::sync::Arc::new(move || state_r.read().clone()),
            set_app_state: std::sync::Arc::new(
                move |f: Box<
                    dyn FnOnce(
                        crate::types::app_state::AppState,
                    ) -> crate::types::app_state::AppState,
                >| {
                    let mut s = state_w.write();
                    let old = s.clone();
                    *s = f(old);
                },
            ),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            bg_agent_tx: None,
        };

        // Missing skill field entirely
        let result = tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 1, .. }
        ));

        // Empty skill name
        let result = tool.validate_input(&json!({"skill": ""}), &ctx).await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 2, .. }
        ));

        // Non-existent skill
        skills::clear_skills();
        let result = tool
            .validate_input(&json!({"skill": "nonexistent"}), &ctx)
            .await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 3, .. }
        ));
    }

    #[tokio::test]
    async fn test_validate_existing_skill() {
        // Register a unique skill name to avoid race conditions with other tests
        let unique_name = format!("test-skill-{}", uuid::Uuid::new_v4());
        skills::register_skill(SkillDefinition {
            name: unique_name.clone(),
            source: skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: skills::SkillFrontmatter {
                description: "Test".to_string(),
                ..Default::default()
            },
            prompt_body: "Do it.".to_string(),
        });

        let tool = SkillTool;
        let state = std::sync::Arc::new(parking_lot::RwLock::new(
            crate::types::app_state::AppState::default(),
        ));
        let state_r = state.clone();
        let state_w = state.clone();

        let ctx = ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".to_string(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: std::sync::Arc::new(move || state_r.read().clone()),
            set_app_state: std::sync::Arc::new(
                move |f: Box<
                    dyn FnOnce(
                        crate::types::app_state::AppState,
                    ) -> crate::types::app_state::AppState,
                >| {
                    let mut s = state_w.write();
                    let old = s.clone();
                    *s = f(old);
                },
            ),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            bg_agent_tx: None,
        };

        let result = tool
            .validate_input(&json!({"skill": &unique_name}), &ctx)
            .await;
        assert!(matches!(result, ValidationResult::Ok));

        // With leading /
        let result = tool
            .validate_input(&json!({"skill": format!("/{}", unique_name)}), &ctx)
            .await;
        assert!(matches!(result, ValidationResult::Ok));
    }
}
