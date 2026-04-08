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

use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::message::StreamEvent;

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

    // ── 2. Send Ready ───────────────────────────────────────────────
    let ready = BackendMessage::Ready {
        session_id: engine.session_id.to_string(),
        model,
        cwd: engine.cwd().to_string(),
    };
    send_to_frontend(&ready)?;

    // ── 3. Read stdin lines ─────────────────────────────────────────
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
            // ── SubmitPrompt ────────────────────────────────────────
            FrontendMessage::SubmitPrompt { text, id } => {
                debug!("headless: submit_prompt id={}", id);
                engine.reset_abort();

                let engine_clone = engine.clone();
                let message_id = id;

                // Spawn a task to stream the response so we don't block the
                // stdin reader (abort/permission responses can arrive while streaming).
                tokio::spawn(async move {
                    let stream = engine_clone.submit_message(&text, QuerySource::ReplMainThread);
                    let mut stream = std::pin::pin!(stream);

                    while let Some(sdk_msg) = stream.next().await {
                        let send_result = handle_sdk_message(&sdk_msg, &message_id);

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

            // ── PermissionResponse ──────────────────────────────────
            FrontendMessage::PermissionResponse {
                tool_use_id,
                decision,
            } => {
                debug!(
                    "headless: permission response tool_use_id={} decision={}",
                    tool_use_id, decision
                );
                // Forward to the waiting permission callback
                if let Some(tx) = pending_permissions.lock().remove(&tool_use_id) {
                    let _ = tx.send(decision);
                } else {
                    warn!(
                        "headless: no pending permission for tool_use_id={}",
                        tool_use_id
                    );
                }
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
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SdkMessage → BackendMessage mapping
// ---------------------------------------------------------------------------

/// Map a single [`SdkMessage`] to the appropriate [`BackendMessage`](s) and
/// send them to the frontend. This is the central dispatch for the headless
/// protocol — every `SdkMessage` variant is handled here.
fn handle_sdk_message(sdk_msg: &SdkMessage, message_id: &str) -> std::io::Result<()> {
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
                if let crate::types::message::ContentBlock::ToolUse { id, name, input } = block {
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
            // Tool results flow through UserReplay in SDK mode.
            // Forward as system_info so the frontend knows about them.
            if replay.is_synthetic {
                debug!("headless: user replay (synthetic): {}", replay.content);
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
                send_to_frontend(&BackendMessage::Error {
                    message: r.result.clone(),
                    recoverable: true,
                })
            } else {
                Ok(())
            }
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
