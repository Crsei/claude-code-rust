use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use gateway::{
    BusyPolicy, BusySnapshot, GatewayApiState, GatewayCommand, GatewayCommandReceipt,
    GatewayCommandSink, GatewayConfig, GatewayError, GatewayPersistence, GatewayPolicy,
    GatewayRunner, GatewayStore, RemoteGatewayAuth, RemoteSource, RemoteTransport, RunEventKind,
    RunPolicy, RunRequest, SessionKeyPolicy, StaticBusySnapshotProvider,
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
            command_id: format!("cmd-{}", self.commands.lock().unwrap().len()),
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

fn request(idempotency_key: Option<&str>) -> RunRequest {
    RunRequest {
        prompt: "e2e gateway smoke".to_string(),
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
        idempotency_key: idempotency_key.map(str::to_string),
    }
}

fn app(root: &std::path::Path, sink: RecordingSink, snapshot: BusySnapshot) -> axum::Router {
    let config = GatewayConfig {
        persistence: persistence(root),
        security: gateway::GatewaySecurityConfig {
            allowed_origins: vec!["https://trusted.example".to_string()],
            remote_token: Some("remote-token".to_string()),
        },
        ..GatewayConfig::default()
    };
    let policy = GatewayPolicy::default();
    let runner = GatewayRunner::new(
        GatewayStore::new(config.persistence.clone(), SessionKeyPolicy::default()),
        policy.clone(),
    );
    let state = GatewayApiState::new(
        runner,
        sink,
        RemoteGatewayAuth::new("remote-token").unwrap(),
        StaticBusySnapshotProvider::new(snapshot),
        policy,
        config,
    );
    gateway::api::router(state)
}

#[tokio::test]
async fn e2e_create_run_read_events_and_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let app = app(tmp.path(), sink.clone(), BusySnapshot::default());
    let body = serde_json::to_vec(&request(Some("delivery-1"))).unwrap();

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/remote-control/v1/runs")
                .header("x-cc-rust-remote-token", "remote-token")
                .header("origin", "https://trusted.example")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::ACCEPTED);
    let create_body: serde_json::Value =
        serde_json::from_slice(&to_bytes(create.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    let run_id = create_body["runId"].as_str().unwrap();

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/remote-control/v1/runs/{run_id}/events"))
                .header("x-cc-rust-remote-token", "remote-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_body: serde_json::Value =
        serde_json::from_slice(&to_bytes(events.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert!(events_body["events"].as_array().unwrap().len() >= 2);

    let stop = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/remote-control/v1/runs/{run_id}/stop"))
                .header("x-cc-rust-remote-token", "remote-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop.status(), StatusCode::ACCEPTED);
    let stop_body: serde_json::Value =
        serde_json::from_slice(&to_bytes(stop.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(stop_body["status"], "cancelled");

    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let run_id = gateway::RunId::from_string(run_id).unwrap();
    let events = store.read_events(&run_id).unwrap();
    let cancelled_at = events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                RunEventKind::StatusChanged {
                    status: gateway::RunStatus::Cancelled
                }
            )
        })
        .unwrap();
    assert!(!events[cancelled_at + 1..]
        .iter()
        .any(|event| matches!(&event.kind, RunEventKind::AssistantDelta { .. })));
    assert_eq!(sink.commands().len(), 2);
}

#[tokio::test]
async fn e2e_rejects_bad_token_and_bad_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(
        tmp.path(),
        RecordingSink::default(),
        BusySnapshot::default(),
    );
    let body = serde_json::to_vec(&request(None)).unwrap();

    let bad_token = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/remote-control/v1/runs")
                .header("x-cc-rust-remote-token", "wrong")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_token.status(), StatusCode::UNAUTHORIZED);
    let bad_token_body = to_bytes(bad_token.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&bad_token_body).contains("invalid_remote_token"));

    let bad_origin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/remote-control/v1/runs")
                .header("x-cc-rust-remote-token", "remote-token")
                .header("origin", "https://evil.example")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_origin.status(), StatusCode::FORBIDDEN);
    let bad_origin_body = to_bytes(bad_origin.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&bad_origin_body).contains("bad_origin"));
}

#[tokio::test]
async fn e2e_duplicate_idempotency_returns_existing_run() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let app = app(tmp.path(), sink.clone(), BusySnapshot::default());
    let body = serde_json::to_vec(&request(Some("delivery-1"))).unwrap();
    let mut run_ids = Vec::new();

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/remote-control/v1/runs")
                    .header("x-cc-rust-remote-token", "remote-token")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response.status().is_success());
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        run_ids.push(body["runId"].as_str().unwrap().to_string());
    }

    assert_eq!(run_ids[0], run_ids[1]);
    assert_eq!(sink.commands().len(), 1);
}

#[test]
fn e2e_restart_recovery_keeps_queued_run_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let runner = GatewayRunner::new(store.clone(), GatewayPolicy::default());
    let sink = RecordingSink::default();
    let queued = runner
        .submit_run(
            request(None),
            BusySnapshot {
                running: 1,
                queued: 0,
                max_running: 1,
                max_queued: 32,
            },
            &sink,
        )
        .unwrap();
    assert_eq!(queued.meta.status, gateway::RunStatus::Queued);

    let report = runner
        .recover_on_startup(std::time::Duration::from_secs(60))
        .unwrap();

    assert_eq!(report.queued, 1);
    assert_eq!(
        store.load_run(&queued.meta.run_id).unwrap().status,
        gateway::RunStatus::Queued
    );
}
