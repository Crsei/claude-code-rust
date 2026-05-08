use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

use crate::auth::{extract_gateway_token, GatewayAuthMode, GatewayAuthVerifier};
use crate::{
    BusySnapshot, GatewayCommandSink, GatewayConfig, GatewayDiagnostic, GatewayError,
    GatewayPolicy, GatewayRunAction, GatewayRunSubmission, GatewayRunner, GatewayStore, RunId,
    RunMeta, RunRequest, SessionKeyPolicy,
};

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

#[derive(Debug, Clone)]
pub struct GatewayApiState<S, A, B> {
    runner: GatewayRunner,
    sink: S,
    auth: A,
    snapshot: B,
    policy: GatewayPolicy,
    config: GatewayConfig,
}

impl<S, A, B> GatewayApiState<S, A, B> {
    pub fn new(
        runner: GatewayRunner,
        sink: S,
        auth: A,
        snapshot: B,
        policy: GatewayPolicy,
        config: GatewayConfig,
    ) -> Self {
        Self {
            runner,
            sink,
            auth,
            snapshot,
            policy,
            config,
        }
    }
}

impl<S, A, B> GatewayApiState<S, A, B>
where
    S: GatewayCommandSink,
{
    pub fn with_default_runner(sink: S, auth: A, snapshot: B) -> Self {
        let config = GatewayConfig::default();
        let policy = GatewayPolicy::default();
        let runner = GatewayRunner::new(
            GatewayStore::new(config.persistence.clone(), SessionKeyPolicy::default()),
            policy.clone(),
        );
        Self::new(runner, sink, auth, snapshot, policy, config)
    }
}

pub fn router<S, A, B>(state: GatewayApiState<S, A, B>) -> Router
where
    S: GatewayCommandSink + Clone + Send + Sync + 'static,
    A: GatewayAuthVerifier,
    B: GatewayBusySnapshotProvider,
{
    Router::new()
        .route(
            "/remote-control/v1/capabilities",
            get(capabilities::<S, A, B>),
        )
        .route("/remote-control/v1/runs", post(create_run::<S, A, B>))
        .route("/remote-control/v1/runs/{run_id}", get(get_run::<S, A, B>))
        .with_state(state)
}

async fn capabilities<S, A, B>(
    State(state): State<GatewayApiState<S, A, B>>,
    headers: HeaderMap,
) -> Response
where
    S: GatewayCommandSink + Clone + Send + Sync + 'static,
    A: GatewayAuthVerifier,
    B: GatewayBusySnapshotProvider,
{
    if let Err(error) = authorize(&state.auth, &headers) {
        return error_response(error);
    }
    Json(capabilities_body(&state)).into_response()
}

async fn create_run<S, A, B>(
    State(state): State<GatewayApiState<S, A, B>>,
    headers: HeaderMap,
    Json(request): Json<RunRequest>,
) -> Response
where
    S: GatewayCommandSink + Clone + Send + Sync + 'static,
    A: GatewayAuthVerifier,
    B: GatewayBusySnapshotProvider,
{
    if let Err(error) = authorize(&state.auth, &headers) {
        return error_response(error);
    }

    match state
        .runner
        .submit_run(request, state.snapshot.snapshot(), &state.sink)
    {
        Ok(submission) => {
            let status = match submission.action {
                GatewayRunAction::Started
                | GatewayRunAction::Queued
                | GatewayRunAction::Interrupted => StatusCode::ACCEPTED,
                GatewayRunAction::Existing => StatusCode::OK,
                GatewayRunAction::Rejected => StatusCode::CONFLICT,
            };
            (status, Json(create_run_body(submission))).into_response()
        }
        Err(error) => error_response(error),
    }
}

async fn get_run<S, A, B>(
    State(state): State<GatewayApiState<S, A, B>>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response
where
    S: GatewayCommandSink + Clone + Send + Sync + 'static,
    A: GatewayAuthVerifier,
    B: GatewayBusySnapshotProvider,
{
    if let Err(error) = authorize(&state.auth, &headers) {
        return error_response(error);
    }

    let run_id = match RunId::from_string(run_id) {
        Ok(run_id) => run_id,
        Err(error) => return error_response(error),
    };
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

fn authorize<A>(auth: &A, headers: &HeaderMap) -> Result<(), GatewayError>
where
    A: GatewayAuthVerifier,
{
    auth.verify(extract_gateway_token(headers))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesResponse {
    pub version: &'static str,
    pub auth_mode: GatewayAuthMode,
    pub transports: Vec<&'static str>,
    pub busy_policies: Vec<&'static str>,
    pub supports_steer: bool,
    pub max_payload_bytes: usize,
    pub max_running: usize,
    pub max_queued: usize,
    pub endpoints: Vec<&'static str>,
}

fn capabilities_body<S, A, B>(state: &GatewayApiState<S, A, B>) -> CapabilitiesResponse
where
    A: GatewayAuthVerifier,
{
    CapabilitiesResponse {
        version: env!("CARGO_PKG_VERSION"),
        auth_mode: state.auth.mode(),
        transports: vec!["http"],
        busy_policies: vec!["queue", "reject", "interrupt", "steer"],
        supports_steer: state.policy.supports_steer,
        max_payload_bytes: state.config.limits.max_payload_bytes,
        max_running: state.policy.max_running,
        max_queued: state.policy.max_queued,
        endpoints: vec![
            "/remote-control/v1/capabilities",
            "/remote-control/v1/runs",
            "/remote-control/v1/runs/{run_id}",
        ],
    }
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
        "busy" | "stale_response" => StatusCode::CONFLICT,
        "queue_full" => StatusCode::TOO_MANY_REQUESTS,
        "unsupported" => StatusCode::NOT_IMPLEMENTED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AllowAllGatewayAuth;
    use crate::{
        BusyPolicy, GatewayCommand, GatewayCommandReceipt, GatewayPersistence, RemoteSource,
        RemoteTransport, RunPolicy,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingSink {
        commands: Arc<Mutex<Vec<GatewayCommand>>>,
    }

    impl RecordingSink {
        fn commands(&self) -> Vec<GatewayCommand> {
            self.commands.lock().unwrap().clone()
        }
    }

    impl GatewayCommandSink for RecordingSink {
        fn dispatch(&self, command: GatewayCommand) -> Result<GatewayCommandReceipt, GatewayError> {
            self.commands.lock().unwrap().push(command);
            Ok(GatewayCommandReceipt {
                command_id: "cmd-1".to_string(),
                target: "assistant-session-1".to_string(),
            })
        }
    }

    fn persistence(root: &std::path::Path) -> GatewayPersistence {
        GatewayPersistence {
            gateway_dir: root.join("gateway"),
            runs_dir: root.join("gateway").join("runs"),
            adapters_dir: root.join("gateway").join("adapters"),
            webhooks_dir: root.join("gateway").join("webhooks"),
        }
    }

    fn state(
        root: &std::path::Path,
    ) -> GatewayApiState<RecordingSink, AllowAllGatewayAuth, StaticBusySnapshotProvider> {
        let policy = GatewayPolicy::default();
        let config = GatewayConfig {
            persistence: persistence(root),
            ..GatewayConfig::default()
        };
        GatewayApiState::new(
            GatewayRunner::new(
                GatewayStore::new(config.persistence.clone(), SessionKeyPolicy::default()),
                policy.clone(),
            ),
            RecordingSink::default(),
            AllowAllGatewayAuth,
            StaticBusySnapshotProvider::default(),
            policy,
            config,
        )
    }

    fn request() -> RunRequest {
        RunRequest {
            prompt: "hello".to_string(),
            source: RemoteSource::new(
                RemoteTransport::Http,
                "local",
                "F:/AIclassmanager/cc/rust",
                "dashboard",
                "86186",
                "thread-1",
            ),
            policy: RunPolicy {
                busy: BusyPolicy::Queue,
                ..RunPolicy::default()
            },
            idempotency_key: Some("delivery-1".to_string()),
        }
    }

    #[test]
    fn capabilities_include_phase4a_endpoints() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        let body = capabilities_body(&state);

        assert!(body.endpoints.contains(&"/remote-control/v1/capabilities"));
        assert!(body.endpoints.contains(&"/remote-control/v1/runs"));
        assert!(!body.supports_steer);
    }

    #[test]
    fn create_run_response_shape_has_events_url() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        let submission = state
            .runner
            .submit_run(request(), state.snapshot.snapshot(), &state.sink)
            .unwrap();
        let body = create_run_body(submission);

        assert_eq!(body["status"], "running");
        assert!(body["eventsUrl"].as_str().unwrap().contains("/events"));
        assert_eq!(state.sink.commands().len(), 1);
    }

    #[test]
    fn run_body_exposes_redacted_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        let meta = state
            .runner
            .submit_run(request(), state.snapshot.snapshot(), &state.sink)
            .unwrap()
            .meta;

        let body = run_body(meta);
        assert_eq!(body["status"], "running");
        assert!(body["runId"].as_str().unwrap().starts_with("run_"));
    }

    #[test]
    fn diagnostic_status_mapping_is_stable() {
        assert_eq!(
            status_for_diagnostic(&GatewayDiagnostic::new("queue_full", "", "")),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            status_for_diagnostic(&GatewayDiagnostic::new("run_not_found", "", "")),
            StatusCode::NOT_FOUND
        );
    }
}
