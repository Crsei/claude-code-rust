//! Bridge between the remote-control gateway runner and daemon worker protocol.
//!
//! The gateway crate owns run policy and durable run metadata. This module is
//! the daemon-side adapter that maps gateway-neutral commands onto existing
//! worker command files without making `gateway` depend on `claude-code-rs`.

#![allow(dead_code)]

use gateway::{
    GatewayCommand, GatewayCommandKind, GatewayCommandReceipt, GatewayCommandSink,
    GatewayDiagnostic, GatewayError,
};
use serde_json::{json, Value};

use super::protocol::{self, DaemonCommandKind};
use super::supervisor::ASSISTANT_WORKER_ID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayDaemonBridge {
    target_worker_id: String,
}

impl GatewayDaemonBridge {
    pub fn assistant_worker() -> Self {
        Self {
            target_worker_id: ASSISTANT_WORKER_ID.to_string(),
        }
    }

    pub fn for_worker(target_worker_id: impl Into<String>) -> Self {
        Self {
            target_worker_id: target_worker_id.into(),
        }
    }

    pub fn target_worker_id(&self) -> &str {
        &self.target_worker_id
    }
}

impl GatewayCommandSink for GatewayDaemonBridge {
    fn dispatch(&self, command: GatewayCommand) -> Result<GatewayCommandReceipt, GatewayError> {
        let kind = daemon_kind(command.kind);
        let payload = daemon_payload(&command);
        let queued = protocol::enqueue_command(
            &self.target_worker_id,
            kind,
            payload,
            command.idempotency_key.clone(),
        )
        .map_err(|error| enqueue_error(&command, error))?;

        Ok(GatewayCommandReceipt {
            command_id: queued.command_id,
            target: queued.target_worker_id,
        })
    }
}

fn daemon_kind(kind: GatewayCommandKind) -> DaemonCommandKind {
    match kind {
        GatewayCommandKind::Submit => DaemonCommandKind::Submit,
        GatewayCommandKind::Abort => DaemonCommandKind::Abort,
        GatewayCommandKind::PermissionResponse => DaemonCommandKind::PermissionResponse,
        GatewayCommandKind::AskUserResponse => DaemonCommandKind::AskUserResponse,
    }
}

fn daemon_payload(command: &GatewayCommand) -> Value {
    match command.kind {
        GatewayCommandKind::Submit => {
            let text = command
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            json!({
                "text": text,
                "idempotencyKey": command.idempotency_key.clone(),
                "gateway": gateway_context(command),
            })
        }
        GatewayCommandKind::Abort
        | GatewayCommandKind::PermissionResponse
        | GatewayCommandKind::AskUserResponse => {
            let mut payload = command.payload.clone();
            if let Some(object) = payload.as_object_mut() {
                object.insert("gateway".to_string(), gateway_context(command));
                payload
            } else {
                json!({
                    "value": payload,
                    "gateway": gateway_context(command),
                })
            }
        }
    }
}

fn gateway_context(command: &GatewayCommand) -> Value {
    json!({
        "runId": command.run_id.clone(),
        "sessionKey": command.session_key.clone(),
    })
}

fn enqueue_error(command: &GatewayCommand, error: anyhow::Error) -> GatewayError {
    GatewayError::new(
        GatewayDiagnostic::new(
            "daemon_command_enqueue_failed",
            "The daemon bridge could not enqueue the gateway command.",
            "Check daemon command directory permissions and worker state.",
        )
        .with_context(format!(
            "run_id={}, kind={:?}, error={:#}",
            command.run_id, command.kind, error
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::path::Path;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    #[serial]
    fn bridge_enqueues_submit_with_gateway_context() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let bridge = GatewayDaemonBridge::for_worker("assistant-session-1");

        let receipt = bridge
            .dispatch(GatewayCommand {
                kind: GatewayCommandKind::Submit,
                run_id: "run_bridge123".to_string(),
                session_key: "remote:http:abc".to_string(),
                payload: json!({ "text": "hello" }),
                idempotency_key: Some("delivery-1".to_string()),
            })
            .unwrap();

        let command = protocol::read_command("assistant-session-1", &receipt.command_id)
            .unwrap()
            .unwrap();
        assert_eq!(command.payload["text"], "hello");
        assert_eq!(command.payload["idempotencyKey"], "delivery-1");
        assert_eq!(command.payload["gateway"]["runId"], "run_bridge123");
    }
}
