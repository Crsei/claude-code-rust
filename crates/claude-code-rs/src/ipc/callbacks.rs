//! Permission and AskUserQuestion callback installation.
//!
//! The headless runtime needs to bridge the engine's callback mechanism with
//! the IPC protocol.  This module provides functions that install the
//! appropriate closures on the [`QueryEngine`], capturing the pending-request
//! maps and the [`FrontendSink`] so that permission requests and questions are
//! forwarded to the frontend and responses are awaited via oneshot channels.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::engine::lifecycle::QueryEngine;

use super::protocol::BackendMessage;
use super::sink::FrontendSink;

/// Pending permission requests awaiting a response from the frontend.
pub type PendingPermissions = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;
/// Pending AskUserQuestion requests awaiting the user's next submit_prompt.
pub type PendingQuestions = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

/// Install the permission callback on the engine.
///
/// When the engine requests a tool-use permission, a `PermissionRequest`
/// message is sent to the frontend and a oneshot channel is registered in
/// `pending`.  The headless runtime completes the channel when it receives a
/// `PermissionResponse` from the frontend.
pub fn install_permission_callback(
    engine: &QueryEngine,
    pending: PendingPermissions,
    sink: FrontendSink,
) {
    let callback: crate::types::tool::PermissionCallback = Arc::new(
        move |tool_use_id: String, tool_name: String, description: String, options: Vec<String>| {
            let pending = pending.clone();
            let sink = sink.clone();
            Box::pin(async move {
                let _ = sink.send(&BackendMessage::PermissionRequest {
                    tool_use_id: tool_use_id.clone(),
                    tool: tool_name,
                    command: description,
                    options,
                });

                let (tx, rx) = oneshot::channel();
                pending.lock().insert(tool_use_id, tx);

                match rx.await {
                    Ok(decision) => decision,
                    Err(_) => "deny".to_string(),
                }
            })
        },
    );
    engine.set_permission_callback(callback);
}

/// Install the `ToolProgress` callback on the engine.
///
/// Whenever a tool emits a [`ToolProgress`](crate::types::tool::ToolProgress)
/// event — typically the Bash tool streaming its stdout tail — the callback
/// pulls the relevant fields out of the attached `data` JSON and sends a
/// [`BackendMessage::ToolProgress`] to the frontend.
///
/// Missing fields are tolerated: only `tool_use_id` is required to drive UI
/// routing; everything else is optional and the frontend falls back to its
/// empty-state rendering.
pub fn install_tool_progress_callback(engine: &QueryEngine, sink: FrontendSink) {
    let callback = Arc::new(move |progress: crate::types::tool::ToolProgress| {
        let data = &progress.data;
        let tool = data
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let output = data
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let elapsed_seconds = data
            .get("elapsed_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_lines = data.get("total_lines").and_then(|v| v.as_u64());
        let total_bytes = data.get("total_bytes").and_then(|v| v.as_u64());
        let timeout_ms = data.get("timeout_ms").and_then(|v| v.as_u64());

        let _ = sink.send(&BackendMessage::ToolProgress {
            tool_use_id: progress.tool_use_id,
            tool,
            output,
            elapsed_seconds,
            total_lines,
            total_bytes,
            timeout_ms,
        });
    });
    engine.set_tool_progress_callback(callback);
}

/// Install the AskUserQuestion callback on the engine.
///
/// When the engine asks a question, a `QuestionRequest` message is sent to the
/// frontend and a oneshot channel is registered in `pending`.  The headless
/// runtime completes the channel when it receives a `QuestionResponse` (or, for
/// backward compatibility, a `SubmitPrompt` while a question is pending).
pub fn install_ask_user_callback(
    engine: &QueryEngine,
    pending: PendingQuestions,
    sink: FrontendSink,
) {
    let callback: crate::types::tool::AskUserCallback = Arc::new(move |question: String| {
        let pending = pending.clone();
        let sink = sink.clone();
        Box::pin(async move {
            let question_id = uuid::Uuid::new_v4().to_string();
            let _ = sink.send(&BackendMessage::QuestionRequest {
                id: question_id.clone(),
                text: question,
            });

            let (tx, rx) = oneshot::channel();
            pending.lock().insert(question_id, tx);

            rx.await.unwrap_or_default()
        })
    });
    engine.set_ask_user_callback(callback);
}
