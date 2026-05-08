//! Filesystem command/event protocol for daemon supervisor workers.
//!
//! Commands are durable JSON files under `~/.cc-rust/daemon/commands/<worker>/`.
//! Workers only process `pending` commands, so an acknowledged command is not
//! repeated after a restart. Events are appended as NDJSON per worker for the
//! control plane to replay in later phases.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::process_state;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCommandKind {
    Submit,
    Abort,
    PermissionResponse,
    AskUserResponse,
    Shutdown,
    ReloadConfig,
}

impl DaemonCommandKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Abort => "abort",
            Self::PermissionResponse => "permission_response",
            Self::AskUserResponse => "ask_user_response",
            Self::Shutdown => "shutdown",
            Self::ReloadConfig => "reload_config",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCommandStatus {
    Pending,
    Acked,
    Handled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonCommand {
    pub schema_version: u32,
    pub command_id: String,
    pub idempotency_key: Option<String>,
    pub target_worker_id: String,
    pub kind: DaemonCommandKind,
    pub payload: Value,
    pub status: DaemonCommandStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub acked_at: Option<DateTime<Utc>>,
    pub handled_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub worker_id: String,
    pub command_id: Option<String>,
    pub event_type: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandProcessingResult {
    pub inspected: usize,
    pub acked: usize,
    pub handled: usize,
    pub shutdown_requested: bool,
}

pub fn commands_dir() -> PathBuf {
    process_state::daemon_dir().join("commands")
}

pub fn worker_commands_dir(worker_id: &str) -> PathBuf {
    commands_dir().join(sanitize_path_component(worker_id))
}

pub fn command_path(worker_id: &str, command_id: &str) -> PathBuf {
    worker_commands_dir(worker_id).join(format!("{}.json", sanitize_path_component(command_id)))
}

pub fn events_dir() -> PathBuf {
    process_state::daemon_dir().join("events")
}

pub fn worker_events_path(worker_id: &str) -> PathBuf {
    events_dir().join(format!("{}.ndjson", sanitize_path_component(worker_id)))
}

pub fn enqueue_command(
    target_worker_id: &str,
    kind: DaemonCommandKind,
    payload: Value,
    idempotency_key: Option<String>,
) -> Result<DaemonCommand> {
    if let Some(key) = idempotency_key.as_deref() {
        if let Some(existing) = find_command_by_idempotency_key(target_worker_id, key)? {
            return Ok(existing);
        }
    }

    let now = Utc::now();
    let command = DaemonCommand {
        schema_version: SCHEMA_VERSION,
        command_id: next_id("cmd"),
        idempotency_key,
        target_worker_id: target_worker_id.to_string(),
        kind,
        payload,
        status: DaemonCommandStatus::Pending,
        created_at: now,
        updated_at: now,
        acked_at: None,
        handled_at: None,
        error: None,
    };
    write_command(&command)?;
    Ok(command)
}

pub fn read_worker_commands(worker_id: &str) -> Result<Vec<DaemonCommand>> {
    let dir = worker_commands_dir(worker_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut commands = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        commands.push(read_command_file(&path)?);
    }
    commands.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.command_id.cmp(&right.command_id))
    });
    Ok(commands)
}

pub fn read_command(worker_id: &str, command_id: &str) -> Result<Option<DaemonCommand>> {
    let path = command_path(worker_id, command_id);
    if !path.exists() {
        return Ok(None);
    }
    read_command_file(&path).map(Some)
}

pub fn process_pending_commands(
    worker_id: &str,
    worker_kind: &str,
) -> Result<CommandProcessingResult> {
    let commands = read_worker_commands(worker_id)?;
    let mut result = CommandProcessingResult {
        inspected: commands.len(),
        acked: 0,
        handled: 0,
        shutdown_requested: false,
    };

    for command in commands {
        if command.status != DaemonCommandStatus::Pending {
            continue;
        }

        let mut command = transition_command(command, DaemonCommandStatus::Acked, None);
        command.acked_at = Some(Utc::now());
        write_command(&command)?;
        result.acked += 1;
        append_event(
            worker_id,
            Some(&command.command_id),
            "command_ack",
            json!({
                "kind": command.kind.as_str(),
                "worker_kind": worker_kind,
            }),
        )?;

        match command.kind {
            DaemonCommandKind::Submit => {
                let gateway_run_id = command
                    .payload
                    .get("gateway")
                    .and_then(|gateway| gateway.get("runId"))
                    .cloned();
                append_event(
                    worker_id,
                    Some(&command.command_id),
                    "command_deferred",
                    json!({
                        "kind": "submit",
                        "gateway_run_id": gateway_run_id,
                        "reason": "assistant execution remains in the HTTP supervisor until Phase 4",
                    }),
                )?;
            }
            DaemonCommandKind::Abort => {
                let command = mark_handled(command)?;
                result.handled += 1;
                append_event(
                    worker_id,
                    Some(&command.command_id),
                    "abort_ack",
                    json!({ "handled": true }),
                )?;
            }
            DaemonCommandKind::PermissionResponse
            | DaemonCommandKind::AskUserResponse
            | DaemonCommandKind::ReloadConfig => {
                let command = mark_handled(command)?;
                result.handled += 1;
                append_event(
                    worker_id,
                    Some(&command.command_id),
                    "command_handled",
                    json!({ "kind": command.kind.as_str() }),
                )?;
            }
            DaemonCommandKind::Shutdown => {
                let command = mark_handled(command)?;
                result.handled += 1;
                result.shutdown_requested = true;
                append_event(
                    worker_id,
                    Some(&command.command_id),
                    "worker_shutdown_ack",
                    json!({ "handled": true }),
                )?;
            }
        }
    }

    Ok(result)
}

pub fn append_event(
    worker_id: &str,
    command_id: Option<&str>,
    event_type: &str,
    data: Value,
) -> Result<DaemonEvent> {
    let event = DaemonEvent {
        schema_version: SCHEMA_VERSION,
        event_id: next_id("evt"),
        worker_id: worker_id.to_string(),
        command_id: command_id.map(ToOwned::to_owned),
        event_type: event_type.to_string(),
        data,
        created_at: Utc::now(),
    };
    let path = worker_events_path(worker_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open daemon event log {}", path.display()))?;
    let mut bytes = serde_json::to_vec(&event)?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .with_context(|| format!("failed to write daemon event log {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to sync daemon event log {}", path.display()))?;
    Ok(event)
}

pub fn read_worker_events(worker_id: &str) -> Result<Vec<DaemonEvent>> {
    let path = worker_events_path(worker_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read daemon event log {}", path.display()))?;
    let mut events = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse daemon event log {} line {}",
                path.display(),
                idx + 1
            )
        })?;
        events.push(event);
    }
    Ok(events)
}

fn mark_handled(command: DaemonCommand) -> Result<DaemonCommand> {
    let mut command = transition_command(command, DaemonCommandStatus::Handled, None);
    command.handled_at = Some(Utc::now());
    write_command(&command)?;
    Ok(command)
}

fn transition_command(
    mut command: DaemonCommand,
    status: DaemonCommandStatus,
    error: Option<String>,
) -> DaemonCommand {
    command.status = status;
    command.updated_at = Utc::now();
    command.error = error;
    command
}

fn write_command(command: &DaemonCommand) -> Result<()> {
    process_state::atomic_write_json(
        &command_path(&command.target_worker_id, &command.command_id),
        command,
    )
}

fn read_command_file(path: &Path) -> Result<DaemonCommand> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read daemon command {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse daemon command {}", path.display()))
}

fn find_command_by_idempotency_key(
    worker_id: &str,
    idempotency_key: &str,
) -> Result<Option<DaemonCommand>> {
    Ok(read_worker_commands(worker_id)?
        .into_iter()
        .find(|command| command.idempotency_key.as_deref() == Some(idempotency_key)))
}

fn next_id(prefix: &str) -> String {
    let now = Utc::now();
    let nanos = now
        .timestamp_nanos_opt()
        .unwrap_or_else(|| now.timestamp_micros() * 1_000);
    format!("{prefix}-{nanos}-{}", std::process::id())
}

fn sanitize_path_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const WORKER_ID: &str = "assistant-session-1";

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
    fn enqueue_command_deduplicates_idempotency_key() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());

        let first = enqueue_command(
            WORKER_ID,
            DaemonCommandKind::Submit,
            json!({ "text": "hello" }),
            Some("same-key".to_string()),
        )
        .unwrap();
        let second = enqueue_command(
            WORKER_ID,
            DaemonCommandKind::Submit,
            json!({ "text": "hello again" }),
            Some("same-key".to_string()),
        )
        .unwrap();

        assert_eq!(first.command_id, second.command_id);
        assert_eq!(read_worker_commands(WORKER_ID).unwrap().len(), 1);
    }

    #[test]
    #[serial]
    fn process_pending_commands_acks_submit_once() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let command = enqueue_command(
            WORKER_ID,
            DaemonCommandKind::Submit,
            json!({ "text": "hello" }),
            None,
        )
        .unwrap();

        let first = process_pending_commands(WORKER_ID, "assistant-session").unwrap();
        let second = process_pending_commands(WORKER_ID, "assistant-session").unwrap();
        let stored = read_command(WORKER_ID, &command.command_id)
            .unwrap()
            .unwrap();
        let events = read_worker_events(WORKER_ID).unwrap();

        assert_eq!(first.acked, 1);
        assert_eq!(second.acked, 0);
        assert_eq!(stored.status, DaemonCommandStatus::Acked);
        assert!(events.iter().any(|event| event.event_type == "command_ack"));
    }

    #[test]
    #[serial]
    fn abort_command_is_handled_and_emits_event() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let command =
            enqueue_command(WORKER_ID, DaemonCommandKind::Abort, json!({}), None).unwrap();

        let result = process_pending_commands(WORKER_ID, "assistant-session").unwrap();
        let stored = read_command(WORKER_ID, &command.command_id)
            .unwrap()
            .unwrap();
        let events = read_worker_events(WORKER_ID).unwrap();

        assert_eq!(result.acked, 1);
        assert_eq!(result.handled, 1);
        assert_eq!(stored.status, DaemonCommandStatus::Handled);
        assert!(events.iter().any(|event| event.event_type == "abort_ack"));
    }

    #[test]
    fn submit_payload_can_carry_optional_gateway_context() {
        let command = DaemonCommand {
            schema_version: SCHEMA_VERSION,
            command_id: "cmd-1".to_string(),
            idempotency_key: None,
            target_worker_id: WORKER_ID.to_string(),
            kind: DaemonCommandKind::Submit,
            payload: json!({ "text": "hello", "gateway": { "runId": "run_abc123" } }),
            status: DaemonCommandStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            acked_at: None,
            handled_at: None,
            error: None,
        };

        assert_eq!(command.payload["gateway"]["runId"], "run_abc123");
    }
}
