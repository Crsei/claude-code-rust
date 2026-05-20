// test infrastructure — JSONL session logging for future IPC adapter integration
//! Optional JSONL session event logging for TUI adapters.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use serde::Serialize;

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Serialize)]
struct SessionLogRecord<'a, T> {
    ts: String,
    kind: &'a str,
    payload: &'a T,
}

pub fn maybe_init(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)?;
    *LOG_DIR.lock().expect("session log lock poisoned") = Some(dir.to_path_buf());
    Ok(())
}

pub fn maybe_init_from_env() -> std::io::Result<bool> {
    let Some(dir) = std::env::var_os("CC_RUST_TUI_SESSION_LOG_DIR") else {
        return Ok(false);
    };
    maybe_init(PathBuf::from(dir))?;
    Ok(true)
}

pub fn log_record<T: Serialize>(kind: &str, payload: &T) {
    let Some(dir) = LOG_DIR.lock().expect("session log lock poisoned").clone() else {
        return;
    };
    let _ = write_record(&dir, kind, payload);
}

pub fn log_session_end() {
    log_record("session_end", &serde_json::json!({}));
}

fn write_record<T: Serialize>(dir: &Path, kind: &str, payload: &T) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join("tui-session.jsonl");
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let record = SessionLogRecord {
        ts: now_ts(),
        kind,
        payload,
    };
    serde_json::to_writer(&mut file, &record)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn now_ts() -> String {
    Utc::now().to_rfc3339()
}
