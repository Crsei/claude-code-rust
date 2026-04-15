//! IPC protocol types for headless mode.
//!
//! Defines the JSON-lines protocol between the Rust backend and a separate
//! UI process (e.g. an Ink/React terminal frontend).
//!
//! Both enums use `#[serde(tag = "type")]` so each JSON line carries a
//! discriminator field, e.g. `{"type":"submit_prompt","text":"hello","id":"…"}`.

use serde::{Deserialize, Serialize};

use crate::types::message::ContentBlock;

/// Lightweight description of a content block in a tool result,
/// suitable for forwarding to the frontend without embedding raw image data.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentInfo {
    /// Plain text content.
    Text { text: String },
    /// An image was returned (data is NOT forwarded — only metadata).
    Image {
        /// MIME type (e.g. "image/png").
        media_type: String,
        /// Approximate byte size of the base64-decoded image data.
        #[serde(skip_serializing_if = "Option::is_none")]
        size_bytes: Option<usize>,
    },
}

#[derive(Serialize, Debug, Clone)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<ContentBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

// ---------------------------------------------------------------------------
// Frontend → Backend (deserialized from stdin)
// ---------------------------------------------------------------------------

/// Messages sent by the UI process to the Rust backend.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrontendMessage {
    /// User submits a prompt.
    SubmitPrompt { text: String, id: String },
    /// User hits Ctrl+C to abort the current query.
    AbortQuery,
    /// User responds to a permission dialog.
    PermissionResponse {
        tool_use_id: String,
        /// One of "allow", "deny", "always_allow".
        decision: String,
    },
    /// User typed a slash command.
    SlashCommand { raw: String },
    /// Terminal was resized.
    Resize { cols: u16, rows: u16 },
    /// User wants to exit.
    Quit,

    /// LSP lifecycle command.
    LspCommand {
        command: super::subsystem_events::LspCommand,
    },

    /// MCP lifecycle command.
    McpCommand {
        command: super::subsystem_events::McpCommand,
    },

    /// Plugin lifecycle command.
    PluginCommand {
        command: super::subsystem_events::PluginCommand,
    },

    /// Skill management command.
    SkillCommand {
        command: super::subsystem_events::SkillCommand,
    },

    /// Query all subsystem statuses.
    QuerySubsystemStatus,

    /// Agent management commands.
    AgentCommand {
        command: super::agent_events::AgentCommand,
    },

    /// Team management commands.
    TeamCommand {
        command: super::agent_events::TeamCommand,
    },
}

// ---------------------------------------------------------------------------
// Backend → Frontend (serialized to stdout)
// ---------------------------------------------------------------------------

/// Messages sent by the Rust backend to the UI process.
#[allow(dead_code)] // Protocol variants used by future phases (permission system, tool results, suggestions)
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendMessage {
    /// Backend has initialized and is ready to accept prompts.
    Ready {
        session_id: String,
        model: String,
        cwd: String,
    },
    /// Assistant started streaming a new content block.
    StreamStart { message_id: String },
    /// Streaming text delta for an in-progress content block.
    StreamDelta { message_id: String, text: String },
    /// Streaming thinking delta for an in-progress thinking block.
    ThinkingDelta {
        message_id: String,
        thinking: String,
    },
    /// Streaming for a content block has finished.
    StreamEnd { message_id: String },
    /// Final assistant message (content is the serialized Vec<ContentBlock>).
    AssistantMessage {
        id: String,
        content: serde_json::Value,
        cost_usd: f64,
    },
    /// A tool invocation.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Result of a tool invocation.
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
        /// Structured content blocks when the result includes non-text data
        /// (e.g. images from Computer Use screenshot). The frontend can use
        /// this to render image placeholders or thumbnails.
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ToolResultContentInfo>>,
    },
    /// Ask the UI to show a permission dialog for a tool call.
    PermissionRequest {
        tool_use_id: String,
        tool: String,
        command: String,
        options: Vec<String>,
    },
    /// A system-level informational message.
    SystemInfo {
        text: String,
        /// One of "info", "warning", "error".
        level: String,
    },
    /// Replace the full visible conversation history in the frontend.
    ConversationReplaced { messages: Vec<ConversationMessage> },
    /// Token usage update.
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    },
    /// Prompt suggestions for the UI to display.
    Suggestions { items: Vec<String> },
    /// An error occurred.
    Error { message: String, recoverable: bool },
    /// A background agent has completed execution.
    BackgroundAgentComplete {
        agent_id: String,
        description: String,
        /// Truncated preview of the result (for UI display).
        result_preview: String,
        had_error: bool,
        duration_ms: u64,
    },

    /// Brief mode message from the model (via BriefTool).
    BriefMessage {
        message: String,
        status: String,
        attachments: Vec<String>,
    },

    /// Autonomous action started (proactive tick).
    AutonomousStart { source: String, time: String },

    /// Push notification sent.
    NotificationSent { title: String, level: String },

    /// LSP subsystem event.
    LspEvent {
        event: super::subsystem_events::LspEvent,
    },

    /// MCP subsystem event.
    McpEvent {
        event: super::subsystem_events::McpEvent,
    },

    /// Plugin subsystem event.
    PluginEvent {
        event: super::subsystem_events::PluginEvent,
    },

    /// Skill subsystem event.
    SkillEvent {
        event: super::subsystem_events::SkillEvent,
    },

    /// Aggregated subsystem status snapshot.
    SubsystemStatus {
        status: super::subsystem_types::SubsystemStatusSnapshot,
    },

    /// Agent lifecycle + streaming events.
    AgentEvent {
        event: super::agent_events::AgentEvent,
    },

    /// Team events.
    TeamEvent {
        event: super::agent_events::TeamEvent,
    },
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Write a [`BackendMessage`] as a JSON line to stdout.
pub fn send_to_frontend(msg: &BackendMessage) -> std::io::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", json)?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::ConversationMessage;
    use crate::types::message::ContentBlock;

    #[test]
    fn conversation_message_serializes_content_blocks_when_present() {
        let message = ConversationMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            content: "summary".to_string(),
            timestamp: 1,
            content_blocks: Some(vec![
                ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "Read".to_string(),
                    input: serde_json::json!({ "file_path": "/tmp/a.ts" }),
                },
                ContentBlock::Text {
                    text: "summary".to_string(),
                },
            ]),
            cost_usd: Some(0.01),
            thinking: None,
            level: None,
        };

        let value = serde_json::to_value(&message).expect("serialize conversation message");
        let blocks = value
            .get("content_blocks")
            .and_then(|v| v.as_array())
            .expect("content_blocks array");

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "tool_use");
        assert_eq!(blocks[1]["type"], "text");
    }

    #[test]
    fn backend_lsp_event_serializes() {
        use super::BackendMessage;
        use crate::ipc::subsystem_events::LspEvent;
        let msg = BackendMessage::LspEvent {
            event: LspEvent::ServerStateChanged {
                language_id: "rust".to_string(),
                state: "running".to_string(),
                error: None,
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "lsp_event");
        assert_eq!(json["event"]["kind"], "server_state_changed");
    }

    #[test]
    fn backend_subsystem_status_serializes() {
        use super::BackendMessage;
        use crate::ipc::subsystem_types::SubsystemStatusSnapshot;
        let msg = BackendMessage::SubsystemStatus {
            status: SubsystemStatusSnapshot {
                lsp: vec![],
                mcp: vec![],
                plugins: vec![],
                skills: vec![],
                timestamp: 100,
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "subsystem_status");
        assert_eq!(json["status"]["timestamp"], 100);
    }

    #[test]
    fn frontend_lsp_command_deserializes() {
        use super::FrontendMessage;
        let json = r#"{"type":"lsp_command","command":{"kind":"start_server","language_id":"rust"}}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::LspCommand { .. }));
    }

    #[test]
    fn frontend_query_subsystem_status_deserializes() {
        use super::FrontendMessage;
        let json = r#"{"type":"query_subsystem_status"}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::QuerySubsystemStatus));
    }

    #[test]
    fn backend_agent_event_serializes() {
        use super::BackendMessage;
        use crate::ipc::agent_events::AgentEvent as AE;
        let msg = BackendMessage::AgentEvent {
            event: AE::Spawned {
                agent_id: "a1".into(),
                parent_agent_id: None,
                description: "test".into(),
                agent_type: None,
                model: None,
                is_background: false,
                depth: 1,
                chain_id: "c1".into(),
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "agent_event");
        assert_eq!(json["event"]["kind"], "spawned");
    }

    #[test]
    fn frontend_agent_command_deserializes() {
        use super::FrontendMessage;
        let json = r#"{"type":"agent_command","command":{"kind":"abort_agent","agent_id":"a1"}}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::AgentCommand { .. }));
    }

    #[test]
    fn backend_team_event_serializes() {
        use super::BackendMessage;
        use crate::ipc::agent_events::TeamEvent as TE;
        let msg = BackendMessage::TeamEvent {
            event: TE::MemberLeft {
                team_name: "t1".into(),
                agent_id: "a1".into(),
                agent_name: "w".into(),
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "team_event");
        assert_eq!(json["event"]["kind"], "member_left");
    }

    #[test]
    fn frontend_team_command_deserializes() {
        use super::FrontendMessage;
        let json = r#"{"type":"team_command","command":{"kind":"query_team_status","team_name":"t1"}}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::TeamCommand { .. }));
    }

    #[test]
    fn conversation_message_omits_content_blocks_when_absent() {
        let message = ConversationMessage {
            id: "system-1".to_string(),
            role: "system".to_string(),
            content: "info".to_string(),
            timestamp: 1,
            content_blocks: None,
            cost_usd: None,
            thinking: None,
            level: Some("info".to_string()),
        };

        let value = serde_json::to_value(&message).expect("serialize conversation message");
        assert!(value.get("content_blocks").is_none());
    }
}
