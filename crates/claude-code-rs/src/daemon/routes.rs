//! Route handlers for the KAIROS daemon HTTP API.
//!
//! Groups:
//! - **API routes** (`/api/*`) -- query submission, abort, status, attach/detach
//! - **Webhook routes** (`/webhook/*`) -- Phase-3 stubs for GitHub/Slack/generic
//! - **Health** (`/health`) -- simple liveness probe

#![allow(dead_code)]

use std::sync::atomic::Ordering;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tracing::{info, warn};

use crate::commands::{self, CommandContext, CommandResult};
use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::plan_workflow::PlanWorkflowRecord;

use super::process_state::{self, DaemonStatusSnapshot, DaemonWorkerSummary};
use super::protocol::{self, DaemonCommandKind};
use super::state::{DaemonState, SseEvent};
use super::supervisor::ASSISTANT_WORKER_ID;
use super::team_memory_proxy;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub text: String,
    pub id: Option<String>,
    pub idempotency_key: Option<String>,
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
    pub daemon_sleep_until: Option<String>,
    pub daemon_sleep_reason: Option<String>,
    pub permission_mode: String,
    pub plan_workflow: Option<PlanWorkflowRecord>,
    pub supervisor_status: String,
    pub supervisor_pid: Option<u32>,
    pub health_url: Option<String>,
    pub workers: Vec<DaemonWorkerSummary>,
    pub command_root: String,
    pub assistant_event_log: String,
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
/// Returns `None` only for message variants that are internal to the SDK
/// stream and do not have a daemon-facing contract.
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
        SdkMessage::Tombstone(t) => (
            "tombstone".to_string(),
            json!({
                "message_id": message_id,
                "assistant_id": t.message.uuid,
                "session_id": t.session_id,
            }),
        ),
        SdkMessage::ApiRetry(retry) => (
            "api_retry".to_string(),
            json!({
                "message_id": message_id,
                "attempt": retry.attempt,
                "max_retries": retry.max_retries,
                "retry_delay_ms": retry.retry_delay_ms,
                "error_status": retry.error_status,
                "error": retry.error,
                "session_id": retry.session_id,
            }),
        ),
        SdkMessage::CompactBoundary(boundary) => (
            "compact_boundary".to_string(),
            json!({
                "message_id": message_id,
                "compact_metadata": boundary.compact_metadata,
                "session_id": boundary.session_id,
            }),
        ),
        SdkMessage::ToolUseSummary(summary) => (
            "tool_use_summary".to_string(),
            json!({
                "message_id": message_id,
                "summary": summary.summary,
                "preceding_tool_use_ids": summary.preceding_tool_use_ids,
                "session_id": summary.session_id,
            }),
        ),
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

fn require_control_token(headers: &HeaderMap) -> Result<(), Json<Value>> {
    let Some(candidate) = extract_control_token(headers) else {
        return Err(Json(json!({
            "status": "unauthorized",
            "message": "missing daemon control token",
        })));
    };
    match process_state::verify_control_token(candidate) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Json(json!({
            "status": "unauthorized",
            "message": "invalid daemon control token",
        }))),
        Err(err) => Err(Json(json!({
            "status": "error",
            "message": err.to_string(),
        }))),
    }
}

fn extract_control_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-cc-rust-daemon-token")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        })
}

/// `POST /api/submit` -- submit a user message and begin streaming.
async fn submit(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    Json(body): Json<SubmitRequest>,
) -> Json<Value> {
    if let Err(response) = require_control_token(&headers) {
        return response;
    }
    submit_authorized(state, body).await
}

async fn submit_authorized(state: DaemonState, body: SubmitRequest) -> Json<Value> {
    let text = body.text;
    let message_id = body.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let command = match protocol::enqueue_command(
        ASSISTANT_WORKER_ID,
        DaemonCommandKind::Submit,
        json!({
            "text": text.clone(),
            "message_id": message_id.clone(),
            "source": "http",
        }),
        Some(body.idempotency_key.unwrap_or_else(|| message_id.clone())),
    ) {
        Ok(command) => command,
        Err(err) => {
            return Json(json!({
                "status": "error",
                "message": err.to_string(),
            }));
        }
    };

    info!(message_id, text_len = text.len(), "submit received");
    super::memory_log::append_log_entry(&format!("user submit: {}", &text));
    state.broadcast(SseEvent {
        id: String::new(),
        event_type: "daemon_command".to_string(),
        data: json!({
            "command_id": command.command_id,
            "worker_id": command.target_worker_id,
            "kind": "submit",
        }),
    });

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

    Json(json!({
        "status": "ok",
        "message_id": message_id,
        "command_id": command.command_id,
    }))
}

/// `POST /api/abort` -- abort the currently running query.
async fn abort(State(state): State<DaemonState>, headers: HeaderMap) -> Json<Value> {
    if let Err(response) = require_control_token(&headers) {
        return response;
    }
    info!("abort request received");
    let command = match protocol::enqueue_command(
        ASSISTANT_WORKER_ID,
        DaemonCommandKind::Abort,
        json!({ "source": "http" }),
        None,
    ) {
        Ok(command) => command,
        Err(err) => {
            return Json(json!({
                "status": "error",
                "message": err.to_string(),
            }));
        }
    };
    state.engine.abort();
    Json(json!({ "status": "ok", "command_id": command.command_id }))
}

/// `POST /api/command` -- execute a slash command.
async fn command(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    Json(body): Json<CommandRequest>,
) -> Json<Value> {
    if let Err(response) = require_control_token(&headers) {
        return response;
    }
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
        session_id: state.engine.current_session_id(),
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
                    let session_id = state.engine.start_new_session();
                    Json(json!({
                        "status": "ok",
                        "kind": "clear",
                        "session_id": session_id.to_string()
                    }))
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
async fn permission(
    State(_state): State<DaemonState>,
    headers: HeaderMap,
    Json(body): Json<PermissionRequest>,
) -> Json<Value> {
    if let Err(response) = require_control_token(&headers) {
        return response;
    }
    warn!(
        tool_use_id = %body.tool_use_id,
        decision = %body.decision,
        "permission endpoint queued daemon command"
    );
    let command = match protocol::enqueue_command(
        ASSISTANT_WORKER_ID,
        DaemonCommandKind::PermissionResponse,
        json!({
            "tool_use_id": body.tool_use_id.clone(),
            "decision": body.decision.clone(),
            "source": "http",
        }),
        None,
    ) {
        Ok(command) => command,
        Err(err) => {
            return Json(json!({
                "status": "error",
                "message": err.to_string(),
            }));
        }
    };
    Json(json!({ "status": "queued", "command_id": command.command_id }))
}

/// `GET /api/status` -- return daemon status.
async fn status(State(state): State<DaemonState>) -> Json<StatusResponse> {
    let app_state = state.engine.app_state();
    let daemon_sleep = process_state::active_sleep_state().unwrap_or_default();
    let (supervisor_status, supervisor_pid, health_url, workers) =
        match process_state::status_snapshot() {
            Ok(DaemonStatusSnapshot::Running(process)) => (
                "running".to_string(),
                Some(process.pid),
                Some(process.health_url),
                process.workers,
            ),
            Ok(DaemonStatusSnapshot::Stale(process)) => (
                "stale".to_string(),
                Some(process.pid),
                Some(process.health_url),
                process.workers,
            ),
            Ok(DaemonStatusSnapshot::Stopped) => ("stopped".to_string(), None, None, Vec::new()),
            Err(err) => (format!("error: {err}"), None, None, Vec::new()),
        };
    Json(StatusResponse {
        kairos_active: state.features.kairos,
        proactive: state.features.proactive,
        query_running: state.is_query_running.load(Ordering::SeqCst),
        clients_connected: state.clients.read().len(),
        sleeping: state.engine.is_sleeping() || daemon_sleep.is_some(),
        daemon_sleep_until: daemon_sleep
            .as_ref()
            .map(|sleep| sleep.sleeping_until.to_rfc3339()),
        daemon_sleep_reason: daemon_sleep.and_then(|sleep| sleep.reason),
        permission_mode: app_state.tool_permission_context.mode.as_str().to_string(),
        plan_workflow: app_state.plan_workflow,
        supervisor_status,
        supervisor_pid,
        health_url,
        workers,
        command_root: protocol::commands_dir().display().to_string(),
        assistant_event_log: protocol::worker_events_path(ASSISTANT_WORKER_ID)
            .display()
            .to_string(),
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
async fn resize(headers: HeaderMap) -> Json<Value> {
    if let Err(response) = require_control_token(&headers) {
        return response;
    }
    Json(json!({ "status": "noop", "message": "resize forwarding is not implemented yet" }))
}

/// `GET /api/history` -- return conversation history (stub).
async fn history(State(state): State<DaemonState>) -> Json<Value> {
    let daemon_events = protocol::read_worker_events(ASSISTANT_WORKER_ID).unwrap_or_default();
    Json(json!({
        "history": [],
        "sse_events": state.events_since("0"),
        "daemon_events": daemon_events,
    }))
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

async fn webhook_github(headers: HeaderMap, body: Bytes) -> Json<Value> {
    if let Err(message) = verify_optional_github_signature(&headers, &body) {
        return Json(json!({
            "status": "unauthorized",
            "source": "github",
            "message": message,
        }));
    }

    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload) => payload,
        Err(error) => {
            return Json(json!({
                "status": "error",
                "source": "github",
                "message": format!("invalid JSON payload: {error}"),
            }));
        }
    };
    let event_name = header_str(&headers, "x-github-event");
    let delivery_id = header_str(&headers, "x-github-delivery");
    let Some(activity) = crate::tools::pr_activity::parse_github_pr_activity(
        &payload,
        event_name.as_deref(),
        delivery_id.as_deref(),
    ) else {
        return Json(json!({
            "status": "ignored",
            "source": "github",
            "event": event_name,
        }));
    };

    match crate::tools::pr_activity::route_github_pr_activity(&activity) {
        Ok(result) => Json(json!({
            "status": "received",
            "source": "github",
            "event": event_name,
            "matched": result.matched,
            "delivered": result.delivered,
        })),
        Err(error) => Json(json!({
            "status": "error",
            "source": "github",
            "message": error.to_string(),
        })),
    }
}

async fn webhook_slack() -> Json<Value> {
    Json(json!({ "status": "received", "source": "slack" }))
}

async fn webhook_generic() -> Json<Value> {
    Json(json!({ "status": "received", "source": "generic" }))
}

fn verify_optional_github_signature(headers: &HeaderMap, body: &[u8]) -> Result<(), String> {
    let secret = std::env::var("CC_RUST_GITHUB_WEBHOOK_SECRET")
        .or_else(|_| std::env::var("GITHUB_WEBHOOK_SECRET"))
        .ok()
        .filter(|value| !value.trim().is_empty());
    let Some(secret) = secret else {
        return Ok(());
    };
    let Some(signature) = header_str(headers, "x-hub-signature-256") else {
        return Err("missing X-Hub-Signature-256".to_string());
    };
    if crate::daemon::webhook::verify_github_signature(body, &signature, &secret) {
        Ok(())
    } else {
        Err("invalid GitHub webhook signature".to_string())
    }
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
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

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::engine::sdk_types::{SdkApiRetry, SdkCompactBoundary, SdkToolUseSummary};
    use crate::types::message::CompactMetadata;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn daemon_sse_broadcasts_api_retry_events() {
        let event = sdk_message_to_sse(
            &SdkMessage::ApiRetry(SdkApiRetry {
                attempt: 1,
                max_retries: 3,
                retry_delay_ms: 250,
                error_status: Some(529),
                error: "overloaded".to_string(),
                session_id: "session-1".to_string(),
                uuid: Uuid::new_v4(),
            }),
            "message-1",
        )
        .expect("api retry should be broadcast");

        assert_eq!(event.event_type, "api_retry");
        assert_eq!(event.data["message_id"], "message-1");
        assert_eq!(event.data["attempt"], 1);
        assert_eq!(event.data["max_retries"], 3);
        assert_eq!(event.data["error_status"], 529);
        assert_eq!(event.data["session_id"], "session-1");
    }

    #[test]
    fn daemon_sse_broadcasts_compact_boundaries() {
        let event = sdk_message_to_sse(
            &SdkMessage::CompactBoundary(SdkCompactBoundary {
                session_id: "session-1".to_string(),
                uuid: Uuid::new_v4(),
                compact_metadata: Some(CompactMetadata {
                    pre_compact_token_count: 100,
                    post_compact_token_count: 40,
                    preserved_segment: None,
                }),
            }),
            "message-1",
        )
        .expect("compact boundary should be broadcast");

        assert_eq!(event.event_type, "compact_boundary");
        assert_eq!(event.data["message_id"], "message-1");
        assert_eq!(
            event.data["compact_metadata"]["pre_compact_token_count"],
            100
        );
        assert_eq!(
            event.data["compact_metadata"]["post_compact_token_count"],
            40
        );
        assert_eq!(event.data["session_id"], "session-1");
    }

    #[test]
    fn daemon_sse_broadcasts_tool_use_summaries() {
        let event = sdk_message_to_sse(
            &SdkMessage::ToolUseSummary(SdkToolUseSummary {
                summary: "Read finished".to_string(),
                preceding_tool_use_ids: vec!["toolu_1".to_string()],
                session_id: "session-1".to_string(),
                uuid: Uuid::new_v4(),
            }),
            "message-1",
        )
        .expect("tool use summary should be broadcast");

        assert_eq!(event.event_type, "tool_use_summary");
        assert_eq!(event.data["message_id"], "message-1");
        assert_eq!(event.data["summary"], "Read finished");
        assert_eq!(event.data["preceding_tool_use_ids"][0], "toolu_1");
        assert_eq!(event.data["session_id"], "session-1");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn github_webhook_routes_matching_pr_activity_to_mailbox() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("CC_RUST_HOME", home.path().to_str().unwrap());
        let _github_secret = EnvGuard::set("CC_RUST_GITHUB_WEBHOOK_SECRET", "");
        let _legacy_secret = EnvGuard::set("GITHUB_WEBHOOK_SECRET", "");
        crate::teams::helpers::create_team("phase4-route", None, None, ".").unwrap();
        crate::tools::pr_activity::subscribe(
            "AIclassmanager".into(),
            "cc-rust".into(),
            42,
            "phase4-route".into(),
            crate::teams::constants::TEAM_LEAD_NAME.into(),
        )
        .unwrap();
        let body = serde_json::to_vec(&json!({
            "action": "opened",
            "repository": {
                "name": "cc-rust",
                "owner": { "login": "AIclassmanager" }
            },
            "pull_request": {
                "number": 42,
                "title": "Phase 4",
                "html_url": "https://github.com/AIclassmanager/cc-rust/pull/42"
            },
            "sender": { "login": "octocat" }
        }))
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-github-event", "pull_request".parse().unwrap());
        headers.insert("x-github-delivery", "delivery-42".parse().unwrap());

        let Json(response) = webhook_github(headers, Bytes::from(body)).await;

        assert_eq!(response["status"], "received");
        assert_eq!(response["matched"], 1);
        assert_eq!(response["delivered"], 1);
        let inbox = crate::teams::mailbox::read_mailbox(
            crate::teams::constants::TEAM_LEAD_NAME,
            "phase4-route",
        )
        .unwrap();
        assert_eq!(inbox.len(), 1);
        assert!(inbox[0].text.contains("delivery-42"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn github_webhook_rejects_bad_signature_when_secret_is_configured() {
        let _secret = EnvGuard::set("CC_RUST_GITHUB_WEBHOOK_SECRET", "secret");
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-hub-signature-256",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
        );

        let Json(response) = webhook_github(headers, Bytes::from_static(b"{}")).await;

        assert_eq!(response["status"], "unauthorized");
        assert!(response["message"].as_str().unwrap().contains("invalid"));
    }
}
