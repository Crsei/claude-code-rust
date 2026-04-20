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
