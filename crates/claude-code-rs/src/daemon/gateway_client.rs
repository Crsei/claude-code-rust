//! Local client for the loopback remote-control gateway.
//!
//! The client intentionally talks to the daemon's loopback HTTP API for
//! mutating operations. Read-only run listing can inspect the gateway store so
//! `/remote runs` remains useful when the daemon is stopped.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use gateway::{
    AdapterProvider, AdapterStatus, AdapterTestMessage, GatewayConfig, GatewayDiagnostic,
    GatewayPersistence, GatewayStore, RunEvent, RunId, RunMeta, SessionKeyPolicy,
};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::daemon::process_state::{self, DaemonStatusSnapshot};

const TOKEN_HEADER: &str = "x-cc-rust-daemon-token";

#[derive(Debug, Clone)]
pub enum LocalGatewayDaemonStatus {
    Running {
        pid: u32,
        base_url: String,
        health_url: String,
    },
    Stale {
        pid: u32,
    },
    Stopped,
}

#[derive(Debug, Clone)]
pub struct LocalGatewayClient {
    base_url: String,
    token: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCapabilitiesSnapshot {
    pub version: String,
    pub auth_mode: Value,
    pub supports_steer: bool,
    pub max_running: usize,
    pub max_queued: usize,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRunActionResponse {
    pub run_id: RunId,
    pub status: gateway::RunStatus,
    pub action: Value,
    #[serde(default)]
    pub diagnostic: Option<GatewayDiagnostic>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRunEvents {
    pub events: Vec<RunEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAdapters {
    pub adapters: Vec<AdapterStatus>,
}

impl LocalGatewayClient {
    pub fn daemon_status() -> Result<LocalGatewayDaemonStatus> {
        Ok(match process_state::status_snapshot()? {
            DaemonStatusSnapshot::Running(state) => LocalGatewayDaemonStatus::Running {
                pid: state.pid,
                base_url: base_url_for_port(state.port),
                health_url: state.health_url,
            },
            DaemonStatusSnapshot::Stale(state) => {
                LocalGatewayDaemonStatus::Stale { pid: state.pid }
            }
            DaemonStatusSnapshot::Stopped => LocalGatewayDaemonStatus::Stopped,
        })
    }

    pub fn from_running_daemon() -> Result<Self, GatewayDiagnostic> {
        let status = Self::daemon_status().map_err(|error| {
            diagnostic(
                "daemon_state_unavailable",
                "The daemon state could not be read.",
                "Check the daemon state file and retry /remote.",
            )
            .with_context(redact_text(&error.to_string()))
        })?;

        let LocalGatewayDaemonStatus::Running { base_url, .. } = status else {
            return Err(match status {
                LocalGatewayDaemonStatus::Stale { pid } => diagnostic(
                    "daemon_stale",
                    "The daemon state is stale.",
                    "Restart the daemon with `claude daemon restart`.",
                )
                .with_context(format!("pid={pid}")),
                LocalGatewayDaemonStatus::Stopped => diagnostic(
                    "daemon_stopped",
                    "The daemon is not running.",
                    "Start it with `FEATURE_KAIROS=1 claude daemon start`.",
                ),
                LocalGatewayDaemonStatus::Running { .. } => unreachable!(),
            });
        };

        let token = process_state::read_control_token()
            .map_err(|error| {
                diagnostic(
                    "control_token_unavailable",
                    "The daemon control token could not be read.",
                    "Check daemon token file permissions.",
                )
                .with_context(redact_text(&error.to_string()))
            })?
            .map(|token| token.token)
            .ok_or_else(|| {
                diagnostic(
                    "control_token_missing",
                    "The daemon control token is missing.",
                    "Restart the daemon so it can publish a fresh local control token.",
                )
            })?;

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| {
                diagnostic(
                    "gateway_client_unavailable",
                    "The local gateway HTTP client could not be created.",
                    "Retry the command or inspect the local TLS/network configuration.",
                )
                .with_context(redact_text(&error.to_string()))
            })?;

        Ok(Self {
            base_url,
            token,
            http,
        })
    }

    pub async fn capabilities(&self) -> Result<GatewayCapabilitiesSnapshot, GatewayDiagnostic> {
        self.get_json("/remote-control/v1/capabilities").await
    }

    pub async fn adapters(&self) -> Result<Vec<AdapterStatus>, GatewayDiagnostic> {
        Ok(self
            .get_json::<GatewayAdapters>("/remote-control/v1/adapters")
            .await?
            .adapters)
    }

    pub async fn connect_adapter(
        &self,
        provider: AdapterProvider,
    ) -> Result<AdapterStatus, GatewayDiagnostic> {
        self.post_json(
            &format!("/remote-control/v1/adapters/{}/connect", provider.as_str()),
            &json!({}),
        )
        .await
    }

    pub async fn test_adapter_message(
        &self,
        provider: AdapterProvider,
        target: String,
        text: String,
    ) -> Result<AdapterStatus, GatewayDiagnostic> {
        self.post_json(
            &format!(
                "/remote-control/v1/adapters/{}/test-message",
                provider.as_str()
            ),
            &AdapterTestMessage { target, text },
        )
        .await
    }

    pub async fn show_run(&self, run_id: &RunId) -> Result<RunMeta, GatewayDiagnostic> {
        self.get_json(&format!("/remote-control/v1/runs/{run_id}"))
            .await
    }

    pub async fn run_events(&self, run_id: &RunId) -> Result<Vec<RunEvent>, GatewayDiagnostic> {
        Ok(self
            .get_json::<GatewayRunEvents>(&format!("/remote-control/v1/runs/{run_id}/events"))
            .await?
            .events)
    }

    pub async fn stop_run(
        &self,
        run_id: &RunId,
    ) -> Result<GatewayRunActionResponse, GatewayDiagnostic> {
        self.post_json(
            &format!("/remote-control/v1/runs/{run_id}/stop"),
            &json!({}),
        )
        .await
    }

    async fn get_json<T>(&self, path: &str) -> Result<T, GatewayDiagnostic>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .get(self.url(path))
            .header(TOKEN_HEADER, &self.token)
            .send()
            .await
            .map_err(http_transport_diagnostic)?;
        decode_response(response).await
    }

    async fn post_json<B, T>(&self, path: &str, body: &B) -> Result<T, GatewayDiagnostic>
    where
        B: serde::Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let response = self
            .http
            .post(self.url(path))
            .header(TOKEN_HEADER, &self.token)
            .json(body)
            .send()
            .await
            .map_err(http_transport_diagnostic)?;
        decode_response(response).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

pub fn list_local_runs(limit: usize) -> Result<Vec<RunMeta>> {
    let persistence = GatewayPersistence::default();
    let runs_dir = persistence.runs_dir.clone();
    if !runs_dir.exists() {
        return Ok(Vec::new());
    }

    let store = GatewayStore::new(persistence, SessionKeyPolicy::default());
    let mut runs = Vec::new();
    for entry in
        fs::read_dir(&runs_dir).with_context(|| format!("failed to read {}", runs_dir.display()))?
    {
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

pub fn load_local_run(run_id: &RunId) -> Result<RunMeta, GatewayDiagnostic> {
    GatewayStore::new(GatewayPersistence::default(), SessionKeyPolicy::default())
        .load_run(run_id)
        .map_err(|error| error.into_diagnostic())
}

pub fn read_local_events(run_id: &RunId) -> Result<Vec<RunEvent>, GatewayDiagnostic> {
    GatewayStore::new(GatewayPersistence::default(), SessionKeyPolicy::default())
        .read_events(run_id)
        .map_err(|error| error.into_diagnostic())
}

pub fn gateway_paths() -> GatewayPaths {
    let config = GatewayConfig::default();
    GatewayPaths {
        gateway_dir: config.persistence.gateway_dir,
        runs_dir: config.persistence.runs_dir,
        adapters_dir: config.persistence.adapters_dir,
        config_path: GatewayConfig::default_config_path(),
        token_path: process_state::control_token_path(),
    }
}

#[derive(Debug, Clone)]
pub struct GatewayPaths {
    pub gateway_dir: PathBuf,
    pub runs_dir: PathBuf,
    pub adapters_dir: PathBuf,
    pub config_path: PathBuf,
    pub token_path: PathBuf,
}

async fn decode_response<T>(response: reqwest::Response) -> Result<T, GatewayDiagnostic>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let text = response.text().await.map_err(http_transport_diagnostic)?;
    if status.is_success() {
        return serde_json::from_str(&text).map_err(|error| {
            diagnostic(
                "gateway_response_decode_failed",
                "The gateway response could not be decoded.",
                "Retry the command or inspect the daemon gateway route output.",
            )
            .with_context(redact_text(&error.to_string()))
        });
    }

    if let Ok(envelope) = serde_json::from_str::<GatewayErrorEnvelope>(&text) {
        return Err(envelope.error);
    }

    Err(diagnostic(
        "gateway_http_error",
        "The gateway returned an unexpected HTTP error.",
        "Retry the command or inspect daemon logs.",
    )
    .with_context(format!(
        "status={}, body={}",
        status_code(status),
        redact_text(&text)
    )))
}

#[derive(Debug, Deserialize)]
struct GatewayErrorEnvelope {
    error: GatewayDiagnostic,
}

fn http_transport_diagnostic(error: reqwest::Error) -> GatewayDiagnostic {
    diagnostic(
        "gateway_unreachable",
        "The local gateway HTTP API could not be reached.",
        "Verify the daemon is running and listening on loopback.",
    )
    .with_context(redact_text(&error.to_string()))
}

fn diagnostic(code: &str, message: &str, action: &str) -> GatewayDiagnostic {
    GatewayDiagnostic::new(code, message, action)
}

fn base_url_for_port(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

fn status_code(status: StatusCode) -> u16 {
    status.as_u16()
}

pub fn redact_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
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
