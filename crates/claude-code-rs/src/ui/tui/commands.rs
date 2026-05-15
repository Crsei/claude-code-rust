use super::subsystem_events::{add_system_error, add_system_info};
use crate::ui::app::App;
use crate::ui::command_surface::CommandSurface;
use cc_commands as slash_commands;
use cc_commands::{CommandContext, CommandResult};
use cc_engine::lifecycle::QueryEngine;
use cc_types::message::{ContentBlock, Message, MessageContent};
use std::sync::Arc;
// ---------------------------------------------------------------------------
// Slash-command execution
// ---------------------------------------------------------------------------

/// Internal action returned after executing a slash command.
pub(super) enum CmdAction {
    /// Command fully handled, output already added to app.
    Handled,
    /// Command requested exit.
    Quit(String),
    /// Command produced messages to send to the model.
    Query(Vec<Message>),
}

fn conversation_changed(before: &[Message], after: &[Message]) -> bool {
    before.len() != after.len()
        || before
            .iter()
            .zip(after.iter())
            .any(|(lhs, rhs)| lhs.uuid() != rhs.uuid())
}

fn replace_app_messages(app: &mut App, messages: &[Message]) {
    app.clear_messages();
    for message in messages {
        app.add_message(message.clone());
    }
}

/// Try to execute a slash command. Returns `None` if the input is not a command.
pub(super) async fn try_execute_command(
    text: &str,
    engine: &Arc<QueryEngine>,
    app: &mut App,
) -> Option<CmdAction> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let (cmd_idx, args) = slash_commands::parse_command_input(trimmed)?;
    let all_commands = slash_commands::get_all_commands();
    let cmd = &all_commands[cmd_idx];
    let original_messages = engine.messages();

    let mut ctx = CommandContext {
        messages: original_messages.clone(),
        cwd: std::path::PathBuf::from(engine.cwd()),
        app_state: engine.app_state(),
        session_id: engine.current_session_id(),
    };

    if let Some(surface) =
        CommandSurface::for_slash_command(&cmd.name, &args, &ctx.app_state, &ctx.cwd)
    {
        app.open_command_surface(surface);
        return Some(CmdAction::Handled);
    }

    match cmd.handler.execute(&args, &mut ctx).await {
        Ok(result) => match result {
            CommandResult::Output(text) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                sync_app_runtime_from_state(engine, app, &ctx.app_state);
                add_system_info(app, &text);
                Some(CmdAction::Handled)
            }
            CommandResult::Clear => {
                let new_session_id = engine.start_new_session();
                app.clear_messages();
                app.set_session_id(new_session_id.to_string());
                add_system_info(app, "Started a new session.");
                Some(CmdAction::Handled)
            }
            CommandResult::Exit(msg) => Some(CmdAction::Quit(msg)),
            CommandResult::Query(msgs) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                sync_app_runtime_from_state(engine, app, &ctx.app_state);
                Some(CmdAction::Query(msgs))
            }
            CommandResult::None => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                sync_app_runtime_from_state(engine, app, &ctx.app_state);
                Some(CmdAction::Handled)
            }
        },
        Err(e) => {
            add_system_error(app, &format!("Command error: {e}"));
            Some(CmdAction::Handled)
        }
    }
}

fn sync_app_runtime_from_state(
    engine: &Arc<QueryEngine>,
    app: &mut App,
    state: &cc_engine::types::app_state::AppState,
) {
    engine.update_app_state(|engine_state| {
        engine_state.main_loop_model = state.main_loop_model.clone();
        engine_state.main_loop_backend = state.main_loop_backend.clone();
        engine_state.settings = state.settings.clone();
        engine_state.tool_permission_context = state.tool_permission_context.clone();
        engine_state.thinking_enabled = state.thinking_enabled;
        engine_state.fast_mode = state.fast_mode;
        engine_state.effort_value = state.effort_value.clone();
        engine_state.keybindings = state.keybindings.clone();
    });

    app.set_model_name(state.main_loop_model.clone());
    app.set_backend_name(state.main_loop_backend.clone());
    app.set_status_line_settings(state.settings.status_line.clone());
    app.set_output_style(state.settings.output_style.clone());
    app.set_editor_mode(state.settings.editor_mode.as_deref());
    app.set_keybindings(state.keybindings.clone());
    app.sync_status_context_from_state(state);

    let lang = cc_voice::language::normalize_language_for_stt(state.settings.language.as_deref());
    let voice_supported = matches!(
        cc_commands::voice_cmd::current_feasibility(),
        cc_voice::Feasibility::Ready { .. }
    );
    app.set_voice_settings(
        state.settings.voice_enabled.unwrap_or(false),
        lang.code,
        voice_supported,
    );
}

pub(super) fn query_prompt_text(messages: &[Message]) -> String {
    let mut parts = Vec::new();

    for msg in messages {
        let text = match msg {
            Message::User(user) => message_content_text(&user.content),
            Message::Assistant(assistant) => assistant
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Message::System(system) => system.content.clone(),
            Message::Progress(progress) => progress.data.to_string(),
            Message::Attachment(attachment) => format!("{:?}", attachment.attachment),
        };

        if !text.trim().is_empty() {
            parts.push(text);
        }
    }

    parts.join("\n\n")
}

fn message_content_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
