//! Adapter helpers between IPC backend messages and app events.

use crate::ipc::protocol::BackendMessage;

use super::app_event::AppEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySnapshot {
    pub session_id: String,
    pub model: String,
    pub cwd: String,
}

pub fn app_event_from_backend(message: BackendMessage) -> AppEvent {
    AppEvent::backend(message)
}

pub fn ready_snapshot(message: &BackendMessage) -> Option<ReadySnapshot> {
    match message {
        BackendMessage::Ready {
            session_id,
            model,
            cwd,
            ..
        } => Some(ReadySnapshot {
            session_id: session_id.clone(),
            model: model.clone(),
            cwd: cwd.clone(),
        }),
        _ => None,
    }
}

pub fn backend_message_kind(message: &BackendMessage) -> &'static str {
    match message {
        BackendMessage::Ready { .. } => "ready",
        BackendMessage::StreamStart { .. } => "stream_start",
        BackendMessage::StreamDelta { .. } => "stream_delta",
        BackendMessage::ThinkingDelta { .. } => "thinking_delta",
        BackendMessage::StreamEnd { .. } => "stream_end",
        BackendMessage::Tombstone { .. } => "tombstone",
        BackendMessage::AssistantMessage { .. } => "assistant_message",
        BackendMessage::ToolUse { .. } => "tool_use",
        BackendMessage::ToolResult { .. } => "tool_result",
        BackendMessage::ToolProgress { .. } => "tool_progress",
        BackendMessage::PermissionRequest { .. } => "permission_request",
        BackendMessage::QuestionRequest { .. } => "question_request",
        BackendMessage::PlanWorkflowEvent { .. } => "plan_workflow_event",
        BackendMessage::SystemInfo { .. } => "system_info",
        BackendMessage::ConversationReplaced { .. } => "conversation_replaced",
        BackendMessage::UsageUpdate { .. } => "usage_update",
        BackendMessage::StatusLineUpdate { .. } => "status_line_update",
        BackendMessage::Suggestions { .. } => "suggestions",
        BackendMessage::Error { .. } => "error",
        BackendMessage::BackgroundAgentComplete { .. } => "background_agent_complete",
        BackendMessage::BriefMessage { .. } => "brief_message",
        BackendMessage::AutonomousStart { .. } => "autonomous_start",
        BackendMessage::NotificationSent { .. } => "notification_sent",
        BackendMessage::LspEvent { .. } => "lsp_event",
        BackendMessage::McpEvent { .. } => "mcp_event",
        BackendMessage::PluginEvent { .. } => "plugin_event",
        BackendMessage::SkillEvent { .. } => "skill_event",
        BackendMessage::IdeEvent { .. } => "ide_event",
        BackendMessage::AgentSettingsEvent { .. } => "agent_settings_event",
        BackendMessage::SubsystemStatus { .. } => "subsystem_status",
        BackendMessage::AgentEvent { .. } => "agent_event",
        BackendMessage::TeamEvent { .. } => "team_event",
        BackendMessage::FileSearchResult { .. } => "file_search_result",
    }
}
