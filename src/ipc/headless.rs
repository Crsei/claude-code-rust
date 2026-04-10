//! Headless event loop — replaces `tui::run_tui()` when `--headless` is passed.
//!
//! Communicates with an external UI process via JSON lines on stdin/stdout
//! using the protocol defined in [`super::protocol`].

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use parking_lot::Mutex;
use tokio::io::AsyncBufReadExt;
use tokio::sync::oneshot;
use tracing::{debug, error, warn};

use crate::commands::{self, CommandContext, CommandResult};
use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::config::QuerySource;
use crate::types::message::{ContentBlock, Message, StreamEvent, ToolResultContent};

use super::protocol::{send_to_frontend, BackendMessage, FrontendMessage};

/// Pending permission requests awaiting a response from the frontend.
type PendingPermissions = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

/// Run the headless event loop.
///
/// Reads [`FrontendMessage`]s from stdin (one JSON object per line) and writes
/// [`BackendMessage`]s to stdout. This function only returns when the UI sends
/// [`FrontendMessage::Quit`] or stdin is closed.
pub async fn run_headless(engine: Arc<QueryEngine>, model: String) -> anyhow::Result<()> {
    // ── 1. Permission bridge setup ─────────────────────────────────
    let pending_permissions: PendingPermissions = Arc::new(Mutex::new(HashMap::new()));

    // Set up the permission callback on the engine
    {
        let pending = pending_permissions.clone();
        let callback: crate::types::tool::PermissionCallback = Arc::new(
            move |tool_use_id: String,
                  tool_name: String,
                  description: String,
                  options: Vec<String>| {
                let pending = pending.clone();
                Box::pin(async move {
                    // Send PermissionRequest to frontend
                    let _ = send_to_frontend(&BackendMessage::PermissionRequest {
                        tool_use_id: tool_use_id.clone(),
                        tool: tool_name,
                        command: description,
                        options,
                    });

                    // Create a oneshot channel and wait for the response
                    let (tx, rx) = oneshot::channel();
                    pending.lock().insert(tool_use_id, tx);

                    // Await the frontend's decision
                    match rx.await {
                        Ok(decision) => decision,
                        Err(_) => "deny".to_string(), // channel dropped = deny
                    }
                })
            },
        );
        engine.set_permission_callback(callback);
    }

    // ── 1b. Background agent channel setup ────────────────────────
    let (bg_tx, mut bg_rx) = tokio::sync::mpsc::unbounded_channel();
    engine.set_bg_agent_tx(bg_tx);
    let pending_bg = engine.pending_bg_results.clone();

    // ── 2. Prompt suggestion service ───────────────────────────────
    let suggestion_svc = Arc::new(Mutex::new(PromptSuggestionService::new(true)));

    // ── 3. Send Ready ──────────────────────────────────────────────
    let ready = BackendMessage::Ready {
        session_id: engine.session_id.to_string(),
        model,
        cwd: engine.cwd().to_string(),
    };
    send_to_frontend(&ready)?;

    // ── 4. Read stdin lines ────────────────────────────────────────
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    loop {
        tokio::select! {
            // ── Branch 1: Frontend message (stdin) ──────────────
            line = lines.next_line() => {
                let line = match line {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        debug!("headless: stdin closed, exiting");
                        break;
                    }
                    Err(e) => {
                        error!("headless: error reading stdin: {}", e);
                        break;
                    }
                };

                let msg: FrontendMessage = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(
                            "headless: failed to parse FrontendMessage: {} — line: {}",
                            e, line
                        );
                        let _ = send_to_frontend(&BackendMessage::Error {
                            message: format!("invalid FrontendMessage: {}", e),
                            recoverable: true,
                        });
                        continue;
                    }
                };

                match msg {
                    FrontendMessage::SubmitPrompt { text, id } => {
                        debug!("headless: submit_prompt id={}", id);
                        engine.reset_abort();

                        let engine_clone = engine.clone();
                        let message_id = id;
                        let svc = suggestion_svc.clone();

                        tokio::spawn(async move {
                            let stream =
                                engine_clone.submit_message(&text, QuerySource::ReplMainThread);
                            let mut stream = std::pin::pin!(stream);

                            while let Some(sdk_msg) = stream.next().await {
                                let send_result =
                                    handle_sdk_message(&sdk_msg, &message_id, &engine_clone, &svc);
                                if let Err(e) = send_result {
                                    error!("headless: failed to send to frontend: {}", e);
                                    break;
                                }
                            }
                        });
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

                    FrontendMessage::SlashCommand { raw } => {
                        debug!("headless: slash command: {}", raw);
                        handle_slash_command(&raw, &engine).await;
                    }

                    FrontendMessage::Resize { cols, rows } => {
                        debug!("headless: resize {}x{}", cols, rows);
                        let mut ps = crate::bootstrap::PROCESS_STATE.write();
                        ps.terminal_cols = cols;
                        ps.terminal_rows = rows;
                    }

                    FrontendMessage::Quit => {
                        debug!("headless: quit requested");
                        break;
                    }
                }
            }

            // ── Branch 2: Background agent completed ────────────
            Some(completed) = bg_rx.recv() => {
                debug!(
                    agent_id = %completed.agent_id,
                    description = %completed.description,
                    had_error = completed.had_error,
                    "headless: background agent completed"
                );

                // Truncate result for UI preview (char-boundary safe)
                let result_preview = if completed.result_text.len() > 200 {
                    let end = completed.result_text.floor_char_boundary(200);
                    format!("{}...", &completed.result_text[..end])
                } else {
                    completed.result_text.clone()
                };

                // Notify frontend immediately
                let _ = send_to_frontend(&BackendMessage::BackgroundAgentComplete {
                    agent_id: completed.agent_id.clone(),
                    description: completed.description.clone(),
                    result_preview,
                    had_error: completed.had_error,
                    duration_ms: completed.duration.as_millis() as u64,
                });

                // Push to shared buffer for query loop injection
                pending_bg.push(completed);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Slash command execution
// ---------------------------------------------------------------------------

/// Parse and execute a slash command, sending results as BackendMessages.
async fn handle_slash_command(raw: &str, engine: &Arc<QueryEngine>) {
    let trimmed = raw.trim();
    if !trimmed.starts_with('/') {
        let _ = send_to_frontend(&BackendMessage::Error {
            message: format!("not a slash command: {}", trimmed),
            recoverable: true,
        });
        return;
    }

    let Some((cmd_idx, args)) = commands::parse_command_input(trimmed) else {
        let _ = send_to_frontend(&BackendMessage::Error {
            message: format!("unknown command: {}", trimmed),
            recoverable: true,
        });
        return;
    };

    let all_commands = commands::get_all_commands();
    let cmd = &all_commands[cmd_idx];

    let mut ctx = CommandContext {
        messages: engine.messages(),
        cwd: std::path::PathBuf::from(engine.cwd()),
        app_state: engine.app_state(),
        session_id: engine.session_id.clone(),
    };

    match cmd.handler.execute(&args, &mut ctx).await {
        Ok(result) => match result {
            CommandResult::Output(text) => {
                let _ = send_to_frontend(&BackendMessage::SystemInfo {
                    text,
                    level: "info".to_string(),
                });
            }
            CommandResult::Clear => {
                // TODO: engine currently has no clear_messages() method;
                // for now, notify the frontend that the conversation was cleared.
                let _ = send_to_frontend(&BackendMessage::SystemInfo {
                    text: "Conversation cleared.".to_string(),
                    level: "info".to_string(),
                });
            }
            CommandResult::Exit(msg) => {
                let _ = send_to_frontend(&BackendMessage::SystemInfo {
                    text: msg,
                    level: "info".to_string(),
                });
                // The frontend should observe this and send Quit.
            }
            CommandResult::Query(msgs) => {
                // The command produced messages that should be sent to the model.
                // Spawn a query, similar to SubmitPrompt.
                let engine_clone = engine.clone();
                let message_id = uuid::Uuid::new_v4().to_string();

                // Build a text representation for the query
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
                    let svc = Arc::new(Mutex::new(PromptSuggestionService::new(true)));
                    tokio::spawn(async move {
                        engine_clone.reset_abort();
                        let stream = engine_clone
                            .submit_message(&prompt_text, QuerySource::ReplMainThread);
                        let mut stream = std::pin::pin!(stream);

                        while let Some(sdk_msg) = stream.next().await {
                            if let Err(e) = handle_sdk_message(
                                &sdk_msg,
                                &message_id,
                                &engine_clone,
                                &svc,
                            ) {
                                error!("headless: command query send error: {}", e);
                                break;
                            }
                        }
                    });
                }
            }
            CommandResult::None => {}
        },
        Err(e) => {
            let _ = send_to_frontend(&BackendMessage::Error {
                message: format!("command error: {}", e),
                recoverable: true,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// SdkMessage → BackendMessage mapping
// ---------------------------------------------------------------------------

/// Map a single [`SdkMessage`] to the appropriate [`BackendMessage`](s) and
/// send them to the frontend. This is the central dispatch for the headless
/// protocol — every `SdkMessage` variant is handled here.
fn handle_sdk_message(
    sdk_msg: &SdkMessage,
    message_id: &str,
    engine: &Arc<QueryEngine>,
    suggestion_svc: &Arc<Mutex<PromptSuggestionService>>,
) -> std::io::Result<()> {
    match sdk_msg {
        // ── SystemInit ──────────────────────────────────────────
        SdkMessage::SystemInit(init) => {
            send_to_frontend(&BackendMessage::SystemInfo {
                text: format!(
                    "Session {} initialized — model: {}, permission: {}, {} tools",
                    init.session_id,
                    init.model,
                    init.permission_mode,
                    init.tools.len()
                ),
                level: "info".to_string(),
            })
        }

        // ── StreamEvent ─────────────────────────────────────────
        SdkMessage::StreamEvent(evt) => handle_stream_event(&evt.event, message_id),

        // ── Assistant message ───────────────────────────────────
        SdkMessage::Assistant(a) => {
            // First send individual ToolUse messages for each tool call
            // so the frontend can render them immediately.
            for block in &a.message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    let _ = send_to_frontend(&BackendMessage::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
            }

            // Then send the full assistant message
            let content =
                serde_json::to_value(&a.message.content).unwrap_or(serde_json::Value::Null);
            send_to_frontend(&BackendMessage::AssistantMessage {
                id: a.message.uuid.to_string(),
                content,
                cost_usd: a.message.cost_usd,
            })
        }

        // ── UserReplay (includes tool results) ──────────────────
        SdkMessage::UserReplay(replay) => {
            if replay.is_synthetic {
                debug!("headless: user replay (synthetic): {}", replay.content);
            }

            // Extract and forward tool results from content blocks
            if let Some(ref blocks) = replay.content_blocks {
                for block in blocks {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = block
                    {
                        let output = match content {
                            ToolResultContent::Text(t) => t.clone(),
                            ToolResultContent::Blocks(inner) => {
                                // Collect text from nested blocks
                                inner
                                    .iter()
                                    .filter_map(|b| {
                                        if let ContentBlock::Text { text } = b {
                                            Some(text.as_str())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            }
                        };
                        let _ = send_to_frontend(&BackendMessage::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            output,
                            is_error: *is_error,
                        });
                    }
                }
            }

            Ok(())
        }

        // ── CompactBoundary ─────────────────────────────────────
        SdkMessage::CompactBoundary(boundary) => {
            let text = if let Some(ref meta) = boundary.compact_metadata {
                format!(
                    "Context compacted: {} → {} tokens",
                    meta.pre_compact_token_count, meta.post_compact_token_count
                )
            } else {
                "Context compacted".to_string()
            };
            send_to_frontend(&BackendMessage::SystemInfo {
                text,
                level: "info".to_string(),
            })
        }

        // ── ApiRetry ────────────────────────────────────────────
        SdkMessage::ApiRetry(retry) => send_to_frontend(&BackendMessage::Error {
            message: format!(
                "API retry {}/{}: {} (waiting {}ms)",
                retry.attempt, retry.max_retries, retry.error, retry.retry_delay_ms
            ),
            recoverable: true,
        }),

        // ── ToolUseSummary ──────────────────────────────────────
        SdkMessage::ToolUseSummary(summary) => {
            send_to_frontend(&BackendMessage::SystemInfo {
                text: summary.summary.clone(),
                level: "info".to_string(),
            })
        }

        // ── Result ──────────────────────────────────────────────
        SdkMessage::Result(r) => {
            // Always send StreamEnd to clear UI streaming state
            let _ = send_to_frontend(&BackendMessage::StreamEnd {
                message_id: message_id.to_string(),
            });

            let usage_msg = BackendMessage::UsageUpdate {
                input_tokens: r.usage.total_input_tokens,
                output_tokens: r.usage.total_output_tokens,
                cost_usd: r.usage.total_cost_usd,
            };
            let _ = send_to_frontend(&usage_msg);

            if r.is_error {
                let _ = send_to_frontend(&BackendMessage::Error {
                    message: r.result.clone(),
                    recoverable: true,
                });
            }

            // Generate prompt suggestions after query completion
            generate_and_send_suggestions(engine, suggestion_svc);

            Ok(())
        }
    }
}

/// Map a [`StreamEvent`] to the appropriate [`BackendMessage`] and send it.
fn handle_stream_event(event: &StreamEvent, message_id: &str) -> std::io::Result<()> {
    match event {
        StreamEvent::MessageStart { .. } => send_to_frontend(&BackendMessage::StreamStart {
            message_id: message_id.to_string(),
        }),
        StreamEvent::ContentBlockStart { .. } => Ok(()),
        StreamEvent::ContentBlockDelta { ref delta, .. } => {
            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                send_to_frontend(&BackendMessage::StreamDelta {
                    message_id: message_id.to_string(),
                    text: text.to_string(),
                })
            } else {
                Ok(())
            }
        }
        StreamEvent::MessageStop => send_to_frontend(&BackendMessage::StreamEnd {
            message_id: message_id.to_string(),
        }),
        _ => {
            debug!("headless: ignoring stream event {:?}", event);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Prompt suggestions
// ---------------------------------------------------------------------------

/// Generate prompt suggestions from the last assistant message and send them.
fn generate_and_send_suggestions(
    engine: &Arc<QueryEngine>,
    svc: &Arc<Mutex<PromptSuggestionService>>,
) {
    let messages = engine.messages();

    let mut svc = svc.lock();

    // Check suppression (too few messages, rate-limited, etc.)
    if svc.get_suppression_reason(messages.len(), false).is_some() {
        return;
    }

    // Find last assistant message
    let last_assistant = messages.iter().rev().find_map(|msg| match msg {
        Message::Assistant(a) => Some(a),
        _ => None,
    });
    let Some(assistant) = last_assistant else {
        return;
    };

    // Extract tool names and text summary
    let tool_names: Vec<String> = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let summary: String = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(suggestions) = svc.try_generate(&summary, &tool_names) {
        let items: Vec<String> = suggestions
            .into_iter()
            .take(3)
            .map(|s| format!("{} {}", s.category.icon(), s.text))
            .collect();

        if !items.is_empty() {
            let _ = send_to_frontend(&BackendMessage::Suggestions { items });
        }
    }
}
