use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, info};

static EVENT_LOG_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const DEFAULT_PORT: u16 = 19838;
const HEALTH_CHECK_TIMEOUT_MS: u64 = 5000;
const HEALTH_CHECK_INTERVAL_MS: u64 = 100;

#[derive(Debug, Clone)]
pub struct DashboardConfig {
    pub port: u16,
    pub event_log_path: PathBuf,
    pub auto_open_browser: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            event_log_path: event_log_path(),
            auto_open_browser: std::env::var("FEATURE_SUBAGENT_DASHBOARD_OPEN")
                .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"))
                .unwrap_or(false),
        }
    }
}

pub struct DashboardCompanion {
    child: Child,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentEvent {
    pub ts: String,
    pub kind: String,
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub depth: usize,
    pub background: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl DashboardCompanion {
    pub async fn spawn(config: DashboardConfig) -> Result<Self> {
        let script_path = resolve_dashboard_script_path()?;
        let mut cmd = Command::new("bun");
        cmd.arg("run")
            .arg(&script_path)
            .arg("--port")
            .arg(config.port.to_string())
            .arg("--event-log")
            .arg(&config.event_log_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().with_context(|| {
            format!(
                "failed to spawn dashboard companion: {}",
                script_path.display()
            )
        })?;

        wait_for_health(config.port).await?;

        let url = format!("http://127.0.0.1:{}/", config.port);
        info!(url = %url, "dashboard companion ready");
        if config.auto_open_browser {
            let _ = open_browser(&url);
        }

        Ok(Self { child })
    }

    pub fn kill(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for DashboardCompanion {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn event_log_path() -> PathBuf {
    let base = resolve_base_dir();
    event_log_path_for_base(&base)
}

pub fn emit_subagent_event(
    kind: &str,
    agent_id: &str,
    parent_agent_id: Option<&str>,
    description: Option<&str>,
    model: Option<&str>,
    depth: usize,
    background: bool,
    payload: Option<Value>,
) -> Result<()> {
    if !dashboard_enabled() {
        return Ok(());
    }

    let event = SubagentEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        kind: kind.to_string(),
        agent_id: agent_id.to_string(),
        parent_agent_id: parent_agent_id.map(ToOwned::to_owned),
        description: description.map(ToOwned::to_owned),
        model: model.map(ToOwned::to_owned),
        depth,
        background,
        payload,
    };
    append_event(&event)
}

fn append_event(event: &SubagentEvent) -> Result<()> {
    let path = event_log_path();
    append_event_to_path(&path, event)
}

fn append_event_to_path(path: &Path, event: &SubagentEvent) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create event log dir: {}", parent.display()))?;
    }

    let _guard = EVENT_LOG_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("event log lock poisoned"))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open event log: {}", path.display()))?;

    serde_json::to_writer(&mut file, event)
        .with_context(|| format!("failed to serialize event for {}", path.display()))?;
    writeln!(file)?;
    file.flush()?;

    debug!(path = %path.display(), kind = %event.kind, agent_id = %event.agent_id, "subagent event appended");
    Ok(())
}

fn event_log_path_for_base(base: &Path) -> PathBuf {
    base.join(".logs").join("subagent-events.ndjson")
}

fn resolve_base_dir() -> PathBuf {
    let cwd = crate::bootstrap::state::original_cwd();
    if !cwd.as_os_str().is_empty() {
        return cwd;
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn dashboard_enabled() -> bool {
    std::env::var("FEATURE_SUBAGENT_DASHBOARD")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"))
        .unwrap_or(false)
}

fn resolve_dashboard_script_path() -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()?
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let candidates = [
        exe_dir.join("../ui/subagent-dashboard/server.ts"),
        exe_dir.join("../../ui/subagent-dashboard/server.ts"),
        PathBuf::from("ui/subagent-dashboard/server.ts"),
    ];

    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("dashboard script not found"))
}

async fn wait_for_health(port: u16) -> Result<()> {
    let health_url = format!("http://127.0.0.1:{}/health", port);
    let client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(HEALTH_CHECK_TIMEOUT_MS);

    loop {
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "dashboard companion failed to start within {}ms",
                HEALTH_CHECK_TIMEOUT_MS
            );
        }

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await,
        }
    }
}

fn open_browser(url: &str) -> Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }
    .with_context(|| format!("failed to open browser for {}", url))?;

    if status.success() {
        Ok(())
    } else {
        bail!("browser open command exited with {}", status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
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
    fn event_log_path_for_base_appends_logs_file() {
        let path = event_log_path_for_base(Path::new("/tmp/dashboard-test"));
        assert_eq!(
            path,
            PathBuf::from("/tmp/dashboard-test/.logs/subagent-events.ndjson")
        );
    }

    #[test]
    fn append_event_to_path_writes_valid_ndjson() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("nested").join("subagent-events.ndjson");
        let event = SubagentEvent {
            ts: "2026-04-14T12:00:00Z".to_string(),
            kind: "spawn".to_string(),
            agent_id: "agent-123".to_string(),
            parent_agent_id: Some("parent-1".to_string()),
            description: Some("dashboard smoke".to_string()),
            model: Some("test-model".to_string()),
            depth: 2,
            background: true,
            payload: Some(serde_json::json!({ "had_error": false })),
        };

        append_event_to_path(&path, &event).expect("append event");

        let text = std::fs::read_to_string(&path).expect("read event log");
        let line = text.lines().next().expect("first line");
        let value: Value = serde_json::from_str(line).expect("parse json line");

        assert_eq!(value["kind"], "spawn");
        assert_eq!(value["agent_id"], "agent-123");
        assert_eq!(value["background"], true);
        assert_eq!(value["payload"]["had_error"], false);
    }

    #[test]
    fn emit_subagent_event_noops_when_feature_disabled() {
        let _feature = EnvGuard::set("FEATURE_SUBAGENT_DASHBOARD", "0");
        let temp = TempDir::new().expect("tempdir");
        let original_cwd = {
            let mut ps = crate::bootstrap::PROCESS_STATE.write();
            let original = ps.original_cwd.clone();
            ps.original_cwd = temp.path().to_path_buf();
            original
        };

        emit_subagent_event(
            "spawn",
            "agent-disabled",
            None,
            Some("disabled"),
            Some("test-model"),
            1,
            false,
            None,
        )
        .expect("emit event");

        {
            let mut ps = crate::bootstrap::PROCESS_STATE.write();
            ps.original_cwd = original_cwd;
        }

        assert!(
            !temp
                .path()
                .join(".logs")
                .join("subagent-events.ndjson")
                .exists(),
            "event log should not be created when the feature is disabled"
        );
    }
}
