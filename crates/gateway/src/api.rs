use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::adapters::lark::LarkAdapter;
use crate::adapters::telegram::TelegramAdapter;
use crate::auth::{extract_gateway_token, GatewayAuthMode, GatewayAuthVerifier};
use crate::{
    AdapterProvider, AdapterRegistry, AdapterTestMessage, BusySnapshot, GatewayCommandSink,
    GatewayConfig, GatewayDiagnostic, GatewayError, GatewayPolicy, GatewayRunAction,
    GatewayRunSubmission, GatewayRunner, GatewayStore, RunId, RunMeta, RunRequest,
    SessionKeyPolicy,
};

const CAPABILITIES_PATH: &str = "/remote-control/v1/capabilities";
const RUNS_PATH: &str = "/remote-control/v1/runs";
const RUN_PATH: &str = "/remote-control/v1/runs/{run_id}";
const RUN_EVENTS_PATH: &str = "/remote-control/v1/runs/{run_id}/events";
const RUN_STOP_PATH: &str = "/remote-control/v1/runs/{run_id}/stop";
const RUN_APPROVAL_PATH: &str = "/remote-control/v1/runs/{run_id}/approval";
const RUN_ASK_USER_PATH: &str = "/remote-control/v1/runs/{run_id}/ask-user";
const ADAPTERS_PATH: &str = "/remote-control/v1/adapters";
const ADAPTER_CONNECT_PATH: &str = "/remote-control/v1/adapters/{provider}/connect";
const ADAPTER_TEST_PATH: &str = "/remote-control/v1/adapters/{provider}/test-message";

macro_rules! authorize_or_return {
    ($state:expr, $headers:expr) => {
        if let Err(error) = ($state.auth_verify)(extract_gateway_token(&$headers)) {
            return error_response(error);
        }
    };
}

macro_rules! run_id_or_return {
    ($run_id:expr) => {
        match RunId::from_string($run_id) {
            Ok(run_id) => run_id,
            Err(error) => return error_response(error),
        }
    };
}

pub trait GatewayBusySnapshotProvider: Clone + Send + Sync + 'static {
    fn snapshot(&self) -> BusySnapshot;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StaticBusySnapshotProvider {
    snapshot: BusySnapshot,
}

impl StaticBusySnapshotProvider {
    pub fn new(snapshot: BusySnapshot) -> Self {
        Self { snapshot }
    }
}

impl GatewayBusySnapshotProvider for StaticBusySnapshotProvider {
    fn snapshot(&self) -> BusySnapshot {
        self.snapshot
    }
}

#[derive(Clone)]
pub struct GatewayApiState {
    runner: GatewayRunner,
    sink: Arc<dyn GatewayCommandSink + Send + Sync>,
    auth_mode: GatewayAuthMode,
    auth_verify: Arc<dyn Fn(Option<&str>) -> Result<(), GatewayError> + Send + Sync>,
    busy_snapshot: Arc<dyn Fn() -> BusySnapshot + Send + Sync>,
    policy: GatewayPolicy,
    config: GatewayConfig,
}

impl GatewayApiState {
    pub fn new<S, A, B>(
        runner: GatewayRunner,
        sink: S,
        auth: A,
        snapshot: B,
        policy: GatewayPolicy,
        config: GatewayConfig,
    ) -> Self
    where
        S: GatewayCommandSink + Send + Sync + 'static,
        A: GatewayAuthVerifier,
        B: GatewayBusySnapshotProvider,
    {
        let auth_mode = auth.mode();
        Self {
            runner,
            sink: Arc::new(sink),
            auth_mode,
            auth_verify: Arc::new(move |candidate| auth.verify(candidate)),
            busy_snapshot: Arc::new(move || snapshot.snapshot()),
            policy,
            config,
        }
    }

    pub fn with_default_runner<S, A, B>(sink: S, auth: A, snapshot: B) -> Self
    where
        S: GatewayCommandSink + Send + Sync + 'static,
        A: GatewayAuthVerifier,
        B: GatewayBusySnapshotProvider,
    {
        let config = GatewayConfig::default();
        let policy = GatewayPolicy::default();
        let runner = GatewayRunner::new(
            GatewayStore::new(config.persistence.clone(), SessionKeyPolicy::default()),
            policy.clone(),
        );
        Self::new(runner, sink, auth, snapshot, policy, config)
    }
}

pub fn router(state: GatewayApiState) -> Router {
    Router::new()
        .route(CAPABILITIES_PATH, get(capabilities))
        .route(RUNS_PATH, post(create_run))
        .route(RUN_PATH, get(get_run))
        .route(RUN_EVENTS_PATH, get(get_run_events))
        .route(RUN_STOP_PATH, post(stop_run))
        .route(RUN_APPROVAL_PATH, post(approval_response))
        .route(RUN_ASK_USER_PATH, post(ask_user_response))
        .route(ADAPTERS_PATH, get(list_adapters))
        .route(ADAPTER_CONNECT_PATH, post(connect_adapter))
        .route(ADAPTER_TEST_PATH, post(test_adapter_message))
        .with_state(state)
}

async fn capabilities(State(state): State<GatewayApiState>, headers: HeaderMap) -> Response {
    authorize_or_return!(state, headers);
    Json(capabilities_body(&state)).into_response()
}

async fn create_run(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Json(request): Json<RunRequest>,
) -> Response {
    authorize_or_return!(state, headers);

    match state
        .runner
        .submit_run(request, (state.busy_snapshot)(), state.sink.as_ref())
    {
        Ok(submission) => {
            let status = status_for_submission(&submission);
            (status, Json(create_run_body(submission))).into_response()
        }
        Err(error) => error_response(error),
    }
}

async fn get_run(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response {
    authorize_or_return!(state, headers);
    let run_id = run_id_or_return!(run_id);
    match GatewayStore::new(
        state.config.persistence.clone(),
        SessionKeyPolicy::default(),
    )
    .load_run(&run_id)
    {
        Ok(meta) => Json(run_body(meta)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn get_run_events(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response {
    authorize_or_return!(state, headers);
    let run_id = run_id_or_return!(run_id);
    match GatewayStore::new(
        state.config.persistence.clone(),
        SessionKeyPolicy::default(),
    )
    .read_events(&run_id)
    {
        Ok(events) => Json(json!({ "events": events })).into_response(),
        Err(error) => error_response(error),
    }
}

async fn stop_run(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response {
    authorize_or_return!(state, headers);
    let run_id = run_id_or_return!(run_id);
    match state.runner.stop_run(&run_id, state.sink.as_ref()) {
        Ok(submission) => {
            let status = status_for_submission(&submission);
            (status, Json(create_run_body(submission))).into_response()
        }
        Err(error) => error_response(error),
    }
}

async fn approval_response(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(request): Json<ApprovalResponseRequest>,
) -> Response {
    authorize_or_return!(state, headers);
    let run_id = run_id_or_return!(run_id);
    if request.tool_use_id.trim().is_empty() {
        return error_response(GatewayError::new(GatewayDiagnostic::new(
            "approval_id_missing",
            "Approval response is missing tool_use_id.",
            "Include the pending tool_use_id from the run events.",
        )));
    }

    match state.runner.approve_run(
        &run_id,
        &request.tool_use_id,
        request.approved,
        request.reason.as_deref(),
        state.sink.as_ref(),
    ) {
        Ok(submission) => {
            let status = status_for_submission(&submission);
            (status, Json(create_run_body(submission))).into_response()
        }
        Err(error) => error_response(error),
    }
}

async fn ask_user_response(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(request): Json<AskUserResponseRequest>,
) -> Response {
    authorize_or_return!(state, headers);
    let run_id = run_id_or_return!(run_id);
    if request.question_id.trim().is_empty() {
        return error_response(GatewayError::new(GatewayDiagnostic::new(
            "question_id_missing",
            "Ask-user response is missing question_id.",
            "Include the pending question_id from the run events.",
        )));
    }

    match state.runner.answer_user(
        &run_id,
        &request.question_id,
        &request.response,
        state.sink.as_ref(),
    ) {
        Ok(submission) => {
            let status = status_for_submission(&submission);
            (status, Json(create_run_body(submission))).into_response()
        }
        Err(error) => error_response(error),
    }
}

async fn list_adapters(State(state): State<GatewayApiState>, headers: HeaderMap) -> Response {
    authorize_or_return!(state, headers);

    Json(json!({ "adapters": adapter_registry(&state.config).statuses() })).into_response()
}

async fn connect_adapter(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(provider): Path<String>,
) -> Response {
    authorize_or_return!(state, headers);

    let provider = match AdapterProvider::parse(&provider) {
        Ok(provider) => provider,
        Err(error) => return error_response(error),
    };
    match adapter_registry(&state.config).connect(provider) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

async fn test_adapter_message(
    State(state): State<GatewayApiState>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Json(message): Json<AdapterTestMessage>,
) -> Response {
    authorize_or_return!(state, headers);

    let provider = match AdapterProvider::parse(&provider) {
        Ok(provider) => provider,
        Err(error) => return error_response(error),
    };
    match adapter_registry(&state.config).test_message(provider, message) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalResponseRequest {
    #[serde(alias = "tool_use_id")]
    tool_use_id: String,
    approved: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskUserResponseRequest {
    #[serde(alias = "question_id")]
    question_id: String,
    response: String,
}

fn capabilities_body(state: &GatewayApiState) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "authMode": state.auth_mode,
        "transports": ["http"],
        "busyPolicies": ["queue", "reject", "interrupt", "steer"],
        "supportsSteer": state.policy.supports_steer,
        "maxPayloadBytes": state.config.limits.max_payload_bytes,
        "maxRunning": state.policy.max_running,
        "maxQueued": state.policy.max_queued,
        "endpoints": [
            CAPABILITIES_PATH,
            RUNS_PATH,
            RUN_PATH,
            RUN_EVENTS_PATH,
            RUN_STOP_PATH,
            RUN_APPROVAL_PATH,
            RUN_ASK_USER_PATH,
            ADAPTERS_PATH,
            ADAPTER_CONNECT_PATH,
            ADAPTER_TEST_PATH,
        ],
    })
}

fn create_run_body(submission: GatewayRunSubmission) -> Value {
    let mut body = json!({
        "runId": submission.meta.run_id,
        "sessionKey": submission.meta.session_key,
        "status": submission.meta.status,
        "action": submission.action,
        "eventsUrl": format!("/remote-control/v1/runs/{}/events", submission.meta.run_id),
    });
    if let Some(command) = submission.command {
        body["command"] = serde_json::to_value(command).unwrap_or(Value::Null);
    }
    if let Some(diagnostic) = submission.diagnostic {
        body["diagnostic"] = serde_json::to_value(diagnostic).unwrap_or(Value::Null);
    }
    body
}

fn status_for_submission(submission: &GatewayRunSubmission) -> StatusCode {
    match submission.action {
        GatewayRunAction::Started | GatewayRunAction::Queued | GatewayRunAction::Interrupted => {
            StatusCode::ACCEPTED
        }
        GatewayRunAction::Existing => StatusCode::OK,
        GatewayRunAction::Rejected => StatusCode::CONFLICT,
    }
}

fn run_body(meta: RunMeta) -> Value {
    json!({
        "runId": meta.run_id,
        "sessionKey": meta.session_key,
        "status": meta.status,
        "request": meta.request,
        "createdAtMs": meta.created_at_ms,
        "updatedAtMs": meta.updated_at_ms,
    })
}

fn adapter_registry(config: &GatewayConfig) -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(TelegramAdapter::new(config.adapters.telegram.clone()));
    registry.register(LarkAdapter::new(config.adapters.lark.clone()));
    registry
}

pub fn error_response(error: GatewayError) -> Response {
    let diagnostic = error.into_diagnostic();
    let status = status_for_diagnostic(&diagnostic);
    (
        status,
        Json(json!({
            "error": diagnostic,
        })),
    )
        .into_response()
}

fn status_for_diagnostic(diagnostic: &GatewayDiagnostic) -> StatusCode {
    match diagnostic.code.as_str() {
        "missing_control_token" | "invalid_control_token" => StatusCode::UNAUTHORIZED,
        "invalid_run_id" => StatusCode::BAD_REQUEST,
        "run_not_found" => StatusCode::NOT_FOUND,
        "approval_id_missing"
        | "question_id_missing"
        | "adapter_unsupported"
        | "telegram_target_missing"
        | "lark_target_missing" => StatusCode::BAD_REQUEST,
        "busy" | "stale_response" => StatusCode::CONFLICT,
        "telegram_target_blocked" | "lark_target_blocked" => StatusCode::FORBIDDEN,
        "queue_full" => StatusCode::TOO_MANY_REQUESTS,
        "unsupported" | "telegram_transport_unavailable" | "lark_transport_unavailable" => {
            StatusCode::NOT_IMPLEMENTED
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
