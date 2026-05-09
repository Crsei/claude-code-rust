use gateway::{
    BusyPolicy, CreateRunOutcome, GatewayPersistence, GatewayStore, RemoteSource, RemoteTransport,
    RunEvent, RunEventKind, RunPolicy, RunRequest, RunStatus, SessionKeyPolicy, SessionLockOutcome,
};
use std::sync::{Arc, Barrier};
use std::time::Duration;

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
        prompt: "Summarize the latest build status.".to_string(),
        source: RemoteSource::new(
            RemoteTransport::Http,
            "local",
            "F:/AIclassmanager/cc/rust",
            "dashboard",
            "86186",
            "thread-1",
        )
        .with_metadata("Authorization", "Bearer raw-token"),
        policy: RunPolicy {
            busy: BusyPolicy::Queue,
            ..RunPolicy::default()
        },
        idempotency_key: idempotency_key.map(str::to_string),
    }
}

#[test]
fn store_creates_run_under_gateway_root() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());

    let outcome = store.create_run(request(Some("provider-1"))).unwrap();
    let CreateRunOutcome::Created(meta) = outcome else {
        panic!("expected a new run");
    };

    let run_dir = tmp
        .path()
        .join("gateway")
        .join("runs")
        .join(meta.run_id.as_str());
    assert!(run_dir.join("meta.json").is_file());
    assert!(run_dir.join("events.ndjson").is_file());
    assert!(tmp.path().join("gateway").join("idempotency").is_dir());

    let meta_json = std::fs::read_to_string(run_dir.join("meta.json")).unwrap();
    assert!(!meta_json.contains("raw-token"));
    assert!(!meta_json.contains("provider-1"));
    assert_eq!(meta.status, RunStatus::Queued);
}

#[test]
fn store_rejects_persistence_paths_outside_gateway_root() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(
        GatewayPersistence {
            gateway_dir: tmp.path().join("gateway"),
            runs_dir: tmp.path().join("outside-runs"),
            adapters_dir: tmp.path().join("gateway").join("adapters"),
            webhooks_dir: tmp.path().join("gateway").join("webhooks"),
        },
        SessionKeyPolicy::default(),
    );

    let err = match store.create_run(request(None)) {
        Ok(_) => panic!("expected path escape rejection"),
        Err(error) => error,
    };
    assert_eq!(err.diagnostic().code, "store_path_escape");
    assert!(err
        .diagnostic()
        .action
        .contains("cc-rust gateway directory"));
}

#[test]
fn duplicate_idempotency_returns_existing_run() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());

    let first = store.create_run(request(Some("provider-1"))).unwrap();
    let second = store.create_run(request(Some("provider-1"))).unwrap();

    assert!(matches!(first, CreateRunOutcome::Created(_)));
    let CreateRunOutcome::Existing(existing) = second else {
        panic!("expected existing run");
    };
    assert_eq!(first.meta().run_id, existing.run_id);
}

#[test]
fn concurrent_duplicate_idempotency_creates_one_run() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(GatewayStore::new(
        persistence(tmp.path()),
        SessionKeyPolicy::default(),
    ));
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.create_run(request(Some("provider-1"))).unwrap()
            })
        })
        .collect();

    let outcomes: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    let first_run_id = outcomes[0].meta().run_id.clone();

    assert!(outcomes
        .iter()
        .any(|outcome| matches!(outcome, CreateRunOutcome::Created(_))));
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.meta().run_id == first_run_id));

    let runs: Vec<_> = std::fs::read_dir(tmp.path().join("gateway").join("runs"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(runs.len(), 1);
}

#[test]
fn store_appends_and_replays_events() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let meta = store.create_run(request(None)).unwrap().meta().clone();

    store
        .update_status(&meta.run_id, RunStatus::Running)
        .unwrap();
    store
        .append_event(&RunEvent::new(
            meta.run_id.clone(),
            3,
            RunEventKind::AssistantDelta {
                text: "hello".to_string(),
            },
        ))
        .unwrap();

    let events = store.read_events(&meta.run_id).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
    assert_eq!(events[2].sequence, 3);
}

#[test]
fn append_event_failure_returns_stable_diagnostic() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let meta = store.create_run(request(None)).unwrap().meta().clone();
    let events_path = store.events_path(&meta.run_id);

    std::fs::remove_file(&events_path).unwrap();
    std::fs::create_dir(&events_path).unwrap();

    let err = store
        .append_event(&RunEvent::new(
            meta.run_id.clone(),
            2,
            RunEventKind::StatusChanged {
                status: RunStatus::Running,
            },
        ))
        .unwrap_err();

    assert_eq!(err.diagnostic().code, "event_append_failed");
    assert!(err.diagnostic().message.contains("event log"));
    assert!(err.diagnostic().action.contains("events.ndjson"));
}

#[test]
fn startup_recovery_marks_running_runs_recoverable_and_requeues_queued_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let queued = store.create_run(request(None)).unwrap().meta().clone();
    let running = store
        .create_run(request(Some("provider-2")))
        .unwrap()
        .meta()
        .clone();
    store
        .update_status(&running.run_id, RunStatus::Running)
        .unwrap();

    let report = store.recover_on_startup(Duration::from_secs(60)).unwrap();

    assert_eq!(report.queued, 1);
    assert_eq!(report.recoverable, 1);
    assert_eq!(
        store.load_run(&queued.run_id).unwrap().status,
        RunStatus::Queued
    );
    assert_eq!(
        store.load_run(&running.run_id).unwrap().status,
        RunStatus::Recoverable
    );
    let running_events = store.read_events(&running.run_id).unwrap();
    assert!(running_events.iter().any(|event| matches!(
        &event.kind,
        RunEventKind::Diagnostic { diagnostic } if diagnostic.code == "run_recovered_after_restart"
    )));
    let queued_events = store.read_events(&queued.run_id).unwrap();
    assert!(queued_events.iter().any(|event| matches!(
        &event.kind,
        RunEventKind::Custom { name, .. } if name == "run_requeued_after_restart"
    )));
}

#[test]
fn startup_recovery_retains_pending_runs_with_expiry_event() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let meta = store.create_run(request(None)).unwrap().meta().clone();
    store
        .update_status(&meta.run_id, RunStatus::Running)
        .unwrap();
    store
        .update_status(&meta.run_id, RunStatus::WaitingApproval)
        .unwrap();

    let report = store.recover_on_startup(Duration::from_secs(30)).unwrap();

    assert_eq!(report.pending, 1);
    assert_eq!(
        store.load_run(&meta.run_id).unwrap().status,
        RunStatus::WaitingApproval
    );
    let events = store.read_events(&meta.run_id).unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RunEventKind::Custom { name, payload }
            if name == "pending_response_retained_after_restart"
                && payload.get("expiresAtMs").is_some()
    )));
}

#[test]
fn stale_session_lock_takeover_writes_audit_event() {
    let tmp = tempfile::tempdir().unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    let meta = store.create_run(request(None)).unwrap().meta().clone();

    assert!(matches!(
        store
            .acquire_session_lock(&meta, 100, Duration::from_secs(60))
            .unwrap(),
        SessionLockOutcome::Acquired(_)
    ));
    assert!(matches!(
        store
            .acquire_session_lock(&meta, 200, Duration::from_secs(60))
            .unwrap(),
        SessionLockOutcome::Busy(_)
    ));

    let lock_path = std::fs::read_dir(tmp.path().join("gateway").join("session-locks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut lock: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&lock_path).unwrap()).unwrap();
    lock["heartbeatMs"] = serde_json::json!(0);
    std::fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();

    assert!(matches!(
        store
            .acquire_session_lock(&meta, 200, Duration::from_millis(1))
            .unwrap(),
        SessionLockOutcome::Recovered(_)
    ));
    let events = store.read_events(&meta.run_id).unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RunEventKind::SessionLockRecovered {
            previous_owner_pid: 100,
            new_owner_pid: 200
        }
    )));
}
