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
        crate::teams::is_agent_teams_enabled()
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
        .filter(|m| m.name != sender)
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

        protocol::ProtocolMessage::PlanApprovalResponse { .. } => {
            // Forward plan approval to teammate
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
                data: json!({"sent": true, "type": "plan_approval_response", "to": to}),
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
            get_app_state: Arc::new(|| AppState::default()),
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
        }
    }
}
