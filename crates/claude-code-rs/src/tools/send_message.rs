//! SendMessage tool — routes messages between teammates.
//!
//! Corresponds to TypeScript: `tools/SendMessageTool/`
//!
//! Handles:
//! - Plain text message to a specific teammate
//! - Broadcast ("*") to all teammates
//! - Structured shutdown request/response
//! - Plan approval response

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::teams::in_process::InProcessBackend;
use crate::teams::types::TeammateMessage;
use crate::teams::{constants, helpers, identity, mailbox, protocol};
use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// SendMessage tool.
pub struct SendMessageTool;

#[derive(Deserialize)]
struct SendMessageInput {
    /// Recipient: teammate name, "*" for broadcast.
    to: String,
    /// The message text or structured content.
    message: String,
    /// Optional 5-10 word summary.
    #[serde(default)]
    summary: Option<String>,
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "SendMessage"
    }

    async fn description(&self, _input: &Value) -> String {
        "Send a message to a teammate or broadcast to all teammates.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient teammate name, or \"*\" for broadcast"
                },
                "message": {
                    "type": "string",
                    "description": "Message text or structured JSON message"
                },
                "summary": {
                    "type": "string",
                    "description": "Brief 5-10 word summary of the message"
                }
            },
            "required": ["to", "message"]
        })
    }

    fn is_enabled(&self) -> bool {
        // Always advertise the tool to the model; the call path gracefully
        // rejects invocations when no team context is active. This lets a
        // conversation spin up a team via `/team create` or `TeamSpawn`
        // without the tool being filtered out at startup.
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if to.trim().is_empty() {
            return ValidationResult::Error {
                message: "'to' field is required".into(),
                error_code: 400,
            };
        }
        let msg = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
        if msg.trim().is_empty() {
            return ValidationResult::Error {
                message: "'message' field is required".into(),
                error_code: 400,
            };
        }
        ValidationResult::Ok
    }

    fn backfill_observable_input(&self, input: &mut serde_json::Map<String, Value>) {
        if input.contains_key("type") {
            return;
        }

        let Some(to) = input.get("to").and_then(Value::as_str).map(str::to_string) else {
            return;
        };
        let Some(message) = input.get("message").cloned() else {
            return;
        };

        match message {
            Value::String(text) if to == "*" => {
                input.insert("type".to_string(), Value::String("broadcast".to_string()));
                input.insert("content".to_string(), Value::String(text));
            }
            Value::String(text) => {
                input.insert("type".to_string(), Value::String("message".to_string()));
                input.insert("recipient".to_string(), Value::String(to));
                input.insert("content".to_string(), Value::String(text));
            }
            Value::Object(message) => {
                if let Some(value) = message.get("type") {
                    input.insert("type".to_string(), value.clone());
                }
                input.insert("recipient".to_string(), Value::String(to));
                if let Some(value) = message.get("request_id") {
                    input.insert("request_id".to_string(), value.clone());
                }
                if let Some(value) = message.get("approve") {
                    input.insert("approve".to_string(), value.clone());
                }
                if let Some(value) = message.get("reason").or_else(|| message.get("feedback")) {
                    input.insert("content".to_string(), value.clone());
                }
            }
            _ => {}
        }
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: SendMessageInput = serde_json::from_value(input)?;

        // Get team context
        let app_state = (ctx.get_app_state)();
        let team_ctx = match app_state.team_context {
            Some(ref tc) if !tc.team_name.is_empty() => tc.clone(),
            _ => {
                return Ok(ToolResult {
                    data: json!({"error": "No active team. Create a team first."}),
                    new_messages: vec![],
                    ..Default::default()
                });
            }
        };

        let team_name = &team_ctx.team_name;
        let sender_name = team_ctx
            .self_agent_name
            .as_deref()
            .unwrap_or(constants::TEAM_LEAD_NAME);

        // Check if this is a structured protocol message
        if protocol::is_structured_protocol_message(&params.message) {
            return handle_protocol_message(
                &params.message,
                &params.to,
                sender_name,
                team_name,
                &team_ctx,
            );
        }

        // Plain text message routing
        if params.to == "*" {
            // Broadcast to all teammates
            return handle_broadcast(
                sender_name,
                &params.message,
                params.summary.as_deref(),
                team_name,
            );
        }

        // Single recipient
        handle_single_message(
            sender_name,
            &params.to,
            &params.message,
            params.summary.as_deref(),
            team_name,
        )
    }

    async fn prompt(&self) -> String {
        "Send a message to a teammate or broadcast to all teammates. \
         Use the 'to' field with a teammate name or '*' for broadcast. \
         Include a brief summary for quick context."
            .to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(to) = input.and_then(|v| v.get("to")).and_then(|v| v.as_str()) {
            format!("SendMessage(to: {})", to)
        } else {
            "SendMessage".to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Message routing implementations
// ---------------------------------------------------------------------------

/// Send a message to a single recipient.
fn handle_single_message(
    sender: &str,
    recipient: &str,
    text: &str,
    summary: Option<&str>,
    team_name: &str,
) -> Result<ToolResult> {
    let team_file = helpers::read_team_file(team_name)?;
    let Some(member) = team_file.members.iter().find(|m| m.name == recipient) else {
        bail!(
            "No active team member named '{}' exists in team '{}'",
            recipient,
            team_name
        );
    };
    if member.is_active == Some(false) {
        bail!(
            "Team member '{}' in team '{}' is inactive",
            recipient,
            team_name
        );
    }

    let now = chrono::Utc::now();
    let message = TeammateMessage {
        from: sender.to_string(),
        text: text.to_string(),
        timestamp: now.to_rfc3339(),
        read: false,
        color: identity::get_teammate_color(),
        summary: summary.map(|s| s.to_string()),
    };

    mailbox::write_to_mailbox(recipient, message, team_name)?;

    debug!(from = sender, to = recipient, "message sent");

    Ok(ToolResult {
        data: json!({
            "sent": true,
            "to": recipient,
            "from": sender,
        }),
        new_messages: vec![],
        ..Default::default()
    })
}

/// Broadcast a message to all non-self team members.
fn handle_broadcast(
    sender: &str,
    text: &str,
    summary: Option<&str>,
    team_name: &str,
) -> Result<ToolResult> {
    let team_file = helpers::read_team_file(team_name)?;
    let recipients: Vec<String> = team_file
        .members
        .iter()
        .filter(|m| m.name != sender && m.is_active != Some(false))
        .map(|m| m.name.clone())
        .collect();

    let now = chrono::Utc::now();
    for recipient in &recipients {
        let message = TeammateMessage {
            from: sender.to_string(),
            text: text.to_string(),
            timestamp: now.to_rfc3339(),
            read: false,
            color: identity::get_teammate_color(),
            summary: summary.map(|s| s.to_string()),
        };
        mailbox::write_to_mailbox(recipient, message, team_name)?;
    }

    info!(
        from = sender,
        recipient_count = recipients.len(),
        "broadcast sent"
    );

    Ok(ToolResult {
        data: json!({
            "sent": true,
            "to": "*",
            "recipients": recipients,
            "from": sender,
        }),
        new_messages: vec![],
        ..Default::default()
    })
}

/// Handle a structured protocol message in the `message` field.
fn handle_protocol_message(
    raw_message: &str,
    to: &str,
    sender: &str,
    team_name: &str,
    _team_ctx: &crate::teams::types::TeamContext,
) -> Result<ToolResult> {
    let proto = match protocol::try_parse_protocol_message(raw_message) {
        Some(p) => p,
        None => bail!("Failed to parse protocol message"),
    };

    match proto {
        protocol::ProtocolMessage::ShutdownRequest { .. } => {
            // Forward shutdown request to target teammate
            let now = chrono::Utc::now();
            let message = TeammateMessage {
                from: sender.to_string(),
                text: raw_message.to_string(),
                timestamp: now.to_rfc3339(),
                read: false,
                color: None,
                summary: Some("Shutdown request".into()),
            };
            mailbox::write_to_mailbox(to, message, team_name)?;
            Ok(ToolResult {
                data: json!({"sent": true, "type": "shutdown_request", "to": to}),
                new_messages: vec![],
                ..Default::default()
            })
        }

        protocol::ProtocolMessage::ShutdownApproved { .. } => {
            // Shutdown approval — mark teammate as stopped and inactive
            let agent_id = identity::format_agent_id(to, team_name);

            // Update team file to mark inactive
            let _ = helpers::set_member_active(team_name, &agent_id, false);

            Ok(ToolResult {
                data: json!({
                    "sent": true,
                    "type": "shutdown_approved",
                    "to": to,
                }),
                new_messages: vec![],
                ..Default::default()
            })
        }

        protocol::ProtocolMessage::ShutdownRejected { ref reason, .. } => Ok(ToolResult {
            data: json!({
                "type": "shutdown_rejected",
                "to": to,
                "reason": reason,
            }),
            new_messages: vec![],
            ..Default::default()
        }),

        protocol::ProtocolMessage::PlanApprovalRequest {
            ref from,
            ref request_id,
            ..
        } => {
            let requester = if from.trim().is_empty() {
                sender
            } else {
                from.as_str()
            };
            let marked =
                InProcessBackend::set_plan_approval_pending_by_agent(requester, team_name, true);
            let now = chrono::Utc::now();
            let message = TeammateMessage {
                from: sender.to_string(),
                text: raw_message.to_string(),
                timestamp: now.to_rfc3339(),
                read: false,
                color: None,
                summary: Some("Plan approval request".into()),
            };
            mailbox::write_to_mailbox(to, message, team_name)?;
            Ok(ToolResult {
                data: json!({
                    "sent": true,
                    "type": "plan_approval_request",
                    "to": to,
                    "from": requester,
                    "request_id": request_id,
                    "awaiting_plan_approval": marked,
                }),
                new_messages: vec![],
                ..Default::default()
            })
        }

        protocol::ProtocolMessage::PlanApprovalResponse {
            approved,
            ref feedback,
            ref permission_mode,
            ..
        } => {
            // Forward plan approval to teammate
            let permission_mode = permission_mode.as_deref().map(PermissionMode::parse);
            let pending_cleared =
                InProcessBackend::set_plan_approval_pending_by_agent(to, team_name, false);
            if let Some(mode) = permission_mode.clone() {
                InProcessBackend::set_permission_mode_by_agent(to, team_name, mode);
            }
            let now = chrono::Utc::now();
            let message = TeammateMessage {
                from: sender.to_string(),
                text: raw_message.to_string(),
                timestamp: now.to_rfc3339(),
                read: false,
                color: None,
                summary: Some("Plan approval response".into()),
            };
            mailbox::write_to_mailbox(to, message, team_name)?;
            Ok(ToolResult {
                data: json!({
                    "sent": true,
                    "type": "plan_approval_response",
                    "to": to,
                    "approved": approved,
                    "feedback": feedback,
                    "awaiting_plan_approval": false,
                    "pending_cleared": pending_cleared,
                    "permission_mode": permission_mode.as_ref().map(PermissionMode::as_str),
                }),
                new_messages: vec![],
                ..Default::default()
            })
        }

        _ => {
            // Forward any other protocol message directly
            let now = chrono::Utc::now();
            let message = TeammateMessage {
                from: sender.to_string(),
                text: raw_message.to_string(),
                timestamp: now.to_rfc3339(),
                read: false,
                color: None,
                summary: None,
            };
            mailbox::write_to_mailbox(to, message, team_name)?;
            Ok(ToolResult {
                data: json!({"sent": true, "to": to}),
                new_messages: vec![],
                ..Default::default()
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teams::in_process::InProcessBackend;
    use crate::teams::types::{
        InProcessTeammateTaskState, TaskStatus, TeamContext, TeammateIdentity,
    };

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn test_tool_name() {
        let tool = SendMessageTool;
        assert_eq!(tool.name(), "SendMessage");
    }

    #[test]
    fn test_schema() {
        let tool = SendMessageTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("to"));
        assert!(props.contains_key("message"));
        assert!(props.contains_key("summary"));
    }

    #[tokio::test]
    async fn test_validate_empty_to() {
        let tool = SendMessageTool;
        let input = json!({"to": "", "message": "hello"});
        let ctx = create_test_context();
        match tool.validate_input(&input, &ctx).await {
            ValidationResult::Error { .. } => {}
            _ => panic!("Expected error for empty 'to'"),
        }
    }

    #[tokio::test]
    async fn test_validate_empty_message() {
        let tool = SendMessageTool;
        let input = json!({"to": "worker", "message": ""});
        let ctx = create_test_context();
        match tool.validate_input(&input, &ctx).await {
            ValidationResult::Error { .. } => {}
            _ => panic!("Expected error for empty message"),
        }
    }

    #[tokio::test]
    async fn test_validate_valid() {
        let tool = SendMessageTool;
        let input = json!({"to": "worker", "message": "do this"});
        let ctx = create_test_context();
        match tool.validate_input(&input, &ctx).await {
            ValidationResult::Ok => {}
            _ => panic!("Expected Ok for valid input"),
        }
    }

    #[test]
    fn test_user_facing_name() {
        let tool = SendMessageTool;
        assert_eq!(tool.user_facing_name(None), "SendMessage");
        let input = json!({"to": "researcher"});
        assert_eq!(
            tool.user_facing_name(Some(&input)),
            "SendMessage(to: researcher)"
        );
    }

    #[test]
    #[serial_test::serial]
    fn plan_approval_request_marks_teammate_pending_and_forwards_to_leader() {
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());
        InProcessBackend::clear_registry();
        register_test_teammate(false, PermissionMode::Plan);

        let raw = json!({
            "type": "plan_approval_request",
            "from": "worker",
            "timestamp": "2026-05-05T00:00:00Z",
            "planFilePath": ".cc-rust/current-plan.md",
            "planContent": "Implement in two steps",
            "requestId": "plan_approval-worker-1",
        })
        .to_string();

        let result = handle_protocol_message(
            &raw,
            constants::TEAM_LEAD_NAME,
            "worker",
            "team",
            &TeamContext::default(),
        )
        .unwrap();
        assert_eq!(result.data["type"], "plan_approval_request");
        assert_eq!(result.data["awaiting_plan_approval"], true);

        let snapshot = InProcessBackend::task_snapshots().remove(0);
        assert!(snapshot.awaiting_plan_approval);
        let inbox = mailbox::read_mailbox(constants::TEAM_LEAD_NAME, "team").unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].summary.as_deref(), Some("Plan approval request"));

        InProcessBackend::clear_registry();
    }

    #[test]
    #[serial_test::serial]
    fn plan_approval_response_clears_pending_and_updates_permission_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());
        InProcessBackend::clear_registry();
        register_test_teammate(true, PermissionMode::Plan);

        let raw = json!({
            "type": "plan_approval_response",
            "requestId": "plan_approval-worker-1",
            "approved": true,
            "timestamp": "2026-05-05T00:01:00Z",
            "permissionMode": "acceptEdits",
        })
        .to_string();

        let result = handle_protocol_message(
            &raw,
            "worker",
            constants::TEAM_LEAD_NAME,
            "team",
            &TeamContext::default(),
        )
        .unwrap();
        assert_eq!(result.data["type"], "plan_approval_response");
        assert_eq!(result.data["awaiting_plan_approval"], false);
        assert_eq!(result.data["pending_cleared"], true);
        assert_eq!(result.data["permission_mode"], "acceptEdits");

        let snapshot = InProcessBackend::task_snapshots().remove(0);
        assert!(!snapshot.awaiting_plan_approval);
        assert_eq!(snapshot.permission_mode, PermissionMode::AcceptEdits);
        let inbox = mailbox::read_mailbox("worker", "team").unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].summary.as_deref(), Some("Plan approval response"));

        InProcessBackend::clear_registry();
    }

    fn register_test_teammate(awaiting_plan_approval: bool, permission_mode: PermissionMode) {
        InProcessBackend::register_task(InProcessTeammateTaskState {
            id: "task-1".into(),
            status: TaskStatus::Running,
            identity: TeammateIdentity {
                agent_id: "worker@team".into(),
                agent_name: "worker".into(),
                team_name: "team".into(),
                color: None,
                plan_mode_required: true,
                parent_session_id: "session".into(),
            },
            prompt: "initial".into(),
            model: None,
            abort_handle: None,
            cancellation_token: None,
            awaiting_plan_approval,
            permission_mode,
            error: None,
            pending_user_messages: vec![],
            is_idle: false,
            shutdown_requested: false,
            last_reported_tool_count: 0,
            last_reported_token_count: 0,
        });
    }

    fn create_test_context() -> ToolUseContext {
        use crate::types::app_state::AppState;
        use std::sync::Arc;

        let (_tx, rx) = tokio::sync::watch::channel(false);
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
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(AppState::default),
            set_app_state: Arc::new(|_| {}),
            session_id: "test-session".to_string(),
            langfuse_session_id: "test-session".to_string(),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            ask_user_callback: None,
            bg_agent_tx: None,
            hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
            command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        }
    }
}
