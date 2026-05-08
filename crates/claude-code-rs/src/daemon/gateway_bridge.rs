//! Bridge between the remote-control gateway runner and daemon worker protocol.
//!
//! The gateway crate owns run policy and durable run metadata. This module is
//! the daemon-side adapter that maps gateway-neutral commands onto existing
//! worker command files without making `gateway` depend on `claude-code-rs`.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use gateway::{
    GatewayCommand, GatewayCommandKind, GatewayCommandReceipt, GatewayCommandSink,
    GatewayDiagnostic, GatewayError,
};
use serde_json::{json, Value};
use tokio_stream::StreamExt;

use crate::engine::lifecycle::QueryEngine;
use crate::types::config::{QueryEngineConfig, QuerySource};

use super::protocol::{self, DaemonCommandKind};
use super::routes::sdk_message_to_sse;
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

pub async fn handle_worker_command(
    worker_id: &str,
    runtime: &mut AssistantWorkerRuntime,
    command: protocol::DaemonCommand,
) -> Result<bool> {
    match command.kind {
        protocol::DaemonCommandKind::Submit => {
            if let Err(err) = runtime.execute_submit(worker_id, &command).await {
                protocol::append_event(
                    worker_id,
                    Some(&command.command_id),
                    "command_failed",
                    json!({
                        "kind": "submit",
                        "error": err.to_string(),
                    }),
                )?;
                protocol::mark_command_failed(command, err.to_string())?;
            } else {
                protocol::mark_command_handled(command)?;
            }
            Ok(false)
        }
        protocol::DaemonCommandKind::Abort => {
            runtime.abort();
            let command = protocol::mark_command_handled(command)?;
            protocol::append_event(
                worker_id,
                Some(&command.command_id),
                "abort_ack",
                json!({ "handled": true }),
            )?;
            Ok(false)
        }
        protocol::DaemonCommandKind::PermissionResponse
        | protocol::DaemonCommandKind::AskUserResponse
        | protocol::DaemonCommandKind::ReloadConfig => {
            let command = protocol::mark_command_handled(command)?;
            protocol::append_event(
                worker_id,
                Some(&command.command_id),
                "command_handled",
                json!({ "kind": command.kind.as_str() }),
            )?;
            Ok(false)
        }
        protocol::DaemonCommandKind::Shutdown => {
            let command = protocol::mark_command_handled(command)?;
            protocol::append_event(
                worker_id,
                Some(&command.command_id),
                "worker_shutdown_ack",
                json!({ "handled": true }),
            )?;
            Ok(true)
        }
    }
}

pub struct AssistantWorkerRuntime {
    engine: Arc<QueryEngine>,
}

impl AssistantWorkerRuntime {
    pub fn new(cwd: &Path) -> Self {
        crate::plugins::init_plugins();
        let tools = crate::tools::registry::get_tools_for_active_session();
        crate::tools::tool_search::install_runtime_tool_catalog(&tools);
        let mut engine = QueryEngine::new(QueryEngineConfig {
            cwd: cwd.to_string_lossy().into_owned(),
            tools,
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: None,
            fallback_model: None,
            max_turns: None,
            max_budget_usd: None,
            task_budget: None,
            verbose: false,
            initial_messages: None,
            commands: crate::commands::get_all_commands()
                .iter()
                .map(|command| command.name.clone())
                .collect(),
            thinking_config: None,
            json_schema: None,
            replay_user_messages: false,
            persist_session: true,
            resolved_model: None,
            auto_save_session: true,
            agent_context: None,
        });
        engine.set_hook_runner(Arc::new(crate::tools::hooks::ShellHookRunner::new()));
        engine.set_command_dispatcher(Arc::new(crate::commands::DefaultCommandDispatcher::new()));

        Self {
            engine: Arc::new(engine),
        }
    }

    async fn execute_submit(
        &self,
        worker_id: &str,
        command: &protocol::DaemonCommand,
    ) -> Result<()> {
        let text = command
            .payload
            .get("text")
            .and_then(|value| value.as_str())
            .context("submit command payload missing text")?;
        let message_id = command
            .payload
            .get("message_id")
            .and_then(|value| value.as_str())
            .unwrap_or(&command.command_id);

        protocol::append_event(
            worker_id,
            Some(&command.command_id),
            "submit_started",
            json!({
                "message_id": message_id,
                "source": command.payload.get("source").cloned().unwrap_or_else(|| json!("worker")),
                "gateway": command.payload.get("gateway").cloned(),
            }),
        )?;

        self.engine.wake_up();
        let stream = self
            .engine
            .submit_message(text, QuerySource::ReplMainThread);
        tokio::pin!(stream);
        while let Some(sdk_msg) = stream.next().await {
            if let Some(event) = sdk_message_to_sse(&sdk_msg, message_id) {
                protocol::append_event(
                    worker_id,
                    Some(&command.command_id),
                    &event.event_type,
                    event.data,
                )?;
            }
        }

        protocol::append_event(
            worker_id,
            Some(&command.command_id),
            "submit_completed",
            json!({ "message_id": message_id }),
        )?;
        Ok(())
    }

    fn abort(&self) {
        self.engine.abort();
    }
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
