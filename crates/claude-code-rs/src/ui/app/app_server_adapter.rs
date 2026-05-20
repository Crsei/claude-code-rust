// test infrastructure — not wired to production yet; tracked in IMPLEMENTATION_GAPS.md
//! Adapter helpers between IPC backend messages and app events.

use cc_ipc_protocol::BackendMessage;

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
        BackendMessage::Completions { .. } => "completions",
        BackendMessage::PluginInstallProgress { .. } => "plugin_install_progress",
        BackendMessage::TelemetryStatus { .. } => "telemetry_status",
        BackendMessage::LspRecommendations { .. } => "lsp_recommendations",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> BackendMessage {
        BackendMessage::Ready {
            session_id: "session".to_string(),
            model: "model".to_string(),
            cwd: "/tmp/project".to_string(),
            permission_mode: "default".to_string(),
            available_models: Vec::new(),
            plan_workflow: None,
            editor_mode: None,
            view_mode: None,
            keybindings: None,
        }
    }

    #[test]
    fn extracts_ready_snapshot() {
        let snapshot = ready_snapshot(&ready()).expect("ready snapshot");
        assert_eq!(snapshot.session_id, "session");
        assert_eq!(snapshot.model, "model");
        assert_eq!(snapshot.cwd, "/tmp/project");
    }

    #[test]
    fn names_backend_message_kind_and_wraps_event() {
        let message = ready();
        assert_eq!(backend_message_kind(&message), "ready");
        assert!(matches!(
            app_event_from_backend(message),
            AppEvent::Backend { .. }
        ));
    }
}
