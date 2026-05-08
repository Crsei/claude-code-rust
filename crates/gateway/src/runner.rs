use crate::{
    BusyDecision, BusySnapshot, GatewayDiagnostic, GatewayError, GatewayPolicy, GatewayStore,
    RunEvent, RunEventKind, RunMeta, RunRequest, RunStatus, SessionKeyPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Gateway-neutral command names for the daemon integration layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCommandKind {
    Submit,
    Abort,
    PermissionResponse,
    AskUserResponse,
}

/// Command envelope emitted by the gateway runner.
///
/// The gateway crate deliberately keeps this daemon-neutral. The daemon bridge
/// is responsible for mapping the envelope onto worker ids and local protocol
/// files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCommand {
    pub kind: GatewayCommandKind,
    pub run_id: String,
    pub session_key: String,
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl GatewayCommand {
    pub fn submit(meta: &RunMeta, prompt: &str) -> Self {
        Self {
            kind: GatewayCommandKind::Submit,
            run_id: meta.run_id.to_string(),
            session_key: meta.session_key.to_string(),
            payload: json!({ "text": prompt }),
            idempotency_key: meta.request.idempotency_key.clone(),
        }
    }

    pub fn abort(meta: &RunMeta, reason: &str) -> Self {
        Self {
            kind: GatewayCommandKind::Abort,
            run_id: meta.run_id.to_string(),
            session_key: meta.session_key.to_string(),
            payload: json!({ "reason": reason }),
            idempotency_key: None,
        }
    }
}

/// A daemon or test bridge that can accept gateway commands.
pub trait GatewayCommandSink {
    fn dispatch(&self, command: GatewayCommand) -> Result<GatewayCommandReceipt, GatewayError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCommandReceipt {
    pub command_id: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayRunAction {
    Started,
    Queued,
    Existing,
    Rejected,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRunSubmission {
    pub meta: RunMeta,
    pub action: GatewayRunAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<GatewayCommandReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<GatewayDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct GatewayRunner {
    store: GatewayStore,
    policy: GatewayPolicy,
}

impl GatewayRunner {
    pub fn new(store: GatewayStore, policy: GatewayPolicy) -> Self {
        Self { store, policy }
    }

    pub fn default_with_policy(session_policy: SessionKeyPolicy, policy: GatewayPolicy) -> Self {
        Self::new(GatewayStore::default_with_policy(session_policy), policy)
    }

    pub fn submit_run(
        &self,
        request: RunRequest,
        snapshot: BusySnapshot,
        sink: &impl GatewayCommandSink,
    ) -> Result<GatewayRunSubmission, GatewayError> {
        let outcome = self.store.create_run(request)?;
        let meta = outcome.meta().clone();
        if matches!(outcome, crate::CreateRunOutcome::Existing(_)) {
            return Ok(GatewayRunSubmission {
                meta,
                action: GatewayRunAction::Existing,
                command: None,
                diagnostic: None,
            });
        }

        match self
            .policy
            .evaluate_busy(meta.request.policy.busy, snapshot)?
        {
            BusyDecision::StartNow => self.start_run(meta, sink),
            BusyDecision::Queue => Ok(GatewayRunSubmission {
                meta,
                action: GatewayRunAction::Queued,
                command: None,
                diagnostic: None,
            }),
            BusyDecision::Interrupt => self.interrupt_then_start(meta, sink),
            BusyDecision::Reject(diagnostic) => {
                let meta = self.fail_created_run(meta, diagnostic.clone())?;
                Ok(GatewayRunSubmission {
                    meta,
                    action: GatewayRunAction::Rejected,
                    command: None,
                    diagnostic: Some(diagnostic),
                })
            }
        }
    }

    pub fn stop_run(
        &self,
        run_id: &crate::RunId,
        sink: &impl GatewayCommandSink,
    ) -> Result<GatewayRunSubmission, GatewayError> {
        let meta = self.store.load_run(run_id)?;
        if meta.status.is_terminal() {
            let diagnostic = GatewayDiagnostic::new(
                "run_already_terminal",
                "The requested run is already terminal.",
                "Reload the run before sending another stop request.",
            )
            .with_context(format!("run_id={}", meta.run_id));
            return Ok(GatewayRunSubmission {
                meta,
                action: GatewayRunAction::Rejected,
                command: None,
                diagnostic: Some(diagnostic),
            });
        }

        let receipt = sink.dispatch(GatewayCommand::abort(&meta, "gateway stop request"))?;
        let meta = self.store.update_status(run_id, RunStatus::Cancelled)?;
        Ok(GatewayRunSubmission {
            meta,
            action: GatewayRunAction::Interrupted,
            command: Some(receipt),
            diagnostic: None,
        })
    }

    fn start_run(
        &self,
        meta: RunMeta,
        sink: &impl GatewayCommandSink,
    ) -> Result<GatewayRunSubmission, GatewayError> {
        let prompt = meta.request.prompt.clone();
        let receipt = sink.dispatch(GatewayCommand::submit(&meta, &prompt))?;
        let meta = self.store.update_status(&meta.run_id, RunStatus::Running)?;
        Ok(GatewayRunSubmission {
            meta,
            action: GatewayRunAction::Started,
            command: Some(receipt),
            diagnostic: None,
        })
    }

    fn interrupt_then_start(
        &self,
        meta: RunMeta,
        sink: &impl GatewayCommandSink,
    ) -> Result<GatewayRunSubmission, GatewayError> {
        let _ = sink.dispatch(GatewayCommand::abort(&meta, "gateway interrupt policy"))?;
        let prompt = meta.request.prompt.clone();
        let receipt = sink.dispatch(GatewayCommand::submit(&meta, &prompt))?;
        let meta = self.store.update_status(&meta.run_id, RunStatus::Running)?;
        Ok(GatewayRunSubmission {
            meta,
            action: GatewayRunAction::Interrupted,
            command: Some(receipt),
            diagnostic: None,
        })
    }

    fn fail_created_run(
        &self,
        meta: RunMeta,
        diagnostic: GatewayDiagnostic,
    ) -> Result<RunMeta, GatewayError> {
        self.store.append_event(&RunEvent::new(
            meta.run_id.clone(),
            self.store.read_events(&meta.run_id)?.len() as u64 + 1,
            RunEventKind::Diagnostic { diagnostic },
        ))?;
        self.store.update_status(&meta.run_id, RunStatus::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BusyPolicy, GatewayPersistence, RemoteSource, RemoteTransport, RunPolicy, SessionKeyPolicy,
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

    #[test]
    fn runner_dispatches_submit_and_marks_running() {
        let tmp = tempfile::tempdir().unwrap();
        let sink = RecordingSink::default();
        let result = runner(tmp.path())
            .submit_run(
                request(BusyPolicy::Queue, None),
                BusySnapshot::default(),
                &sink,
            )
            .unwrap();

        assert_eq!(result.action, GatewayRunAction::Started);
        assert_eq!(result.meta.status, RunStatus::Running);
        assert_eq!(sink.commands().len(), 1);
        assert_eq!(sink.commands()[0].kind, GatewayCommandKind::Submit);
        assert_eq!(sink.commands()[0].payload["text"], "run from gateway");
    }

    #[test]
    fn runner_returns_existing_run_without_duplicate_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let sink = RecordingSink::default();
        let runner = runner(tmp.path());

        let first = runner
            .submit_run(
                request(BusyPolicy::Queue, Some("delivery-1")),
                BusySnapshot::default(),
                &sink,
            )
            .unwrap();
        let second = runner
            .submit_run(
                request(BusyPolicy::Queue, Some("delivery-1")),
                BusySnapshot::default(),
                &sink,
            )
            .unwrap();

        assert_eq!(first.meta.run_id, second.meta.run_id);
        assert_eq!(second.action, GatewayRunAction::Existing);
        assert_eq!(sink.commands().len(), 1);
    }

    #[test]
    fn runner_rejects_busy_with_stable_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        let sink = RecordingSink::default();
        let result = runner(tmp.path())
            .submit_run(
                request(BusyPolicy::Reject, None),
                BusySnapshot {
                    running: 1,
                    ..BusySnapshot::default()
                },
                &sink,
            )
            .unwrap();

        assert_eq!(result.action, GatewayRunAction::Rejected);
        assert_eq!(result.meta.status, RunStatus::Failed);
        assert_eq!(result.diagnostic.unwrap().code, "busy");
        assert!(sink.commands().is_empty());
    }
}
