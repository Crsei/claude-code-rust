//! Shared helpers for headless IPC tests.
//!
//! Provides utilities to spawn the binary in `--headless` mode and
//! communicate via JSONL, mirroring what the real TypeScript client
//! (`ui/src/ipc/client.ts`) does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Timeout for reading a single line from the backend.
pub const LINE_TIMEOUT: Duration = Duration::from_secs(15);

/// Timeout for live API tests (longer due to network + model latency).
pub const LIVE_TIMEOUT: Duration = Duration::from_secs(60);

/// Spawn the binary in --headless mode.
/// Returns (child, stdin_writer, stdout_reader).
pub fn spawn_headless(
    extra_args: &[&str],
    strip_keys: bool,
) -> (
    std::process::Child,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
) {
    let mut cmd = Command::new(
        assert_cmd::cargo::cargo_bin("claude-code-rs")
            .to_str()
            .expect("binary path"),
    );
    cmd.arg("--headless")
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    if strip_keys {
        cmd.env("ANTHROPIC_API_KEY", "")
            .env("AZURE_API_KEY", "")
            .env("OPENAI_API_KEY", "")
            .env("OPENROUTER_API_KEY", "")
            .env("GOOGLE_API_KEY", "")
            .env("DEEPSEEK_API_KEY", "");
    }

    let mut child = cmd.spawn().expect("failed to spawn headless binary");
    let stdin = child.stdin.take().expect("stdin not piped");
    let stdout = BufReader::new(child.stdout.take().expect("stdout not piped"));

    (child, stdin, stdout)
}

/// Read one JSON line from stdout, skipping non-JSON lines (tracing output).
/// This mirrors the real TypeScript client behavior in `client.ts`:
///   `try { JSON.parse(line) } catch { /* ignore */ }`
pub fn read_line_json(
    reader: &mut BufReader<std::process::ChildStdout>,
    _timeout: Duration,
) -> serde_json::Value {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => panic!("headless: stdout closed (EOF) while waiting for JSON message"),
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Skip non-JSON lines (tracing output, ANSI warnings, etc.)
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    return val;
                }
            }
            Err(e) => panic!("headless: read error: {}", e),
        }
    }
}

/// Send a JSON message to the backend's stdin.
pub fn send_msg(stdin: &mut std::process::ChildStdin, msg: &serde_json::Value) {
    let line = serde_json::to_string(msg).expect("serialize message");
    writeln!(stdin, "{}", line).expect("write to stdin");
    stdin.flush().expect("flush stdin");
}

/// Collect JSON lines until a predicate matches or timeout.
/// Returns all collected messages (including the one that matched).
pub fn collect_until(
    reader: &mut BufReader<std::process::ChildStdout>,
    predicate: impl Fn(&serde_json::Value) -> bool,
    timeout: Duration,
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            panic!(
                "collect_until: timeout after {:?} — collected {} messages: {:?}",
                timeout,
                messages.len(),
                messages
            );
        }

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    let done = predicate(&val);
                    messages.push(val);
                    if done {
                        break;
                    }
                }
                // Skip non-JSON lines (tracing output)
            }
            Err(_) => break,
        }
    }

    messages
}
