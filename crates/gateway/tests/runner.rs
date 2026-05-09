use gateway::{
    BusyPolicy, BusySnapshot, GatewayCommand, GatewayCommandReceipt, GatewayCommandSink,
    GatewayError, GatewayPersistence, GatewayPolicy, GatewayRunAction, GatewayRunner, GatewayStore,
    RemoteSource, RemoteTransport, RunPolicy, RunRequest, RunStatus, SessionKeyPolicy,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

fn request(prompt: &str, busy: BusyPolicy, idempotency_key: Option<&str>) -> RunRequest {
    RunRequest {
        prompt: prompt.to_string(),
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
        GatewayPolicy {
            max_running: 1,
            max_queued: 1,
            ..GatewayPolicy::default()
        },
    )
}

#[test]
fn queued_run_survives_restart_recovery_and_starts_later() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());
    let queued = runner
        .submit_run(
            request("queued", BusyPolicy::Queue, None),
            BusySnapshot {
                running: 1,
                queued: 0,
                max_running: 1,
                max_queued: 1,
            },
            &sink,
        )
        .unwrap();
    assert_eq!(queued.action, GatewayRunAction::Queued);

    let report = runner.recover_on_startup(Duration::from_secs(60)).unwrap();
    assert_eq!(report.queued, 1);

    let started = runner
        .start_next_queued(BusySnapshot::default(), &sink)
        .unwrap()
        .expect("queued run should start when capacity is available");
    assert_eq!(started.action, GatewayRunAction::Started);
    assert_eq!(started.meta.status, RunStatus::Running);
    assert_eq!(sink.commands().len(), 1);
}

#[test]
fn bounded_queue_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());

    let rejected = runner
        .submit_run(
            request("queue full", BusyPolicy::Queue, None),
            BusySnapshot {
                running: 1,
                queued: 1,
                max_running: 1,
                max_queued: 1,
            },
            &sink,
        )
        .unwrap();

    assert_eq!(rejected.action, GatewayRunAction::Rejected);
    assert_eq!(rejected.meta.status, RunStatus::Failed);
    assert_eq!(rejected.diagnostic.unwrap().code, "queue_full");
    assert!(sink.commands().is_empty());
}

#[test]
fn duplicate_idempotency_does_not_dispatch_again_after_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = RecordingSink::default();
    let runner = runner(tmp.path());

    let first = runner
        .submit_run(
            request("same", BusyPolicy::Queue, Some("delivery-1")),
            BusySnapshot::default(),
            &sink,
        )
        .unwrap();
    let report = runner.recover_on_startup(Duration::from_secs(60)).unwrap();
    assert_eq!(report.recoverable, 1);

    let duplicate = runner
        .submit_run(
            request("same", BusyPolicy::Queue, Some("delivery-1")),
            BusySnapshot::default(),
            &sink,
        )
        .unwrap();

    assert_eq!(first.meta.run_id, duplicate.meta.run_id);
    assert_eq!(duplicate.action, GatewayRunAction::Existing);
    assert_eq!(sink.commands().len(), 1);
}
