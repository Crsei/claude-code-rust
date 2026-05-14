//! Stable daemon worker command/event file contracts.
//!
//! The full daemon runtime still lives in the root binary during Phase 9. This
//! module owns the reusable wire DTOs and pure format helpers so migrations can
//! verify command JSON files and worker event NDJSON without depending on root
//! runtime modules.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;

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

pub fn command_file_name(command_id: &str) -> String {
    format!("{}.json", sanitize_path_component(command_id))
}

pub fn worker_event_file_name(worker_id: &str) -> String {
    format!("{}.ndjson", sanitize_path_component(worker_id))
}

pub fn event_to_ndjson_line(event: &DaemonEvent) -> serde_json::Result<String> {
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    Ok(line)
}

pub fn events_from_ndjson(text: &str) -> serde_json::Result<Vec<DaemonEvent>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect()
}

pub fn sanitize_path_component(raw: &str) -> String {
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
    use chrono::TimeZone;
    use serde_json::json;

    fn ts(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).single().unwrap()
    }

    #[test]
    fn worker_command_json_matches_root_file_contract() {
        let command = DaemonCommand {
            schema_version: SCHEMA_VERSION,
            command_id: "cmd-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            target_worker_id: "assistant-session-1".to_string(),
            kind: DaemonCommandKind::PermissionResponse,
            payload: json!({
                "decision": "allow",
                "toolUseId": "toolu_1",
                "gateway": { "runId": "run_abc123" }
            }),
            status: DaemonCommandStatus::Acked,
            created_at: ts(1_700_000_000),
            updated_at: ts(1_700_000_010),
            acked_at: Some(ts(1_700_000_005)),
            handled_at: None,
            error: None,
        };

        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            json!({
                "schema_version": 1,
                "command_id": "cmd-1",
                "idempotency_key": "idem-1",
                "target_worker_id": "assistant-session-1",
                "kind": "permission_response",
                "payload": {
                    "decision": "allow",
                    "toolUseId": "toolu_1",
                    "gateway": { "runId": "run_abc123" }
                },
                "status": "acked",
                "created_at": "2023-11-14T22:13:20Z",
                "updated_at": "2023-11-14T22:13:30Z",
                "acked_at": "2023-11-14T22:13:25Z",
                "handled_at": null,
                "error": null
            })
        );
    }

    #[test]
    fn worker_command_json_from_root_contract_deserializes() {
        let command: DaemonCommand = serde_json::from_value(json!({
            "schema_version": 1,
            "command_id": "cmd-2",
            "idempotency_key": null,
            "target_worker_id": "worker/sub",
            "kind": "ask_user_response",
            "payload": { "answer": "yes" },
            "status": "failed",
            "created_at": "2023-11-14T22:13:20Z",
            "updated_at": "2023-11-14T22:13:30Z",
            "acked_at": null,
            "handled_at": "2023-11-14T22:13:30Z",
            "error": "cancelled"
        }))
        .unwrap();

        assert_eq!(command.kind, DaemonCommandKind::AskUserResponse);
        assert_eq!(command.status, DaemonCommandStatus::Failed);
        assert_eq!(command.payload["answer"], "yes");
    }

    #[test]
    fn worker_event_ndjson_matches_root_file_contract() {
        let first = DaemonEvent {
            schema_version: SCHEMA_VERSION,
            event_id: "evt-1".to_string(),
            worker_id: "assistant-session-1".to_string(),
            command_id: Some("cmd-1".to_string()),
            event_type: "command_ack".to_string(),
            data: json!({ "kind": "submit", "worker_kind": "assistant-session" }),
            created_at: ts(1_700_000_000),
        };
        let second = DaemonEvent {
            schema_version: SCHEMA_VERSION,
            event_id: "evt-2".to_string(),
            worker_id: "assistant-session-1".to_string(),
            command_id: None,
            event_type: "worker_heartbeat".to_string(),
            data: json!({}),
            created_at: ts(1_700_000_010),
        };

        let text = format!(
            "{}{}",
            event_to_ndjson_line(&first).unwrap(),
            event_to_ndjson_line(&second).unwrap()
        );
        let events = events_from_ndjson(&text).unwrap();

        assert_eq!(events, vec![first, second]);
        assert!(text.ends_with('\n'));
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn worker_file_names_use_root_sanitization_rules() {
        assert_eq!(command_file_name("cmd/one:two"), "cmd_one_two.json");
        assert_eq!(
            worker_event_file_name("worker one/../two"),
            "worker_one_.._two.ndjson"
        );
    }
}
