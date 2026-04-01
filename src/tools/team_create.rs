//! TeamCreate tool — creates a new agent team.
//!
//! Corresponds to TypeScript: `tools/TeamCreateTool/`
//!
//! The team leader calls this tool to establish a new team,
//! persisting a TeamFile to disk and updating AppState.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use crate::teams::{helpers, identity};
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// TeamCreate tool.
pub struct TeamCreateTool;

#[derive(Deserialize)]
struct TeamCreateInput {
    team_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
}

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "TeamCreate"
    }

    async fn description(&self, _input: &Value) -> String {
        "Create a new agent team for multi-agent collaboration.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name for the new team"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description of the team's purpose"
                },
                "agent_type": {
                    "type": "string",
                    "description": "Optional agent type for the team leader"
                }
            },
            "required": ["team_name"]
        })
    }

    fn is_enabled(&self) -> bool {
        crate::teams::is_agent_teams_enabled()
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.trim().is_empty() {
            return ValidationResult::Error {
                message: "team_name is required and cannot be empty".into(),
                error_code: 400,
            };
        }
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: TeamCreateInput = serde_json::from_value(input)?;

        // Check if already leading a team
        let app_state = (ctx.get_app_state)();
        if let Some(ref tc) = app_state.team_context {
            if !tc.team_name.is_empty() {
                return Ok(ToolResult {
                    data: json!({
                        "error": format!("Already leading team '{}'. Delete it first.", tc.team_name)
                    }),
                    new_messages: vec![],
                });
            }
        }

        // Resolve CWD
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        // Create the team
        let team_file = helpers::create_team(
            &params.team_name,
            params.description,
            None, // session_id
            &cwd,
        )?;

        let team_name = team_file.name.clone();
        let team_file_path = helpers::team_config_path(&team_name)
            .to_string_lossy()
            .to_string();
        let lead_agent_id = team_file.lead_agent_id.clone();

        // Update AppState with team context
        let tc = crate::teams::types::TeamContext {
            team_name: team_name.clone(),
            team_file_path: team_file_path.clone(),
            lead_agent_id: lead_agent_id.clone(),
            self_agent_id: Some(lead_agent_id.clone()),
            self_agent_name: Some(crate::teams::constants::TEAM_LEAD_NAME.to_string()),
            is_leader: Some(true),
            self_agent_color: None,
            teammates: std::collections::HashMap::new(),
        };

        (ctx.set_app_state)(Box::new(move |mut state| {
            state.team_context = Some(tc);
            state
        }));

        info!(team = %team_name, "team created successfully");

        Ok(ToolResult {
            data: json!({
                "team_name": team_name,
                "team_file_path": team_file_path,
                "lead_agent_id": lead_agent_id,
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Create a new agent team for parallel multi-agent task execution. \
         Provide a descriptive team_name. After creation, use the Agent tool \
         to spawn teammates."
            .to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(name) = input
            .and_then(|v| v.get("team_name"))
            .and_then(|v| v.as_str())
        {
            format!("TeamCreate({})", name)
        } else {
            "TeamCreate".to_string()
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
    fn test_tool_name() {
        let tool = TeamCreateTool;
        assert_eq!(tool.name(), "TeamCreate");
    }

    #[test]
    fn test_schema() {
        let tool = TeamCreateTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("team_name"));
        assert!(props.contains_key("description"));
    }

    #[tokio::test]
    async fn test_validate_empty_name() {
        let tool = TeamCreateTool;
        let input = json!({"team_name": ""});
        let ctx = create_test_context();
        match tool.validate_input(&input, &ctx).await {
            ValidationResult::Error { .. } => {}
            _ => panic!("Expected validation error for empty name"),
        }
    }

    #[tokio::test]
    async fn test_validate_valid_name() {
        let tool = TeamCreateTool;
        let input = json!({"team_name": "my-team"});
        let ctx = create_test_context();
        match tool.validate_input(&input, &ctx).await {
            ValidationResult::Ok => {}
            _ => panic!("Expected Ok for valid name"),
        }
    }

    #[test]
    fn test_user_facing_name() {
        let tool = TeamCreateTool;
        assert_eq!(tool.user_facing_name(None), "TeamCreate");
        let input = json!({"team_name": "research"});
        assert_eq!(
            tool.user_facing_name(Some(&input)),
            "TeamCreate(research)"
        );
    }

    fn create_test_context() -> ToolUseContext {
        use std::sync::Arc;
        use crate::types::app_state::AppState;

        let (tx, rx) = tokio::sync::watch::channel(false);
        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".into(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: rx,
            read_file_state: crate::types::tool::FileStateCache::default(),
            get_app_state: Arc::new(|| AppState::default()),
            set_app_state: Arc::new(|_| {}),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        }
    }
}
