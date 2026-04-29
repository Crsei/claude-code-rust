//! Route handlers for the KAIROS daemon HTTP API.
//!
//! Groups:
//! - **API routes** (`/api/*`) -- query submission, abort, status, attach/detach
//! - **Webhook routes** (`/webhook/*`) -- Phase-3 stubs for GitHub/Slack/generic
//! - **Health** (`/health`) -- simple liveness probe

#![allow(dead_code)]

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::StreamExt;
use tracing::{info, warn};

use crate::commands::{self, CommandContext, CommandResult};
use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::plan_workflow::PlanWorkflowRecord;

use super::state::{DaemonState, SseEvent};
use super::team_memory_proxy;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub text: String,
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AbortRequest {}

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub raw: String,
}

#[derive(Debug, Deserialize)]
pub struct PermissionRequest {
    pub tool_use_id: String,
    pub decision: String,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub kairos_active: bool,
    pub proactive: bool,
    pub query_running: bool,
    pub clients_connected: usize,
    pub sleeping: bool,
    pub permission_mode: String,
    pub plan_workflow: Option<PlanWorkflowRecord>,
}

#[derive(Debug, Deserialize)]
pub struct AttachRequest {
    pub client_id: String,
    pub last_seen_event: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DetachRequest {
    pub client_id: String,
}

// ---------------------------------------------------------------------------
// SdkMessage → SseEvent mapping
// ---------------------------------------------------------------------------

/// Convert an [`SdkMessage`] into an [`SseEvent`] suitable for broadcasting
/// over SSE.
///
/// Returns `None` for message variants that do not need to be sent to
/// frontends (currently: `ApiRetry`, `CompactBoundary`, `ToolUseSummary`).
pub fn sdk_message_to_sse(msg: &SdkMessage, message_id: &str) -> Option<SseEvent> {
    let (event_type, data) = match msg {
        SdkMessage::SystemInit(init) => (
            "stream_start".to_string(),
            json!({
                "message_id": message_id,
                "tools": init.tools,
                "model": init.model,
                "session_id": init.session_id,
            }),
        ),
        SdkMessage::StreamEvent(se) => (
            "stream_delta".to_string(),
            json!({
                "message_id": message_id,
                "event": se.event,
                "session_id": se.session_id,
            }),
        ),
        SdkMessage::Assistant(am) => (
            "assistant_message".to_string(),
            json!({
                "message_id": message_id,
                "message": am.message,
                "session_id": am.session_id,
            }),
        ),
        SdkMessage::UserReplay(ur) => (
            "user_replay".to_string(),
            json!({
                "message_id": message_id,
                "content": ur.content,
                "session_id": ur.session_id,
            }),
        ),
        SdkMessage::Result(r) => (
            "stream_end".to_string(),
            json!({
                "message_id": message_id,
                "subtype": r.subtype,
                "is_error": r.is_error,
                "duration_ms": r.duration_ms,
                "result": r.result,
                "session_id": r.session_id,
            }),
        ),
        // Variants we do not broadcast to SSE clients.
        SdkMessage::ApiRetry(_)
        | SdkMessage::CompactBoundary(_)
        | SdkMessage::ToolUseSummary(_) => return None,
    };

    Some(SseEvent {
        id: String::new(), // filled by broadcast()
        event_type,
        data,
    })
}

// ---------------------------------------------------------------------------
// API routes
// ---------------------------------------------------------------------------

/// Returns a [`Router`] containing all `/api/*` endpoints.
pub fn api_routes() -> Router<DaemonState> {
    Router::new()
        .route("/api/submit", post(submit))
        .route("/api/abort", post(abort))
        .route("/api/command", post(command))
        .route("/api/permission", post(permission))
        .route("/api/status", get(status))
        .route("/api/attach", post(attach))
        .route("/api/detach", post(detach))
        .route("/api/resize", post(resize))
        .route("/api/history", get(history))
}

/// `POST /api/submit` -- submit a user message and begin streaming.
async fn submit(State(state): State<DaemonState>, Json(body): Json<SubmitRequest>) -> Json<Value> {
    let text = body.text;
    let message_id = body.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    info!(message_id, text_len = text.len(), "submit received");
    super::memory_log::append_log_entry(&format!("user submit: {}", &text));

    let classifier = crate::plan_workflow::classify_plan_entry(&text, &state.engine.app_state());
    if classifier.should_enter {
        match crate::plan_workflow::enter_engine_plan_mode(
            &state.engine,
            "daemon_classifier",
            Some(&text),
            Some(&classifier.reason),
        ) {
            Ok(record) => {
                state.broadcast(SseEvent {
                    id: String::new(),
                    event_type: "plan_workflow_event".to_string(),
                    data: crate::plan_workflow::event_payload(
                        &record,
                        "classifier_entered",
                        &crate::plan_workflow::summarize(&record),
                    ),
                });
            }
            Err(err) => warn!(error = %err, "daemon plan classifier sync failed"),
        }
    }

    // Wake engine, set running flag, spawn async task.
    state.engine.wake_up();
    state.is_query_running.store(true, Ordering::SeqCst);

    let engine = state.engine.clone();
    let state_clone = state.clone();
    let mid = message_id.clone();

    tokio::spawn(async move {
        let stream = engine.submit_message(&text, QuerySource::ReplMainThread);
        tokio::pin!(stream);
        while let Some(sdk_msg) = stream.next().await {
            if let Some(sse_event) = sdk_message_to_sse(&sdk_msg, &mid) {
                state_clone.broadcast(sse_event);
            }
        }
        state_clone.is_query_running.store(false, Ordering::SeqCst);
    });

    Json(json!({ "status": "ok", "message_id": message_id }))
}

/// `POST /api/abort` -- abort the currently running query.
async fn abort(State(state): State<DaemonState>) -> Json<Value> {
    info!("abort request received");
    state.engine.abort();
    Json(json!({ "status": "ok" }))
}

/// `POST /api/command` -- execute a slash command.
async fn command(
    State(state): State<DaemonState>,
    Json(body): Json<CommandRequest>,
) -> Json<Value> {
    let raw = body.raw.trim().to_string();
    let Some((cmd_idx, args)) = commands::parse_command_input(&raw) else {
        return Json(json!({ "status": "error", "message": format!("unknown command: {raw}") }));
    };

    let all_commands = commands::get_all_commands();
    let cmd = &all_commands[cmd_idx];
    let original_app_state = state.engine.app_state();
    let original_plan = original_app_state.plan_workflow.clone();
    let original_mode = original_app_state.tool_permission_context.mode.clone();

    let mut ctx = CommandContext {
        messages: state.engine.messages(),
        cwd: std::path::PathBuf::from(state.engine.cwd()),
        app_state: original_app_state,
        session_id: state.engine.session_id.clone(),
    };

    match cmd.handler.execute(&args, &mut ctx).await {
        Ok(result) => {
            let plan_changed = ctx.app_state.plan_workflow != original_plan
                || ctx.app_state.tool_permission_context.mode != original_mode;
            crate::plan_workflow::sync_command_app_state(&state.engine, &ctx.app_state);

            if plan_changed {
                if let Some(record) = ctx.app_state.plan_workflow.clone() {
                    state.broadcast(SseEvent {
                        id: String::new(),
                        event_type: "plan_workflow_event".to_string(),
                        data: crate::plan_workflow::event_payload(
                            &record,
                            "slash_command",
                            &crate::plan_workflow::summarize(&record),
                        ),
                    });
                }
            }

            match result {
                CommandResult::Output(text) => {
                    state.broadcast(SseEvent {
                        id: String::new(),
                        event_type: "system_info".to_string(),
                        data: json!({ "text": text, "level": "info" }),
                    });
                    Json(json!({
                        "status": "ok",
                        "kind": "output",
                        "permission_mode": state.engine.app_state().tool_permission_context.mode.as_str(),
                        "plan_workflow": state.engine.app_state().plan_workflow,
                    }))
                }
                CommandResult::Clear => {
                    state.engine.clear_messages();
                    Json(json!({ "status": "ok", "kind": "clear" }))
                }
                CommandResult::Exit(text) => {
                    state.broadcast(SseEvent {
                        id: String::new(),
                        event_type: "system_info".to_string(),
                        data: json!({ "text": text, "level": "info" }),
                    });
                    Json(json!({ "status": "ok", "kind": "exit" }))
                }
                CommandResult::Query(_) => Json(json!({
                    "status": "ok",
                    "kind": "query_not_started",
                    "message": "daemon command endpoint does not start query command results yet"
                })),
                CommandResult::None => Json(json!({ "status": "ok", "kind": "none" })),
            }
        }
        Err(err) => Json(json!({ "status": "error", "message": err.to_string() })),
    }
}

/// `POST /api/permission` -- respond to a permission prompt (stub).
async fn permission(Json(body): Json<PermissionRequest>) -> Json<Value> {
    warn!(
        tool_use_id = body.tool_use_id,
        decision = body.decision,
        "permission endpoint is a stub"
    );
    Json(json!({ "status": "stub", "tool_use_id": body.tool_use_id }))
}

/// `GET /api/status` -- return daemon status.
async fn status(State(state): State<DaemonState>) -> Json<StatusResponse> {
    let app_state = state.engine.app_state();
    Json(StatusResponse {
        kairos_active: state.features.kairos,
        proactive: state.features.proactive,
        query_running: state.is_query_running.load(Ordering::SeqCst),
        clients_connected: state.clients.read().len(),
        sleeping: state.engine.is_sleeping(),
        permission_mode: app_state.tool_permission_context.mode.as_str().to_string(),
        plan_workflow: app_state.plan_workflow,
    })
}

/// `POST /api/attach` -- re-attach a client and return missed events.
async fn attach(State(state): State<DaemonState>, Json(body): Json<AttachRequest>) -> Json<Value> {
    info!(client_id = body.client_id, "client attach");
    let missed: Vec<SseEvent> = body
        .last_seen_event
        .as_deref()
        .map(|id| state.events_since(id))
        .unwrap_or_default();

    Json(json!({ "status": "ok", "missed_events": missed }))
}

/// `POST /api/detach` -- remove a client from the SSE registry.
async fn detach(State(state): State<DaemonState>, Json(body): Json<DetachRequest>) -> Json<Value> {
    info!(client_id = body.client_id, "client detach");
    state.clients.write().remove(&body.client_id);
    Json(json!({ "status": "ok" }))
}

/// `POST /api/resize` -- terminal resize notification (stub).
async fn resize() -> Json<Value> {
    Json(json!({ "status": "stub" }))
}

/// `GET /api/history` -- return conversation history (stub).
async fn history() -> Json<Value> {
    Json(json!({ "history": [] }))
}

// ---------------------------------------------------------------------------
// Webhook routes (Phase 3 stubs)
// ---------------------------------------------------------------------------

/// Returns a [`Router`] containing all `/webhook/*` endpoints.
pub fn webhook_routes() -> Router<DaemonState> {
    Router::new()
        .route("/webhook/github", post(webhook_github))
        .route("/webhook/slack", post(webhook_slack))
        .route("/webhook/generic", post(webhook_generic))
}

async fn webhook_github() -> Json<Value> {
    Json(json!({ "status": "received", "source": "github" }))
}

async fn webhook_slack() -> Json<Value> {
    Json(json!({ "status": "received", "source": "slack" }))
}

async fn webhook_generic() -> Json<Value> {
    Json(json!({ "status": "received", "source": "generic" }))
}

// ---------------------------------------------------------------------------
// Team memory proxy route
// ---------------------------------------------------------------------------

/// Returns a [`Router`] containing the team memory proxy route.
pub fn team_memory_routes() -> Router<DaemonState> {
    Router::new().route(
        "/api/claude_code/team_memory",
        get(team_memory_proxy::proxy_team_memory).put(team_memory_proxy::proxy_team_memory),
    )
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// `GET /health` -- simple liveness probe.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
