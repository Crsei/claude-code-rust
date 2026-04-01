//! TeamDelete tool — deletes an existing agent team.
//!
//! Corresponds to TypeScript: `tools/TeamDeleteTool/`
//!
//! Validates no active members remain, then cleans up all team directories
//! (config, inboxes, worktrees, tasks).

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use crate::teams::helpers;
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// TeamDelete tool.
pub struct TeamDeleteTool;

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        "TeamDelete"
    }

    async fn description(&self, _input: &Value) -> String {
        "Delete the current agent team and clean up all resources.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn is_enabled(&self) -> bool {
        crate::teams::is_agent_teams_enabled()
    }

    async fn call(
        &self,
        _input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        // Get team name from AppState
        let app_state = (ctx.get_app_state)();
        let team_name = match app_state.team_context {
            Some(ref tc) if !tc.team_name.is_empty() => tc.team_name.clone(),
            _ => {
                return Ok(ToolResult {
                    data: json!({
                        "success": false,
                        "message": "No active team to delete"
                    }),
                    new_messages: vec![],
                });
            }
        };

        // Read team file and check for active members
        match helpers::read_team_file(&team_name) {
            Ok(tf) => {
                let active = helpers::get_active_members(&tf);
                if !active.is_empty() {
                    return Ok(ToolResult {
                        data: json!({
                            "success": false,
                            "message": format!(
                                "Cannot cleanup team with {} active member(s): {}. \
                                 Stop them first.",
                                active.len(),
                                active.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
                            )
                        }),
                        new_messages: vec![],
                    });
                }
            }
            Err(e) => {
                // Team file might already be gone — proceed with cleanup
                tracing::warn!(error = %e, "team file read error, proceeding with cleanup");
            }
        }

        // Cleanup team directories
        helpers::cleanup_team_directories(&team_name)?;

        // Clear AppState team context
        (ctx.set_app_state)(Box::new(|mut state| {
            state.team_context = None;
            state
        }));

        info!(team = %team_name, "team deleted");

        Ok(ToolResult {
            data: json!({
                "success": true,
                "message": format!("Team '{}' deleted successfully", team_name),
                "team_name": team_name,
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Delete the current agent team. All teammates must be stopped first. \
         This cleans up all team directories, worktrees, and task lists."
            .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "TeamDelete".to_string()
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
        let tool = TeamDeleteTool;
        assert_eq!(tool.name(), "TeamDelete");
    }

    #[test]
    fn test_schema_no_required() {
        let tool = TeamDeleteTool;
        let schema = tool.input_json_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn test_user_facing_name() {
        let tool = TeamDeleteTool;
        assert_eq!(tool.user_facing_name(None), "TeamDelete");
    }
}
