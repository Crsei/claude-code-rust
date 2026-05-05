//! Cross-process daemon supervisor state.
//!
//! This is the Phase 1 durability layer for daemon management. It lets one
//! process publish daemon status under `~/.cc-rust/daemon/` and another process
//! inspect or request shutdown without sharing memory with the daemon runtime.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
#[cfg(test)]
const DEFAULT_DAEMON_PORT: u16 = 19836;
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(5);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonRunStatus {
    Running,
    Stopped,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonWorkerSummary {
    pub worker_id: String,
    pub kind: String,
    pub pid: Option<u32>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonProcessState {
    pub schema_version: u32,
    pub status: DaemonRunStatus,
    pub pid: u32,
    pub cwd: PathBuf,
    pub port: u16,
    pub health_url: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shutdown_requested: bool,
    pub workers: Vec<DaemonWorkerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonShutdownRequest {
    pub schema_version: u32,
    pub requested_at: DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonStatusSnapshot {
    Running(DaemonProcessState),
    Stale(DaemonProcessState),
    Stopped,
}

pub fn daemon_dir() -> PathBuf {
    crate::config::paths::daemon_dir()
}

pub fn state_path() -> PathBuf {
    daemon_dir().join("supervisor.json")
}

pub fn shutdown_request_path() -> PathBuf {
    daemon_dir().join("shutdown-request.json")
}

pub fn write_started(port: u16, cwd: &Path) -> Result<DaemonProcessState> {
    clear_shutdown_request()?;
    let now = Utc::now();
    let state = DaemonProcessState {
        schema_version: SCHEMA_VERSION,
        status: DaemonRunStatus::Running,
        pid: std::process::id(),
        cwd: cwd.to_path_buf(),
        port,
        health_url: health_url(port),
        started_at: now,
        updated_at: now,
        shutdown_requested: false,
        workers: Vec::new(),
    };
    write_state(&state)?;
    Ok(state)
}

pub fn write_stopped(port: u16, cwd: &Path) -> Result<DaemonProcessState> {
    clear_shutdown_request()?;
    let now = Utc::now();
    let state = DaemonProcessState {
        schema_version: SCHEMA_VERSION,
        status: DaemonRunStatus::Stopped,
        pid: std::process::id(),
        cwd: cwd.to_path_buf(),
        port,
        health_url: health_url(port),
        started_at: now,
        updated_at: now,
        shutdown_requested: false,
        workers: Vec::new(),
    };
    write_state(&state)?;
    Ok(state)
}

pub fn read_state() -> Result<Option<DaemonProcessState>> {
    let path = state_path();
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read daemon state {}", path.display()))?;
    let state: DaemonProcessState = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse daemon state {}", path.display()))?;
    Ok(Some(state))
}

pub fn status_snapshot() -> Result<DaemonStatusSnapshot> {
    let Some(mut state) = read_state()? else {
        return Ok(DaemonStatusSnapshot::Stopped);
    };
    if state.status != DaemonRunStatus::Running {
        return Ok(DaemonStatusSnapshot::Stopped);
    }
    if process_is_alive(state.pid) {
        return Ok(DaemonStatusSnapshot::Running(state));
    }
    state.status = DaemonRunStatus::Stale;
    state.updated_at = Utc::now();
    write_state(&state)?;
    Ok(DaemonStatusSnapshot::Stale(state))
}

pub fn request_shutdown(reason: &str) -> Result<()> {
    ensure_daemon_dir()?;
    let req = DaemonShutdownRequest {
        schema_version: SCHEMA_VERSION,
        requested_at: Utc::now(),
        reason: reason.to_string(),
    };
    atomic_write_json(&shutdown_request_path(), &req)?;

    if let Some(mut state) = read_state()? {
        state.shutdown_requested = true;
        state.updated_at = Utc::now();
        write_state(&state)?;
    }
    Ok(())
}

pub fn shutdown_requested() -> bool {
    shutdown_request_path().is_file()
}

pub fn clear_shutdown_request() -> Result<()> {
    let path = shutdown_request_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub async fn wait_for_shutdown_request() {
    loop {
        if shutdown_requested() {
            return;
        }
        tokio::time::sleep(SHUTDOWN_POLL_INTERVAL).await;
    }
}

pub fn try_run_management_command(args: &[String], cwd: &Path, port: u16) -> Option<ExitCode> {
    if args.first().map(String::as_str) != Some("daemon") {
        return None;
    }

    let subcommand = args.get(1).map(String::as_str).unwrap_or("status");
    let code = match subcommand {
        "start" => print_result(start_daemon(args, cwd, port)),
        "status" => print_result(print_status()),
        "stop" => print_result(stop_daemon()),
        "restart" => print_result(restart_daemon(args, cwd, port)),
        "help" | "--help" | "-h" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown daemon subcommand: {other}");
            print_usage();
            ExitCode::FAILURE
        }
    };
    Some(code)
}

fn start_daemon(args: &[String], cwd: &Path, fallback_port: u16) -> Result<()> {
    match status_snapshot()? {
        DaemonStatusSnapshot::Running(state) => {
            println!(
                "daemon already running: pid={} health={}",
                state.pid, state.health_url
            );
            return Ok(());
        }
        DaemonStatusSnapshot::Stale(state) => {
            eprintln!("cleaned stale daemon state for pid={}", state.pid);
        }
        DaemonStatusSnapshot::Stopped => {}
    }

    if !crate::config::features::enabled(crate::config::features::Feature::Kairos) {
        anyhow::bail!("daemon start requires FEATURE_KAIROS=1");
    }

    ensure_daemon_dir()?;
    let port = parse_port(args).unwrap_or(fallback_port);
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let log_path = daemon_dir().join("supervisor.log");
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open daemon log {}", log_path.display()))?;
    let log_file_err = log_file
        .try_clone()
        .with_context(|| format!("failed to clone daemon log {}", log_path.display()))?;

    let mut cmd = Command::new(exe);
    cmd.arg("--daemon")
        .arg("--port")
        .arg(port.to_string())
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err));

    configure_detached(&mut cmd);
    let child = cmd.spawn().context("failed to spawn daemon supervisor")?;
    println!(
        "daemon start requested: pid={} health={} log={}",
        child.id(),
        health_url(port),
        log_path.display()
    );
    Ok(())
}

fn stop_daemon() -> Result<()> {
    let snapshot = status_snapshot()?;
    let DaemonStatusSnapshot::Running(state) = snapshot else {
        println!("daemon is not running");
        return Ok(());
    };

    request_shutdown("daemon stop command")?;
    let deadline = Instant::now() + STOP_GRACE_PERIOD;
    while Instant::now() < deadline {
        if !process_is_alive(state.pid) {
            println!("daemon stopped: pid={}", state.pid);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    terminate_process_tree(state.pid)?;
    println!("daemon terminated after timeout: pid={}", state.pid);
    Ok(())
}

fn restart_daemon(args: &[String], cwd: &Path, port: u16) -> Result<()> {
    stop_daemon()?;
    start_daemon(args, cwd, port)
}

fn print_status() -> Result<()> {
    match status_snapshot()? {
        DaemonStatusSnapshot::Running(state) => {
            println!("daemon status: running");
            println!("  pid: {}", state.pid);
            println!("  cwd: {}", state.cwd.display());
            println!("  port: {}", state.port);
            println!("  health: {}", state.health_url);
            println!("  started_at: {}", state.started_at.to_rfc3339());
            println!("  updated_at: {}", state.updated_at.to_rfc3339());
            println!("  shutdown_requested: {}", state.shutdown_requested);
            println!("  workers: {}", state.workers.len());
        }
        DaemonStatusSnapshot::Stale(state) => {
            println!("daemon status: stale");
            println!("  stale_pid: {}", state.pid);
            println!("  state: {}", state_path().display());
        }
        DaemonStatusSnapshot::Stopped => {
            println!("daemon status: stopped");
        }
    }
    Ok(())
}

fn write_state(state: &DaemonProcessState) -> Result<()> {
    ensure_daemon_dir()?;
    atomic_write_json(&state_path(), state)
}

fn ensure_daemon_dir() -> Result<()> {
    fs::create_dir_all(daemon_dir())
        .with_context(|| format!("failed to create {}", daemon_dir().display()))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("failed to create {}", tmp.display()))?;
        let bytes = serde_json::to_vec_pretty(value)?;
        file.write_all(&bytes)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", tmp.display()))?;
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(first_err) if path.exists() => {
            fs::remove_file(path)
                .with_context(|| format!("failed to replace {}", path.display()))?;
            fs::rename(&tmp, path).with_context(|| {
                format!(
                    "failed to rename {} to {} after replace fallback: {first_err}",
                    tmp.display(),
                    path.display()
                )
            })
        }
        Err(err) => Err(err)
            .with_context(|| format!("failed to rename {} to {}", tmp.display(), path.display())),
    }
}

fn health_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/health")
}

fn parse_port(args: &[String]) -> Option<u16> {
    args.windows(2)
        .find(|pair| pair[0] == "--port")
        .and_then(|pair| pair[1].parse().ok())
}

fn print_result(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("daemon command failed: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  claude daemon [status]\n  claude daemon start [--port <port>]\n  claude daemon stop\n  claude daemon restart [--port <port>]"
    );
}

#[cfg(windows)]
fn configure_detached(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

#[cfg(not(windows))]
fn configure_detached(_cmd: &mut Command) {}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    let Ok(output) = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.contains(&format!("\"{pid}\"")) || line.contains(&pid.to_string()))
}

#[cfg(unix)]
fn terminate_process_tree(pid: u32) -> Result<()> {
    let pid = pid as libc::pid_t;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_millis(500));
    if process_is_alive(pid as u32) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn terminate_process_tree(pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .context("failed to run taskkill")?;
    if !status.success() {
        anyhow::bail!("taskkill failed with status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

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
    fn write_and_read_state_uses_cc_rust_home() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        let cwd = temp.path().join("workspace");
        fs::create_dir_all(&cwd).unwrap();

        let state = write_started(19999, &cwd).unwrap();
        let read_back = read_state().unwrap().unwrap();

        assert_eq!(state.pid, std::process::id());
        assert_eq!(read_back.port, 19999);
        assert_eq!(read_back.cwd, cwd);
        assert!(state_path().starts_with(temp.path()));
    }

    #[test]
    #[serial]
    fn shutdown_request_sets_state_flag() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", temp.path());
        write_started(DEFAULT_DAEMON_PORT, temp.path()).unwrap();

        request_shutdown("test").unwrap();
        let state = read_state().unwrap().unwrap();

        assert!(shutdown_requested());
        assert!(state.shutdown_requested);
    }

    #[test]
    fn current_process_is_alive() {
        assert!(process_is_alive(std::process::id()));
    }

    #[test]
    fn parses_port_from_management_args() {
        let args = vec![
            "daemon".to_string(),
            "start".to_string(),
            "--port".to_string(),
            "20100".to_string(),
        ];
        assert_eq!(parse_port(&args), Some(20100));
    }
}
