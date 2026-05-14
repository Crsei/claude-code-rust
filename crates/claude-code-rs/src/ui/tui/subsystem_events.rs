use super::engine_events::now_ts;
use crate::ui::app::App;
use crate::ui::command_surface::CommandSurface;
use cc_ipc_protocol::subsystem_events::{LspCommand, LspEvent, SubsystemEvent};
use cc_ipc_protocol::BackendMessage;
use cc_types::message::{InfoLevel, Message, SystemMessage, SystemSubtype};
pub(super) fn handle_subsystem_event(app: &mut App, event: SubsystemEvent) {
    match event {
        SubsystemEvent::Lsp(LspEvent::RecommendationRequest { payload }) => {
            app.open_command_surface(CommandSurface::lsp_recommendation(payload));
        }
        SubsystemEvent::Lsp(LspEvent::CommandError { message, .. }) => {
            add_system_error(app, &message);
        }
        _ => {}
    }
}

pub(super) fn handle_lsp_recommendation_response(
    app: &mut App,
    request_id: String,
    plugin_name: String,
    decision: String,
) {
    crate::ipc::runtime_adapters::ensure_installed();
    let messages =
        cc_ipc::subsystem_handlers::handle_lsp_command(LspCommand::RecommendationResponse {
            request_id,
            plugin_name,
            decision,
        });
    handle_backend_messages(app, messages);
}

fn handle_backend_messages(app: &mut App, messages: Vec<BackendMessage>) {
    for message in messages {
        if let BackendMessage::SystemInfo { text, level } = message {
            add_system_message(app, &text, info_level_from_str(&level));
        }
    }
}

fn info_level_from_str(level: &str) -> InfoLevel {
    match level {
        "error" => InfoLevel::Error,
        "warning" => InfoLevel::Warning,
        _ => InfoLevel::Info,
    }
}

/// Add an informational system message to the app.
pub(super) fn add_system_info(app: &mut App, text: &str) {
    add_system_message(app, text, InfoLevel::Info);
}

fn add_system_message(app: &mut App, text: &str, level: InfoLevel) {
    app.add_message(Message::System(SystemMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        subtype: SystemSubtype::Informational { level },
        content: text.to_string(),
    }));
}

/// Add an error system message to the app.
pub(super) fn add_system_error(app: &mut App, text: &str) {
    add_system_message(app, text, InfoLevel::Error);
}
