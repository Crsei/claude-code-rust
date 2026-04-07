//! Headless event loop — replaces `tui::run_tui()` when `--headless` is passed.
//!
//! Communicates with an external UI process via JSON lines on stdin/stdout
//! using the protocol defined in [`super::protocol`].

use std::sync::Arc;

use futures::StreamExt;
use tokio::io::AsyncBufReadExt;
use tracing::{debug, error, warn};

use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::message::StreamEvent;

use super::protocol::{send_to_frontend, BackendMessage, FrontendMessage};

/// Run the headless event loop.
///
/// Reads [`FrontendMessage`]s from stdin (one JSON object per line) and writes
/// [`BackendMessage`]s to stdout. This function only returns when the UI sends
/// [`FrontendMessage::Quit`] or stdin is closed.
pub async fn run_headless(engine: Arc<QueryEngine>, model: String) -> anyhow::Result<()> {
    // ── 1. Send Ready ───────────────────────────────────────────────────
    let ready = BackendMessage::Ready {
        session_id: engine.session_id.to_string(),
        model,
        cwd: engine.cwd().to_string(),
    };
    send_to_frontend(&ready)?;

    // ── 2. Read stdin lines ─────────────────────────────────────────────
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    loop {
        let line = match lines.next_line().await {
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
                warn!("headless: failed to parse FrontendMessage: {} — line: {}", e, line);
                let _ = send_to_frontend(&BackendMessage::Error {
                    message: format!("invalid FrontendMessage: {}", e),
                    recoverable: true,
                });
                continue;
            }
        };

        match msg {
            // ── SubmitPrompt ────────────────────────────────────────
            FrontendMessage::SubmitPrompt { text, id } => {
                debug!("headless: submit_prompt id={}", id);
                engine.reset_abort();

                let engine_clone = engine.clone();
                let message_id = id;

                // Spawn a task to stream the response so we don't block the
                // stdin reader (abort can arrive while streaming).
                tokio::spawn(async move {
                    let stream =
                        engine_clone.submit_message(&text, QuerySource::ReplMainThread);
                    let mut stream = std::pin::pin!(stream);

                    while let Some(sdk_msg) = stream.next().await {
                        let send_result = match &sdk_msg {
                            SdkMessage::StreamEvent(evt) => {
                                handle_stream_event(&evt.event, &message_id)
                            }
                            SdkMessage::Assistant(a) => {
                                let content = serde_json::to_value(&a.message.content)
                                    .unwrap_or(serde_json::Value::Null);
                                send_to_frontend(&BackendMessage::AssistantMessage {
                                    id: a.message.uuid.to_string(),
                                    content,
                                    cost_usd: a.message.cost_usd,
                                })
                            }
                            SdkMessage::Result(r) => {
                                // Always send StreamEnd to clear UI streaming state
                                let _ = send_to_frontend(&BackendMessage::StreamEnd {
                                    message_id: message_id.clone(),
                                });

                                let usage_msg = BackendMessage::UsageUpdate {
                                    input_tokens: r.usage.total_input_tokens,
                                    output_tokens: r.usage.total_output_tokens,
                                    cost_usd: r.usage.total_cost_usd,
                                };
                                let _ = send_to_frontend(&usage_msg);

                                if r.is_error {
                                    send_to_frontend(&BackendMessage::Error {
                                        message: r.result.clone(),
                                        recoverable: true,
                                    })
                                } else {
                                    Ok(())
                                }
                            }
                            _ => {
                                debug!("headless: ignoring SdkMessage variant");
                                Ok(())
                            }
                        };

                        if let Err(e) = send_result {
                            error!("headless: failed to send to frontend: {}", e);
                            break;
                        }
                    }
                });
            }

            // ── AbortQuery ──────────────────────────────────────────
            FrontendMessage::AbortQuery => {
                debug!("headless: abort requested");
                engine.abort();
            }

            // ── SlashCommand ────────────────────────────────────────
            FrontendMessage::SlashCommand { raw } => {
                debug!("headless: slash command: {}", raw);
                let _ = send_to_frontend(&BackendMessage::SystemInfo {
                    text: format!(
                        "slash commands not yet supported in headless mode (got: {})",
                        raw
                    ),
                    level: "warning".to_string(),
                });
            }

            // ── Resize ──────────────────────────────────────────────
            FrontendMessage::Resize { cols, rows } => {
                debug!("headless: resize {}x{}", cols, rows);
            }

            // ── Quit ────────────────────────────────────────────────
            FrontendMessage::Quit => {
                debug!("headless: quit requested");
                break;
            }

            // ── PermissionResponse ──────────────────────────────────
            FrontendMessage::PermissionResponse {
                tool_use_id,
                decision,
            } => {
                debug!(
                    "headless: permission response tool_use_id={} decision={}",
                    tool_use_id, decision
                );
                // Permission system integration is for a later phase.
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a [`StreamEvent`] to the appropriate [`BackendMessage`] and send it.
fn handle_stream_event(event: &StreamEvent, message_id: &str) -> std::io::Result<()> {
    match event {
        StreamEvent::MessageStart { .. } => {
            send_to_frontend(&BackendMessage::StreamStart {
                message_id: message_id.to_string(),
            })
        }
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
