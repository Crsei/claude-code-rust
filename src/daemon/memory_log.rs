//! Daily append-only log system for KAIROS perpetual sessions.
//! Logs stored at ~/.cc-rust/logs/YYYY/MM/YYYY-MM-DD.md

use std::path::PathBuf;

use chrono::Local;
use tracing::{debug, error};

fn log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cc-rust")
        .join("logs")
}

pub fn today_log_path() -> PathBuf {
    let now = Local::now();
    log_dir()
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(now.format("%Y-%m-%d.md").to_string())
}

pub fn read_today_log() -> String {
    std::fs::read_to_string(today_log_path()).unwrap_or_default()
}

pub fn append_log_entry(content: &str) {
    let path = today_log_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("failed to create log directory: {}", e);
            return;
        }
    }
    let now = Local::now();
    let is_new_file = !path.exists();
    let mut entry = String::new();
    if is_new_file {
        entry.push_str(&format!("## {}\n\n", now.format("%Y-%m-%d")));
    }
    entry.push_str(&format!("### {}\n", now.format("%H:%M")));
    for line in content.lines() {
        entry.push_str(&format!("- {}\n", line));
    }
    entry.push('\n');

    use std::io::Write;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(entry.as_bytes()) {
                error!("failed to write log entry: {}", e);
            } else {
                debug!("log entry appended to {}", path.display());
            }
        }
        Err(e) => error!("failed to open log file: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_today_log_path_format() {
        let path = today_log_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains(".cc-rust"),
            "path should contain .cc-rust: {}",
            path_str
        );
        assert!(
            path_str.contains("logs"),
            "path should contain logs: {}",
            path_str
        );
        assert!(
            path_str.ends_with(".md"),
            "path should end with .md: {}",
            path_str
        );
    }

    #[test]
    fn test_log_dir_structure() {
        let dir = log_dir();
        let dir_str = dir.to_string_lossy();
        assert!(
            dir_str.contains("logs"),
            "dir should contain logs: {}",
            dir_str
        );
    }
}
