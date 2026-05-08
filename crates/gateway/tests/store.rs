use gateway::{
    BusyPolicy, CreateRunOutcome, GatewayPersistence, GatewayStore, RemoteSource, RemoteTransport,
    RunEvent, RunEventKind, RunPolicy, RunRequest, RunStatus, SessionKeyPolicy,
};
use std::sync::{Arc, Barrier};

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
