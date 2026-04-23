//! TeamSpawn tool — creates and runs a new in-process teammate.
//!
//! This is the conversation-facing entry point for agent teams: the model
//! calls `TeamSpawn` to bring up a named teammate with its own prompt,
//! model, and color. If no team exists yet, an implicit team is created
//! named after the current session and the calling agent becomes team lead.
//!
//! Once spawned, the teammate runs as a tokio task in the same process,
//! reads mailbox messages for its name, and can be addressed through the
//! existing `SendMessage` tool.

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::teams::types::{
    BackendType, InProcessTeammateTaskState, TaskStatus, TeamContext, TeamMember, TeammateIdentity,
    TeammateInfo,
};
use crate::teams::{constants, helpers, identity, in_process::InProcessBackend, runner};
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// TeamSpawn tool.
pub struct TeamSpawnTool;

#[derive(Deserialize)]
struct TeamSpawnInput {
    /// Unique teammate name (used as mailbox name + agent id).
    name: String,
    /// Initial prompt passed to the teammate's QueryEngine.
    prompt: String,
    /// Optional model override (defaults to parent model).
    #[serde(default)]
    model: Option<String>,
    /// Optional UI color (red/blue/green/yellow/purple/orange/pink/cyan).
    #[serde(default)]
    color: Option<String>,
    /// Optional team name — if omitted, uses current team or creates one.
    #[serde(default)]
    team: Option<String>,
    /// Optional description for an implicitly-created team.
    #[serde(default)]
    description: Option<String>,
}

#[async_trait]
impl Tool for TeamSpawnTool {
    fn name(&self) -> &str {
        "TeamSpawn"
    }

    async fn description(&self, _input: &Value) -> String {
        "Spawn a new in-process teammate agent that runs in parallel and can be messaged via SendMessage.".into()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique teammate name (letters, numbers, - and _ only)."
                },
                "prompt": {
                    "type": "string",
                    "description": "Initial instruction for the teammate — what role it plays, what task to start on."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model id. Defaults to the parent agent's model."
                },
                "color": {
                    "type": "string",
                    "description": "Optional UI color tag (red, blue, green, yellow, purple, orange, pink, cyan)."
                },
                "team": {
                    "type": "string",
                    "description": "Optional team name. Defaults to the active team, or creates one tied to the current session."
                },
                "description": {
                    "type": "string",
                    "description": "Optional description used only when an implicit team is created."
                }
            },
            "required": ["name", "prompt"]
        })
    }

    fn is_enabled(&self) -> bool {
        // Always advertise — creating a team through this tool is one of the
        // ways users turn teams on for a session.
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.trim().is_empty() {
            return ValidationResult::Error {
                message: "'name' is required".into(),
                error_code: 400,
            };
        }
        if name == constants::TEAM_LEAD_NAME {
            return ValidationResult::Error {
                message: format!(
                    "'{}' is reserved for the team lead",
                    constants::TEAM_LEAD_NAME
                ),
                error_code: 400,
            };
        }
        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        if prompt.trim().is_empty() {
            return ValidationResult::Error {
                message: "'prompt' is required".into(),
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
        let params: TeamSpawnInput = serde_json::from_value(input)?;

        let app_state = (ctx.get_app_state)();
        let cwd = ctx
            .messages
            .first()
            .map(|_| String::new())
            .unwrap_or_default();
        let cwd = if cwd.is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| ".".into())
        } else {
            cwd
        };

        // Resolve team name — explicit > active context > session-derived.
        let (team_name, freshly_created) = match params.team.clone() {
            Some(t) if !t.trim().is_empty() => (t, false),
            _ => match app_state.team_context.as_ref() {
                Some(tc) if !tc.team_name.is_empty() => (tc.team_name.clone(), false),
                _ => {
                    let base = format!(
                        "session-{}",
                        &ctx.session_id.chars().take(8).collect::<String>()
                    );
                    let tf = helpers::create_team(
                        &base,
                        params.description.clone(),
                        Some(ctx.session_id.clone()),
                        &cwd,
                    )?;
                    info!(team = %tf.name, "implicit team created via TeamSpawn");
                    (tf.name, true)
                }
            },
        };

        // Load (or re-load after creation) the TeamFile to assign a color.
        let mut team_file = helpers::read_team_file(&team_name)?;

        // Reject duplicate member names.
        if team_file.members.iter().any(|m| m.name == params.name) {
            return Ok(ToolResult {
                data: json!({
                    "error": format!("teammate '{}' already exists in team '{}'", params.name, team_name),
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        let color = params
            .color
            .clone()
            .unwrap_or_else(|| helpers::assign_color(&team_file));
        let agent_id = identity::format_agent_id(&params.name, &team_name);
        let now = chrono::Utc::now().timestamp();

        let new_member = TeamMember {
            agent_id: agent_id.clone(),
            name: params.name.clone(),
            agent_type: Some("teammate".into()),
            model: params.model.clone(),
            prompt: Some(params.prompt.clone()),
            color: Some(color.clone()),
            plan_mode_required: None,
            joined_at: now,
            tmux_pane_id: String::new(),
            cwd: cwd.clone(),
            worktree_path: None,
            session_id: None,
            subscriptions: vec![],
            backend_type: Some(BackendType::InProcess),
            is_active: Some(true),
            mode: None,
        };
        team_file.members.push(new_member.clone());
        helpers::write_team_file(&team_name, &team_file)?;

        // Register task state + spawn runner.
        let task_id = format!("in_process_teammate_{}", uuid::Uuid::new_v4());
        let teammate_identity = TeammateIdentity {
            agent_id: agent_id.clone(),
            agent_name: params.name.clone(),
            team_name: team_name.clone(),
            color: Some(color.clone()),
            plan_mode_required: false,
            parent_session_id: ctx.session_id.clone(),
        };
        InProcessBackend::register_task(InProcessTeammateTaskState {
            id: task_id.clone(),
            status: TaskStatus::Running,
            identity: teammate_identity.clone(),
            prompt: params.prompt.clone(),
            model: params.model.clone(),
            abort_handle: None,
            awaiting_plan_approval: false,
            permission_mode: crate::types::tool::PermissionMode::Default,
            error: None,
            pending_user_messages: vec![],
            is_idle: false,
            shutdown_requested: false,
            last_reported_tool_count: 0,
            last_reported_token_count: 0,
        });

        let cancel = CancellationToken::new();
        let _handle = runner::start_runner(runner::InProcessRunnerConfig {
            identity: teammate_identity.clone(),
            task_id: task_id.clone(),
            prompt: params.prompt.clone(),
            model: params.model.clone(),
            cwd: cwd.clone(),
            cancellation: cancel,
        });

        // Update app_state.team_context so the session has a live view.
        let tc_team_name = team_name.clone();
        let tc_agent_id = agent_id.clone();
        let tc_agent_name = params.name.clone();
        let tc_color = color.clone();
        let tc_cwd = cwd.clone();
        let tc_task_id = task_id.clone();
        let tc_description = params.description.clone();
        let tc_freshly_created = freshly_created;
        (ctx.set_app_state)(Box::new(move |mut state| {
            let tc = state.team_context.get_or_insert_with(|| TeamContext {
                team_name: tc_team_name.clone(),
                team_file_path: helpers::team_config_path(&tc_team_name)
                    .to_string_lossy()
                    .into_owned(),
                lead_agent_id: identity::lead_agent_id(&tc_team_name),
                self_agent_id: Some(identity::lead_agent_id(&tc_team_name)),
                self_agent_name: Some(constants::TEAM_LEAD_NAME.into()),
                is_leader: Some(true),
                self_agent_color: None,
                teammates: Default::default(),
            });
            // If the previous team was different (or freshly created), reset.
            if tc.team_name != tc_team_name || tc_freshly_created {
                tc.team_name = tc_team_name.clone();
                tc.team_file_path = helpers::team_config_path(&tc_team_name)
                    .to_string_lossy()
                    .into_owned();
                tc.lead_agent_id = identity::lead_agent_id(&tc_team_name);
                tc.self_agent_id = Some(tc.lead_agent_id.clone());
                tc.self_agent_name = Some(constants::TEAM_LEAD_NAME.into());
                tc.is_leader = Some(true);
                tc.teammates.clear();
            }
            tc.teammates.insert(
                tc_agent_id.clone(),
                TeammateInfo {
                    name: tc_agent_name.clone(),
                    agent_type: Some("teammate".into()),
                    color: Some(tc_color.clone()),
                    tmux_session_name: String::new(),
                    tmux_pane_id: String::new(),
                    cwd: tc_cwd.clone(),
                    worktree_path: None,
                    spawned_at: now,
                },
            );
            // Stash task id alongside so later /team kill can find it.
            let _ = tc_task_id;
            let _ = tc_description;
            state
        }));

        info!(
            team = %team_name,
            agent_id = %agent_id,
            task_id = %task_id,
            "teammate spawned via TeamSpawn"
        );

        Ok(ToolResult {
            data: json!({
                "spawned": true,
                "team": team_name,
                "agent_id": agent_id,
                "task_id": task_id,
                "name": params.name,
                "color": color,
                "implicitly_created_team": freshly_created,
            }),
            new_messages: vec![],
            ..Default::default()
        })
    }

    async fn prompt(&self) -> String {
        "Spawn a new teammate to work in parallel. Use SendMessage to \
         communicate with it after spawn. If no team exists, a session-scoped \
         team is created automatically and you become the team lead."
            .into()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(name) = input.and_then(|v| v.get("name")).and_then(|v| v.as_str()) {
            format!("TeamSpawn({})", name)
        } else {
            "TeamSpawn".into()
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
    fn input_json_schema_requires_name_and_prompt() {
        let tool = TeamSpawnTool;
        let schema = tool.input_json_schema();
        let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
        let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.contains(&"name"));
        assert!(names.contains(&"prompt"));
    }

    #[test]
    fn tool_is_enabled_by_default() {
        assert!(TeamSpawnTool.is_enabled());
    }

    #[test]
    fn tool_name_is_team_spawn() {
        assert_eq!(TeamSpawnTool.name(), "TeamSpawn");
    }
}
