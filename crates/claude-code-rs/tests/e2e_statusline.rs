//! E2E tests for the scriptable status line (issue #11).
//!
//! Black-box checks:
//!
//! - the committed settings schema declares every documented statusLine
//!   field (type/command/enabled/padding/refreshIntervalMs/timeoutMs);
//! - CLI `--init-only` accepts a settings.json with a command-typed
//!   statusLine without choking (fail-closed would break the TUI);
//! - CLI `--init-only` tolerates an invalid statusLine (bad type, bogus
//!   padding) — the runtime must boot and let `/statusline status` show
//!   the problem later.
//!
//! Run with: `cargo test --test e2e_statusline`
//!
//! Unit-level behaviour (runner spawn/stdin/stdout, throttling, timeout,
//! payload serialization) is covered in `src/ui/status_line/*.rs`
//! `#[cfg(test)]` blocks.

use serde_json::json;
use serial_test::serial;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn headless_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("claude-code-rs")
}

fn normalize_path_for_assert(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

struct HeadlessSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl HeadlessSession {
    fn spawn(cwd: &std::path::Path, cc_rust_home: &std::path::Path) -> Self {
        let mut child = Command::new(headless_bin())
            .arg("--headless")
            .arg("-C")
            .arg(cwd)
            .env("CC_RUST_HOME", cc_rust_home)
            .env("ANTHROPIC_API_KEY", "")
            .env("AZURE_API_KEY", "")
            .env("OPENAI_API_KEY", "")
            .env_remove("ANTHROPIC_AUTH_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn headless");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, msg: serde_json::Value) {
        writeln!(self.stdin, "{}", serde_json::to_string(&msg).unwrap()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_until_type(&mut self, expected_type: &str) -> serde_json::Value {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self.stdout.read_line(&mut line).expect("read stdout");
            assert!(
                bytes > 0,
                "headless backend exited before {}",
                expected_type
            );
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed: serde_json::Value = serde_json::from_str(trimmed).expect("valid JSONL");
            if parsed.get("type").and_then(|value| value.as_str()) == Some(expected_type) {
                return parsed;
            }
        }
    }

    fn shutdown(&mut self) {
        let _ = self.send_quit();
        let _ = self.child.wait();
    }

    fn send_quit(&mut self) -> std::io::Result<()> {
        writeln!(self.stdin, "{}", json!({ "type": "quit" }))?;
        self.stdin.flush()
    }
}

impl Drop for HeadlessSession {
    fn drop(&mut self) {
        let _ = self.send_quit();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[serial]
fn schema_file_declares_extended_status_line_shape() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("schemas")
        .join("settings.schema.json");
    let txt = std::fs::read_to_string(&path).expect("schema file exists");
    let v: serde_json::Value = serde_json::from_str(&txt).expect("valid JSON");
    let props = v
        .pointer("/properties/statusLine/properties")
        .and_then(|p| p.as_object())
        .expect("statusLine properties exist");
    for must_have in [
        "type",
        "command",
        "script",
        "format",
        "enabled",
        "padding",
        "refreshIntervalMs",
        "timeoutMs",
    ] {
        assert!(
            props.contains_key(must_have),
            "statusLine schema missing key '{}'",
            must_have
        );
    }
    // `type` should carry an explicit enum so IDEs/lints can catch typos.
    let type_enum = v
        .pointer("/properties/statusLine/properties/type/enum")
        .and_then(|e| e.as_array())
        .expect("type enum declared");
    let names: Vec<&str> = type_enum.iter().filter_map(|x| x.as_str()).collect();
    for required in ["none", "minimal", "command", "script"] {
        assert!(
            names.contains(&required),
            "statusLine.type enum missing '{}'",
            required
        );
    }
}

/// CLI init-only should accept a well-formed statusLine config.
#[test]
#[serial]
fn cli_init_only_accepts_status_line_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    let body = json!({
        "statusLine": {
            "type": "command",
            "command": "echo hello",
            "padding": 2,
            "refreshIntervalMs": 500,
            "timeoutMs": 3000,
            "enabled": true
        }
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", dir.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

/// A malformed statusLine (unknown `type`, negative padding) must NOT
/// prevent startup. The runtime logs / surfaces the issue through
/// `/statusline status`; it must still boot.
#[test]
#[serial]
fn cli_init_only_tolerates_malformed_status_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    // Deliberately bogus shape — type is not in the enum and padding is
    // out of range.
    let body = json!({
        "statusLine": {
            "type": "orbiting-laser",
            "command": "",
            "padding": 9999,
            "refreshIntervalMs": 1
        }
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", dir.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

/// A missing statusLine block should still start cleanly with the
/// default (no-custom-command) configuration.
#[test]
#[serial]
fn cli_init_only_starts_without_status_line_config() {
    let dir = tempfile::tempdir().expect("tempdir");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", dir.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

#[test]
#[serial]
fn headless_statusline_payload_surfaces_runtime_snapshot_fields() {
    let home = tempfile::tempdir().expect("cc-rust home");
    std::fs::write(
        home.path().join("settings.json"),
        serde_json::to_string_pretty(&json!({
            "outputStyle": "explanatory",
            "editorMode": "vim"
        }))
        .unwrap(),
    )
    .expect("write settings");

    let project = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = project
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();

    let mut session = HeadlessSession::spawn(&project, home.path());
    let _ready = session.read_until_type("ready");

    session.send(json!({
        "type": "slash_command",
        "raw": "/statusline payload"
    }));

    let msg = session.read_until_type("system_info");
    let payload_text = msg
        .get("text")
        .and_then(|value| value.as_str())
        .expect("system_info text");
    let payload: serde_json::Value =
        serde_json::from_str(payload_text).expect("slash command payload JSON");

    assert_eq!(
        payload
            .pointer("/outputStyle")
            .and_then(|value| value.as_str()),
        Some("explanatory")
    );
    assert_eq!(
        payload
            .pointer("/vim/mode")
            .and_then(|value| value.as_str()),
        Some("NORMAL")
    );
    let payload_project_dir = payload
        .pointer("/workspace/projectDir")
        .and_then(|value| value.as_str())
        .expect("workspace.projectDir");
    assert_eq!(
        normalize_path_for_assert(payload_project_dir),
        normalize_path_for_assert(&workspace_root.to_string_lossy())
    );
    assert!(
        payload
            .pointer("/workspace/gitBranch")
            .and_then(|value| value.as_str())
            .map(|branch| !branch.is_empty())
            .unwrap_or(false),
        "expected gitBranch in payload"
    );
    assert_eq!(
        payload
            .pointer("/context/maxTokens")
            .and_then(|value| value.as_u64()),
        Some(200_000)
    );
    assert_eq!(
        payload
            .pointer("/messageCount")
            .and_then(|value| value.as_u64()),
        Some(0)
    );

    session.shutdown();
}
