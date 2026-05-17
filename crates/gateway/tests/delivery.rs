use gateway::{
    AdapterProvider, BusyPolicy, BusySnapshot, CallbackDeliverySink, ChannelDeliverySink,
    DeliveryPayload, DeliveryRouter, DeliveryStatus, GatewayCommand, GatewayCommandReceipt,
    GatewayCommandSink, GatewayDiagnostic, GatewayError, GatewayPersistence, GatewayPolicy,
    GatewayRunner, GatewayStore, RemoteSource, RemoteTransport, RunEvent, RunEventKind, RunPolicy,
    RunRequest, RunStatus, SessionKeyPolicy,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingCommandSink;

impl GatewayCommandSink for RecordingCommandSink {
    fn dispatch(&self, command: GatewayCommand) -> Result<GatewayCommandReceipt, GatewayError> {
        Ok(GatewayCommandReceipt {
            command_id: format!("cmd-{}", command.run_id),
            target: command.session_key,
        })
    }
}

#[derive(Clone, Default)]
struct RecordingCallbackSink {
    attempts: Arc<Mutex<Vec<String>>>,
    fail: bool,
}

impl RecordingCallbackSink {
    fn failing() -> Self {
        Self {
            attempts: Arc::default(),
            fail: true,
        }
    }

    fn attempts(&self) -> Vec<String> {
        self.attempts.lock().unwrap().clone()
    }
}

impl CallbackDeliverySink for RecordingCallbackSink {
    fn deliver_callback(&self, url: &str, _payload: &DeliveryPayload) -> Result<(), GatewayError> {
        self.attempts.lock().unwrap().push(url.to_string());
        if self.fail {
            return Err(GatewayError::new(GatewayDiagnostic::new(
                "retry_exhausted",
                "Callback delivery failed after retry.",
                "Inspect the callback endpoint and retry manually.",
            )));
        }
        Ok(())
    }
}

type RecordedChannelDeliveries = Arc<Mutex<Vec<(AdapterProvider, String, Option<String>)>>>;

#[derive(Clone, Default)]
struct RecordingChannelSink {
    deliveries: RecordedChannelDeliveries,
    fail: bool,
}

impl RecordingChannelSink {
    fn failing() -> Self {
        Self {
            deliveries: Arc::default(),
            fail: true,
        }
    }
}

impl ChannelDeliverySink for RecordingChannelSink {
    fn deliver_channel(
        &self,
        provider: AdapterProvider,
        target: &str,
        thread: Option<&str>,
        _payload: &DeliveryPayload,
    ) -> Result<(), GatewayError> {
        self.deliveries.lock().unwrap().push((
            provider,
            target.to_string(),
            thread.map(ToOwned::to_owned),
        ));
        if self.fail {
            return Err(GatewayError::new(GatewayDiagnostic::new(
                "delivery_unsupported",
                "Channel delivery is not configured.",
                "Connect the provider before sending channel results.",
            )));
        }
        Ok(())
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

fn request(delivery: Vec<&str>) -> RunRequest {
    RunRequest {
        prompt: "deliver this result".to_string(),
        source: RemoteSource::new(
            RemoteTransport::Webhook,
            "local",
            "F:/AIclassmanager/cc/rust",
            "github",
            "86186",
            "thread-1",
        ),
        policy: RunPolicy {
            busy: BusyPolicy::Queue,
            permission_mode: "ask".to_string(),
            delivery: delivery.into_iter().map(str::to_string).collect(),
        },
        idempotency_key: None,
    }
}

fn runner(root: &std::path::Path) -> GatewayRunner {
    GatewayRunner::new(
        GatewayStore::new(persistence(root), SessionKeyPolicy::default()),
        GatewayPolicy::default(),
    )
}

fn completion_event(run_id: gateway::RunId) -> RunEvent {
    RunEvent::new(
        run_id,
        99,
        RunEventKind::Custom {
            name: "completed".to_string(),
            payload: json!({ "summary": "done" }),
        },
    )
}

#[test]
fn local_delivery_writes_delivery_log_without_changing_completion() {
    let tmp = tempfile::tempdir().unwrap();
    let runner = runner(tmp.path());
    let created = runner
        .submit_run(
            request(vec!["local"]),
            BusySnapshot::default(),
            &RecordingCommandSink,
        )
        .unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    store
        .update_status(&created.meta.run_id, RunStatus::Completed)
        .unwrap();

    let records = runner
        .deliver_event(
            &created.meta.run_id,
            &completion_event(created.meta.run_id.clone()),
            &RecordingCallbackSink::default(),
            &RecordingChannelSink::default(),
        )
        .unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, DeliveryStatus::Delivered);
    let meta = store.load_run(&created.meta.run_id).unwrap();
    assert_eq!(meta.status, RunStatus::Completed);
    let delivery_log =
        std::fs::read_to_string(store.run_dir(&created.meta.run_id).join("delivery.ndjson"))
            .unwrap();
    assert!(delivery_log.contains("\"local\""));
}

#[test]
fn callback_delivery_retries_and_records_failure_event() {
    let tmp = tempfile::tempdir().unwrap();
    let runner = runner(tmp.path());
    let created = runner
        .submit_run(
            request(vec!["callback:https://example.com/hook"]),
            BusySnapshot::default(),
            &RecordingCommandSink,
        )
        .unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    store
        .update_status(&created.meta.run_id, RunStatus::Completed)
        .unwrap();
    let callback = RecordingCallbackSink::failing();

    let records = runner
        .deliver_event(
            &created.meta.run_id,
            &completion_event(created.meta.run_id.clone()),
            &callback,
            &RecordingChannelSink::default(),
        )
        .unwrap();

    assert_eq!(callback.attempts().len(), 3);
    assert_eq!(records[0].status, DeliveryStatus::Failed);
    let events = store.read_events(&created.meta.run_id).unwrap();
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, RunEventKind::DeliveryFailed { .. })));
    assert_eq!(
        store.load_run(&created.meta.run_id).unwrap().status,
        RunStatus::Completed
    );
}

#[test]
fn callback_ssrf_guard_rejects_private_and_non_loopback_http_targets() {
    for target in [
        "callback:http://10.0.0.5/hook",
        "callback:https://169.254.169.254/latest/meta-data",
        "callback:https://localhost/hook",
    ] {
        let err = DeliveryRouter::parse_targets(&[target.to_string()]).unwrap_err();
        assert_eq!(err.diagnostic().code, "callback_blocked");
        assert!(!serde_json::to_string(err.diagnostic())
            .unwrap()
            .contains("Authorization"));
    }

    assert!(DeliveryRouter::parse_targets(&["callback:http://127.0.0.1/hook".to_string()]).is_ok());
}

#[test]
fn channel_delivery_failure_records_delivery_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let runner = runner(tmp.path());
    let created = runner
        .submit_run(
            request(vec!["channel:telegram:chat-1:thread-1"]),
            BusySnapshot::default(),
            &RecordingCommandSink,
        )
        .unwrap();
    let store = GatewayStore::new(persistence(tmp.path()), SessionKeyPolicy::default());
    store
        .update_status(&created.meta.run_id, RunStatus::Completed)
        .unwrap();

    let records = runner
        .deliver_event(
            &created.meta.run_id,
            &completion_event(created.meta.run_id.clone()),
            &RecordingCallbackSink::default(),
            &RecordingChannelSink::failing(),
        )
        .unwrap();

    assert_eq!(
        records[0].diagnostic.as_ref().unwrap().code,
        "delivery_unsupported"
    );
    let delivery_log =
        std::fs::read_to_string(store.run_dir(&created.meta.run_id).join("delivery.ndjson"))
            .unwrap();
    assert!(delivery_log.contains("delivery_unsupported"));
}
