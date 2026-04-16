//! Axum route handlers for the web chat API.

use std::sync::atomic::Ordering;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;

use super::sse::sdk_stream_to_sse;
use super::state::WebState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Deserialize)]
pub struct AbortRequest {
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

#[derive(Serialize)]
pub struct StateResponse {
    pub model: String,
    pub session_id: String,
    pub tools: Vec<String>,
    pub permission_mode: String,
    pub thinking_enabled: Option<bool>,
    pub fast_mode: bool,
    pub effort: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/chat -- Start a streaming chat response via SSE.
pub async fn chat_handler(
    State(state): State<WebState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Check if already streaming
    if state.is_streaming.load(Ordering::SeqCst) {
        return (
            StatusCode::CONFLICT,
            Json(ApiError {
                error: "A query is already in progress".into(),
                code: "engine_busy".into(),
            }),
        )
            .into_response();
    }

    info!(message = %req.message, "POST /api/chat");

    state.is_streaming.store(true, Ordering::SeqCst);

    // Get the stream from the engine
    let stream = state.engine.submit_message(&req.message, QuerySource::Sdk);

    // Wrap in a stream that clears is_streaming when done
    let is_streaming = state.is_streaming.clone();
    let wrapped_stream = Box::pin(futures::stream::unfold(
        (stream, is_streaming, false),
        |(mut stream, flag, done)| async move {
            if done {
                return None;
            }
            use futures::StreamExt;
            match stream.next().await {
                Some(msg) => {
                    let is_result = matches!(&msg, SdkMessage::Result(_));
                    if is_result {
                        flag.store(false, Ordering::SeqCst);
                    }
                    Some((msg, (stream, flag, is_result)))
                }
                None => {
                    flag.store(false, Ordering::SeqCst);
                    None
                }
            }
        },
    ));

    sdk_stream_to_sse(wrapped_stream).into_response()
}

/// POST /api/abort -- Abort the current generation.
pub async fn abort_handler(
    State(state): State<WebState>,
    Json(_req): Json<AbortRequest>,
) -> impl IntoResponse {
    info!("POST /api/abort");
    state.engine.abort();
    state.is_streaming.store(false, Ordering::SeqCst);
    StatusCode::OK
}

/// GET /api/state -- Return current application state.
pub async fn state_handler(State(state): State<WebState>) -> impl IntoResponse {
    let app_state = state.engine.app_state();
    let permission_mode = match app_state.tool_permission_context.mode {
        crate::types::tool::PermissionMode::Default => "default",
        crate::types::tool::PermissionMode::Auto => "auto",
        crate::types::tool::PermissionMode::Bypass => "bypass",
        crate::types::tool::PermissionMode::Plan => "plan",
    };

    Json(StateResponse {
        model: app_state.main_loop_model.clone(),
        session_id: state.engine.session_id.clone(),
        tools: vec![], // TODO: populate from engine tools
        permission_mode: permission_mode.to_string(),
        thinking_enabled: app_state.thinking_enabled,
        fast_mode: app_state.fast_mode,
        effort: app_state.effort_value.clone(),
    })
}
