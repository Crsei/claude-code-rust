use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use gateway::auth::AllowAllGatewayAuth;
use gateway::{
    AdapterProvider, BusyPolicy, BusySnapshot, GatewayApiState, GatewayCommand, GatewayCommandKind,
    GatewayCommandReceipt, GatewayCommandSink, GatewayConfig, GatewayDiagnostic, GatewayError,
    GatewayPersistence, GatewayPolicy, GatewayRunAction, GatewayRunner, GatewayStore, RemoteSource,
    RemoteTransport, RunEvent, RunEventKind, RunPolicy, RunRequest, RunStatus, SessionKeyPolicy,
    StaticBusySnapshotProvider,
};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

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

fn request(busy: BusyPolicy, idempotency_key: Option<&str>) -> RunRequest {
    RunRequest {
        prompt: "run from gateway".to_string(),
        source: RemoteSource::new(
            RemoteTransport::Http,
            "local",
            "F:/AIclassmanager/cc/rust",
            "dashboard",
            "86186",
            "thread-1",
        ),
        policy: RunPolicy {
            busy,
            ..RunPolicy::default()
        },
        idempotency_key: idempotency_key.map(str::to_string),
    }
}

fn runner(root: &std::path::Path) -> GatewayRunner {
    GatewayRunner::new(
        GatewayStore::new(persistence(root), SessionKeyPolicy::default()),
        GatewayPolicy::default(),
    )
}

fn state(root: &std::path::Path, config: GatewayConfig) -> GatewayApiState {
    GatewayApiState::new(
        runner(root),
        RecordingSink::default(),
        AllowAllGatewayAuth,
        StaticBusySnapshotProvider::default(),
        GatewayPolicy::default(),
        config,
    )
}

#[test]
fn router_wires_phase4b_state() {
    let tmp = tempfile::tempdir().unwrap();
    let policy = GatewayPolicy::default();
    let config = GatewayConfig {
        persistence: persistence(tmp.path()),
        ..GatewayConfig::default()
    };
    let state = GatewayApiState::new(
        runner(tmp.path()),
        RecordingSink::default(),
        AllowAllGatewayAuth,
        StaticBusySnapshotProvider::default(),
        policy,
        config,
    );

    let _router = gateway::api::router(state);
}

#[test]
fn adapter_provider_parse_rejects_unknown_provider() {
    let err = AdapterProvider::parse("slack").unwrap_err();
    assert_eq!(err.diagnostic().code, "adapter_unsupported");
}

#[test]
fn api_error_response_uses_stable_contract_statuses() {
    for (code, status) in [
        ("queue_full", axum::http::StatusCode::TOO_MANY_REQUESTS),
        ("unsupported", axum::http::StatusCode::NOT_IMPLEMENTED),
        ("run_already_terminal", axum::http::StatusCode::CONFLICT),
        (
            "payload_too_large",
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
        ),
        ("bad_origin", axum::http::StatusCode::FORBIDDEN),
    ] {
        let response = gateway::api::error_response(GatewayError::new(GatewayDiagnostic::new(
            code, "message", "action",
        )));
        assert_eq!(response.status(), status);
    }
}

#[tokio::test]
async fn api_create_run_rejects_oversized_payload_with_stable_code() {
    let tmp = tempfile::tempdir().unwrap();
    let config = GatewayConfig {
        persistence: persistence(tmp.path()),
        limits: gateway::GatewayLimits {
            max_payload_bytes: 1,
            ..gateway::GatewayLimits::default()
        },
        ..GatewayConfig::default()
    };
    let app = gateway::api::router(state(tmp.path(), config));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/remote-control/v1/runs")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("payload_too_large"));
}

#[tokio::test]
async fn api_create_run_rejects_bad_origin_before_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let config = GatewayConfig {
        persistence: persistence(tmp.path()),
        security: gateway::GatewaySecurityConfig {
            allowed_origins: vec!["https://trusted.example".to_string()],
            remote_token: None,
        },
        ..GatewayConfig::default()
    };
    let app = gateway::api::router(state(tmp.path(), config));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/remote-control/v1/runs")
                .header("origin", "https://evil.example")
                .body(Body::from(
                    serde_json::to_vec(&request(BusyPolicy::Queue, None)).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("bad_origin"));
}

#[tokio::test]
async fn api_create_run_uses_idempotency_key_header_without_persisting_raw_value() {
    let tmp = tempfile::tempdir().unwrap();
    let config = GatewayConfig {
        persistence: persistence(tmp.path()),
        ..GatewayConfig::default()
    };
    let app = gateway::api::router(state(tmp.path(), config));
    let body = serde_json::to_vec(&request(BusyPolicy::Queue, None)).unwrap();

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/remote-control/v1/runs")
                    .header("idempotency-key", "provider-delivery-id")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    let runs: Vec<_> = std::fs::read_dir(tmp.path().join("gateway").join("runs"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(runs.len(), 1);
    let meta_json = std::fs::read_to_string(runs[0].path().join("meta.json")).unwrap();
    assert!(!meta_json.contains("provider-delivery-id"));
}

#[test]
fn runner_dispatches_run_scoped_approval_response() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let created = runner
        .submit_run(
            request(BusyPolicy::Queue, None),
            BusySnapshot::default(),
            &sink,
        )
        .unwrap();
    store
        .update_status(&created.meta.run_id, RunStatus::WaitingApproval)
        .unwrap();
    store
        .append_event(&RunEvent::new(
            created.meta.run_id.clone(),
            4,
            RunEventKind::ApprovalRequested {
                tool_use_id: "tool-1".to_string(),
            },
        ))
        .unwrap();

    let result = runner
        .approve_run(&created.meta.run_id, "tool-1", true, None, &sink)
        .unwrap();

    assert_eq!(result.meta.status, RunStatus::Running);
    assert_eq!(
        sink.commands().last().unwrap().kind,
        GatewayCommandKind::PermissionResponse
    );
    assert_eq!(
        sink.commands().last().unwrap().payload["toolUseId"],
        "tool-1"
    );
}

#[test]
fn runner_rejects_duplicate_approval_response() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let created = runner
        .submit_run(
            request(BusyPolicy::Queue, None),
            BusySnapshot::default(),
            &sink,
        )
        .unwrap();
    store
        .update_status(&created.meta.run_id, RunStatus::WaitingApproval)
        .unwrap();
    store
        .append_event(&RunEvent::new(
            created.meta.run_id.clone(),
            4,
            RunEventKind::ApprovalRequested {
                tool_use_id: "tool-1".to_string(),
            },
        ))
        .unwrap();

    runner
        .approve_run(&created.meta.run_id, "tool-1", true, None, &sink)
        .unwrap();
    let duplicate = runner
        .approve_run(&created.meta.run_id, "tool-1", true, None, &sink)
        .unwrap();

    assert_eq!(duplicate.action, GatewayRunAction::Rejected);
    assert_eq!(duplicate.diagnostic.unwrap().code, "stale_response");
}

#[test]
fn runner_rejects_stale_ask_user_response() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());
    let created = runner
        .submit_run(
            request(BusyPolicy::Queue, None),
            BusySnapshot::default(),
            &sink,
        )
        .unwrap();

    let result = runner
        .answer_user(&created.meta.run_id, "question-1", "answer", &sink)
        .unwrap();

    assert_eq!(result.action, GatewayRunAction::Rejected);
    assert_eq!(result.diagnostic.unwrap().code, "stale_response");
}
