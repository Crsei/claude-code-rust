//! Frontend message dispatch (ingress).
//!
//! Receives parsed [`FrontendMessage`]s and dispatches them to the appropriate
//! handler or runtime action.  Slash command execution also lives here.

use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{debug, warn};

use crate::commands::{self, CommandContext, CommandResult};
use crate::engine::lifecycle::QueryEngine;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::message::{ContentBlock, Message, MessageContent};

use super::callbacks::{PendingPermissions, PendingQuestions};
use super::protocol::{BackendMessage, ConversationMessage, FrontendMessage};
use super::query_runner::spawn_query_turn;
use super::sink::FrontendSink;

// ---------------------------------------------------------------------------
// FrontendMessage dispatch
// ---------------------------------------------------------------------------

/// Dispatch a single [`FrontendMessage`].
///
/// Returns `true` if the event loop should continue, `false` to break.
pub(crate) async fn dispatch(
    msg: FrontendMessage,
    engine: &Arc<QueryEngine>,
    pending_permissions: &PendingPermissions,
    pending_questions: &PendingQuestions,
    suggestion_svc: &Arc<Mutex<PromptSuggestionService>>,
    sink: &FrontendSink,
) -> bool {
    match msg {
        FrontendMessage::SubmitPrompt { text, id } => {
            debug!("headless: submit_prompt id={}", id);

            if let Some(question_id) = try_answer_pending_question(pending_questions, text.clone())
            {
                debug!(
                    "headless: routed submit_prompt to pending AskUserQuestion id={}",
                    question_id
                );
                return true;
            }

            spawn_query_turn(
                engine.clone(),
                text,
                id,
                suggestion_svc.clone(),
                sink.clone(),
            );
        }

        FrontendMessage::AbortQuery => {
            debug!("headless: abort requested");
            engine.abort();
        }

        FrontendMessage::PermissionResponse {
            tool_use_id,
            decision,
        } => {
            debug!(
                "headless: permission response tool_use_id={} decision={}",
                tool_use_id, decision
            );
            if let Some(tx) = pending_permissions.lock().remove(&tool_use_id) {
                let _ = tx.send(decision);
            } else {
                warn!(
                    "headless: no pending permission for tool_use_id={}",
                    tool_use_id
                );
            }
        }

        FrontendMessage::QuestionResponse { id, text } => {
            debug!("headless: question response id={}", id);
            if let Some(tx) = pending_questions.lock().remove(&id) {
                let _ = tx.send(text);
            } else {
                warn!("headless: no pending question for id={}", id);
            }
        }

        FrontendMessage::SlashCommand { raw } => {
            debug!("headless: slash command: {}", raw);
            handle_slash_command(&raw, engine, suggestion_svc, sink).await;
        }

        FrontendMessage::Resize { cols, rows } => {
            debug!("headless: resize {}x{}", cols, rows);
            let mut ps = crate::bootstrap::PROCESS_STATE.write();
            ps.terminal_cols = cols;
            ps.terminal_rows = rows;
        }

        FrontendMessage::Quit => {
            debug!("headless: quit requested");
            return false;
        }

        FrontendMessage::LspCommand { command } => {
            debug!("headless: LSP command: {:?}", command);
            let msgs = super::subsystem_handlers::handle_lsp_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::McpCommand { command } => {
            debug!("headless: MCP command: {:?}", command);
            let msgs = super::subsystem_handlers::handle_mcp_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::PluginCommand { command } => {
            debug!("headless: Plugin command: {:?}", command);
            let msgs = super::subsystem_handlers::handle_plugin_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::SkillCommand { command } => {
            debug!("headless: Skill command: {:?}", command);
            let msgs = super::subsystem_handlers::handle_skill_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::IdeCommand { command } => {
            debug!("headless: IDE command: {:?}", command);
            let msgs = super::subsystem_handlers::handle_ide_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::AgentSettingsCommand { command } => {
            debug!("headless: AgentSettings command: {:?}", command);
            let msgs = super::agent_settings::handle(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::QuerySubsystemStatus => {
            debug!("headless: subsystem status query");
            let status = super::subsystem_handlers::build_subsystem_status_snapshot();
            let _ = sink.send(&BackendMessage::SubsystemStatus { status });
        }
        FrontendMessage::AgentCommand { command } => {
            debug!("headless: Agent command: {:?}", command);
            let msgs = super::agent_handlers::handle_agent_command(command);
            let _ = sink.send_many(msgs);
        }
        FrontendMessage::TeamCommand { command } => {
            debug!("headless: Team command: {:?}", command);
            let msgs = super::agent_handlers::handle_team_command(command);
            let _ = sink.send_many(msgs);
        }

        FrontendMessage::SearchFiles {
            request_id,
            pattern,
            cwd,
            case_insensitive,
            max_results,
        } => {
            debug!(
                "headless: search_files request_id={} pattern={:?}",
                request_id, pattern
            );
            super::file_search::dispatch_search(
                request_id,
                pattern,
                cwd,
                case_insensitive,
                max_results,
                sink,
            );
        }
    }

    true // continue loop
}

// ---------------------------------------------------------------------------
// Pending question helper
// ---------------------------------------------------------------------------

fn try_answer_pending_question(
    pending_questions: &PendingQuestions,
    text: String,
) -> Option<String> {
    let mut pending = pending_questions.lock();
    let pending_id = pending.keys().next().cloned()?;
    let tx = pending.remove(&pending_id)?;
    drop(pending);
    let _ = tx.send(text);
    Some(pending_id)
}

// ---------------------------------------------------------------------------
// Slash command execution
// ---------------------------------------------------------------------------

fn conversation_changed(before: &[Message], after: &[Message]) -> bool {
    before.len() != after.len()
        || before
            .iter()
            .zip(after.iter())
            .any(|(lhs, rhs)| lhs.uuid() != rhs.uuid())
}

fn flatten_blocks(blocks: &[ContentBlock]) -> (String, Option<String>) {
    let mut text_parts = Vec::new();
    let mut thinking_parts = Vec::new();

    for block in blocks {
        match block {
            ContentBlock::Text { text } => text_parts.push(text.clone()),
            ContentBlock::Thinking { thinking, .. } => thinking_parts.push(thinking.clone()),
            _ => {}
        }
    }

    let text = text_parts.join("\n");
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n"))
    };

    (text, thinking)
}

fn to_conversation_message(message: &Message) -> Option<ConversationMessage> {
    match message {
        Message::User(user) => {
            let content = match &user.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::Blocks(blocks) => flatten_blocks(blocks).0,
            };

            Some(ConversationMessage {
                id: user.uuid.to_string(),
                role: "user".to_string(),
                content,
                timestamp: user.timestamp,
                content_blocks: match &user.content {
                    MessageContent::Blocks(blocks) => Some(blocks.clone()),
                    MessageContent::Text(_) => None,
                },
                cost_usd: None,
                thinking: None,
                level: None,
            })
        }
        Message::Assistant(assistant) => {
            let (content, thinking) = flatten_blocks(&assistant.content);
            Some(ConversationMessage {
                id: assistant.uuid.to_string(),
                role: "assistant".to_string(),
                content,
                timestamp: assistant.timestamp,
                content_blocks: Some(assistant.content.clone()),
                cost_usd: Some(assistant.cost_usd),
                thinking,
                level: None,
            })
        }
        Message::System(system) => {
            let level = match &system.subtype {
                crate::types::message::SystemSubtype::Informational { level } => Some(
                    match level {
                        crate::types::message::InfoLevel::Info => "info",
                        crate::types::message::InfoLevel::Warning => "warning",
                        crate::types::message::InfoLevel::Error => "error",
                    }
                    .to_string(),
                ),
                crate::types::message::SystemSubtype::Warning => Some("warning".to_string()),
                crate::types::message::SystemSubtype::ApiError { .. } => Some("error".to_string()),
                _ => Some("info".to_string()),
            };

            Some(ConversationMessage {
                id: system.uuid.to_string(),
                role: "system".to_string(),
                content: system.content.clone(),
                timestamp: system.timestamp,
                content_blocks: None,
                cost_usd: None,
                thinking: None,
                level,
            })
        }
        _ => None,
    }
}

fn send_conversation_replaced(messages: &[Message], sink: &FrontendSink) {
    let visible_messages: Vec<_> = messages
        .iter()
        .filter_map(to_conversation_message)
        .collect();
    let _ = sink.send(&BackendMessage::ConversationReplaced {
        messages: visible_messages,
    });
}

/// Parse and execute a slash command, sending results as BackendMessages.
async fn handle_slash_command(
    raw: &str,
    engine: &Arc<QueryEngine>,
    suggestion_svc: &Arc<Mutex<PromptSuggestionService>>,
    sink: &FrontendSink,
) {
    let trimmed = raw.trim();
    if !trimmed.starts_with('/') {
        let _ = sink.send(&BackendMessage::Error {
            message: format!("not a slash command: {}", trimmed),
            recoverable: true,
        });
        return;
    }

    let Some((cmd_idx, args)) = commands::parse_command_input(trimmed) else {
        let _ = sink.send(&BackendMessage::Error {
            message: format!("unknown command: {}", trimmed),
            recoverable: true,
        });
        return;
    };

    let all_commands = commands::get_all_commands();
    let cmd = &all_commands[cmd_idx];
    let original_messages = engine.messages();

    let mut ctx = CommandContext {
        messages: original_messages.clone(),
        cwd: std::path::PathBuf::from(engine.cwd()),
        app_state: engine.app_state(),
        session_id: engine.session_id.clone(),
    };

    let cmd_result = cmd.handler.execute(&args, &mut ctx).await;

    // Sync any state mutations (e.g. /add-dir, /team) back to the engine.
    // Commands operate on a cloned snapshot; without this sync their edits
    // would be lost to the next tool invocation.
    if cmd_result.is_ok() {
        let new_adl = ctx
            .app_state
            .tool_permission_context
            .additional_working_directories
            .clone();
        let new_team_ctx = ctx.app_state.team_context.clone();
        engine.update_app_state(|s| {
            s.tool_permission_context.additional_working_directories = new_adl;
            s.team_context = new_team_ctx;
        });

        // If this was a /team command (or any command that mutated team_context),
        // push a fresh StatusSnapshot so the Team Dashboard reflects the change
        // without the frontend having to poll.
        if cmd.name == "team" {
            if let Some(tc) = ctx.app_state.team_context.as_ref() {
                if !tc.team_name.is_empty() {
                    let events =
                        super::agent_handlers::build_team_status_events(&tc.team_name);
                    let _ = sink.send_many(events);
                }
            }
        }
    }

    match cmd_result {
        Ok(result) => match result {
            CommandResult::Output(text) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    send_conversation_replaced(&ctx.messages, sink);
                }
                let _ = sink.send(&BackendMessage::SystemInfo {
                    text,
                    level: "info".to_string(),
                });
            }
            CommandResult::Clear => {
                engine.clear_messages();
                send_conversation_replaced(&[], sink);
                let _ = sink.send(&BackendMessage::SystemInfo {
                    text: "Conversation cleared.".to_string(),
                    level: "info".to_string(),
                });
            }
            CommandResult::Exit(msg) => {
                let _ = sink.send(&BackendMessage::SystemInfo {
                    text: msg,
                    level: "info".to_string(),
                });
            }
            CommandResult::Query(msgs) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    send_conversation_replaced(&ctx.messages, sink);
                }
                let prompt_text: String = msgs
                    .iter()
                    .filter_map(|m| match m {
                        Message::User(u) => Some(match &u.content {
                            crate::types::message::MessageContent::Text(t) => t.clone(),
                            crate::types::message::MessageContent::Blocks(_) => {
                                "[content blocks]".to_string()
                            }
                        }),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if !prompt_text.is_empty() {
                    spawn_query_turn(
                        engine.clone(),
                        prompt_text,
                        uuid::Uuid::new_v4().to_string(),
                        suggestion_svc.clone(),
                        sink.clone(),
                    );
                }
            }
            CommandResult::None => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    send_conversation_replaced(&ctx.messages, sink);
                }
            }
        },
        Err(e) => {
            let _ = sink.send(&BackendMessage::Error {
                message: format!("command error: {}", e),
                recoverable: true,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::oneshot;

    #[test]
    fn routes_submit_prompt_to_pending_question() {
        let pending: PendingQuestions = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().insert("question-1".to_string(), tx);

        let routed = try_answer_pending_question(&pending, "my answer".to_string());

        assert_eq!(routed.as_deref(), Some("question-1"));
        assert!(
            pending.lock().is_empty(),
            "pending question should be removed"
        );
        assert_eq!(
            rx.blocking_recv().expect("answer should be delivered"),
            "my answer"
        );
    }

    #[test]
    fn returns_none_when_no_pending_question_exists() {
        let pending: PendingQuestions = Arc::new(Mutex::new(HashMap::new()));
        let routed = try_answer_pending_question(&pending, "ignored".to_string());
        assert!(routed.is_none());
    }

    #[test]
    fn question_response_completes_pending_question_by_id() {
        let pending: PendingQuestions = Arc::new(Mutex::new(HashMap::new()));
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        pending.lock().insert("q-1".to_string(), tx1);
        pending.lock().insert("q-2".to_string(), tx2);

        // Answer q-2 specifically
        if let Some(tx) = pending.lock().remove("q-2") {
            let _ = tx.send("answer-for-q2".to_string());
        }

        assert_eq!(rx2.blocking_recv().unwrap(), "answer-for-q2");
        // q-1 should still be pending
        assert!(pending.lock().contains_key("q-1"));

        // Answer q-1
        if let Some(tx) = pending.lock().remove("q-1") {
            let _ = tx.send("answer-for-q1".to_string());
        }
        assert_eq!(rx1.blocking_recv().unwrap(), "answer-for-q1");
        assert!(pending.lock().is_empty());
    }

    #[test]
    fn submit_prompt_fallback_still_works_for_backward_compat() {
        // When a submit_prompt arrives and there's a pending question,
        // the fallback routes it to the first pending question (old behavior).
        let pending: PendingQuestions = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().insert("legacy-q".to_string(), tx);

        let routed = try_answer_pending_question(&pending, "old-style answer".to_string());
        assert!(routed.is_some());
        assert_eq!(rx.blocking_recv().unwrap(), "old-style answer");
    }
}
