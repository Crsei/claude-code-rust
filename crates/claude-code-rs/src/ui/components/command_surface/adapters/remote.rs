use std::fs;

use gateway::{GatewayConfig, GatewayPersistence, GatewayStore, RunId, SessionKeyPolicy};

use crate::daemon::process_state::{self, DaemonStatusSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteSurfaceSnapshot {
    pub(crate) daemon: RemoteDaemonRow,
    pub(crate) adapters: Vec<RemoteAdapterRow>,
    pub(crate) runs: Vec<RemoteRunRow>,
    pub(crate) security: Vec<RemoteSecurityRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteDaemonRow {
    pub(crate) state: String,
    pub(crate) detail: String,
    pub(crate) action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteAdapterRow {
    pub(crate) provider: String,
    pub(crate) state: String,
    pub(crate) action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteRunRow {
    pub(crate) run_id: String,
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) updated_at_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteSecurityRow {
    pub(crate) label: String,
    pub(crate) value: String,
}

pub(crate) fn remote_surface_snapshot(limit: usize) -> RemoteSurfaceSnapshot {
    RemoteSurfaceSnapshot {
        daemon: daemon_row(),
        adapters: adapter_rows(),
        runs: run_rows(limit),
        security: security_rows(),
    }
}

fn daemon_row() -> RemoteDaemonRow {
    match process_state::status_snapshot() {
        Ok(DaemonStatusSnapshot::Running(state)) => RemoteDaemonRow {
            state: "running".to_string(),
            detail: format!(
                "pid={}; base={}",
                state.pid,
                redact_inline(&format!("http://127.0.0.1:{}", state.port))
            ),
            action: "/remote status".to_string(),
        },
        Ok(DaemonStatusSnapshot::Stale(state)) => RemoteDaemonRow {
            state: "stale".to_string(),
            detail: format!("pid={}; restart daemon", state.pid),
            action: "/remote doctor".to_string(),
        },
        Ok(DaemonStatusSnapshot::Stopped) => RemoteDaemonRow {
            state: "stopped".to_string(),
            detail: "daemon state not running".to_string(),
            action: "/remote status".to_string(),
        },
        Err(error) => RemoteDaemonRow {
            state: "unknown".to_string(),
            detail: redact_inline(&error.to_string()),
            action: "/remote doctor".to_string(),
        },
    }
}

fn adapter_rows() -> Vec<RemoteAdapterRow> {
    ["telegram", "lark"]
        .into_iter()
        .map(|provider| RemoteAdapterRow {
            provider: provider.to_string(),
            state: "status via /remote adapters".to_string(),
            action: format!("/remote connect {provider}"),
        })
        .collect()
}

fn run_rows(limit: usize) -> Vec<RemoteRunRow> {
    list_local_runs(limit)
        .unwrap_or_default()
        .into_iter()
        .map(|meta| RemoteRunRow {
            run_id: meta.run_id.to_string(),
            status: format!("{:?}", meta.status).to_ascii_lowercase(),
            source: format!(
                "{}:{}",
                meta.request.source.transport.as_key_part(),
                truncate_middle(&meta.request.source.thread_id, 24)
            ),
            updated_at_ms: meta.updated_at_ms,
        })
        .collect()
}

fn security_rows() -> Vec<RemoteSecurityRow> {
    let config = GatewayConfig::default();
    vec![
        RemoteSecurityRow {
            label: "auth".to_string(),
            value: "local daemon token required; raw value hidden".to_string(),
        },
        RemoteSecurityRow {
            label: "gateway dir".to_string(),
            value: config.persistence.gateway_dir.display().to_string(),
        },
        RemoteSecurityRow {
            label: "runs dir".to_string(),
            value: config.persistence.runs_dir.display().to_string(),
        },
        RemoteSecurityRow {
            label: "adapter dir".to_string(),
            value: config.persistence.adapters_dir.display().to_string(),
        },
    ]
}

fn list_local_runs(limit: usize) -> std::io::Result<Vec<gateway::RunMeta>> {
    let persistence = GatewayPersistence::default();
    let runs_dir = persistence.runs_dir.clone();
    if !runs_dir.exists() {
        return Ok(Vec::new());
    }

    let store = GatewayStore::new(persistence, SessionKeyPolicy::default());
    let mut runs = Vec::new();
    for entry in fs::read_dir(&runs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(run_id) = RunId::from_string(name) else {
            continue;
        };
        if let Ok(meta) = store.load_run(&run_id) {
            runs.push(meta);
        }
    }
    runs.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    runs.truncate(limit);
    Ok(runs)
}

pub(crate) fn truncate_middle(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    if max_len <= 3 {
        return "...".to_string();
    }
    let head = (max_len - 3) / 2;
    let tail = max_len - 3 - head;
    format!("{}...{}", &value[..head], &value[value.len() - tail..])
}

fn redact_inline(value: &str) -> String {
    let mut out = String::new();
    for part in value.split_whitespace() {
        let lower = part.to_ascii_lowercase();
        let redacted = lower.contains("authorization")
            || lower.starts_with("bearer")
            || lower.contains("token")
            || lower.contains("secret")
            || lower.contains("credential")
            || lower.contains("signature");
        if !out.is_empty() {
            out.push(' ');
        }
        if redacted {
            out.push_str("<redacted>");
        } else {
            out.push_str(part);
        }
    }
    out
}
