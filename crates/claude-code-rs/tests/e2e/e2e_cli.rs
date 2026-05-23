//! E2E tests for the claude-code-rs CLI.
//!
//! Tests exercise the compiled binary as a black box via `assert_cmd`.
//! Workspace for file-side-effect tests: F:\temp
//!
//! Run with: cargo test --test e2e_cli

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;

#[path = "test_workspace.rs"]
mod test_workspace;

/// Helper: build a Command pointing at the compiled binary.
fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn clear_auth_env(cmd: &mut Command) -> &mut Command {
    cmd.env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("AZURE_API_KEY")
        .env_remove("AZURE_AUTH_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_CODEX_AUTH_TOKEN")
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("GOOGLE_API_KEY")
        .env_remove("DEEPSEEK_API_KEY")
}

fn assert_jsonl_stdout(stdout: &[u8]) -> Vec<Value> {
    let text = std::str::from_utf8(stdout).expect("stdout is utf-8");
    let mut messages = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<Value>(trimmed)
            .unwrap_or_else(|err| panic!("stdout line {} is not JSON: {}\n{}", idx + 1, err, line));
        messages.push(parsed);
    }
    assert!(!messages.is_empty(), "expected JSONL messages on stdout");
    messages
}

fn assert_sdk_jsonl(stdout: &[u8]) {
    let messages = assert_jsonl_stdout(stdout);
    assert!(
        messages.iter().any(|msg| msg["type"] == "system_init"),
        "expected system_init message in JSONL output: {:?}",
        messages
    );
    assert!(
        messages.iter().any(|msg| msg["type"] == "result"),
        "expected result message in JSONL output: {:?}",
        messages
    );
}

/// Workspace root used for tests that need a real directory.
fn workspace() -> &'static str {
    test_workspace::workspace()
}

// =========================================================================
// 1. Fast paths (no API key needed, immediate exit)
// =========================================================================

#[test]
fn version_flag_prints_version_and_exits() {
    cli()
        .arg("-V")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-code-rs"));
}

#[test]
fn version_long_flag() {
    cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-code-rs"));
}

#[test]
fn help_flag_prints_usage_and_exits() {
    cli().arg("--help").assert().success().stdout(
        predicate::str::contains("cc-rust CLI")
            .and(predicate::str::contains("--print"))
            .and(predicate::str::contains("--cwd")),
    );
}

#[test]
fn init_only_exits_successfully() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .arg("--init-only")
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn dump_system_prompt_outputs_prompt_and_exits() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--dump-system-prompt", "-C", workspace()])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("tool"));
}

// =========================================================================
// 2. --cwd / -C workspace handling
// =========================================================================

#[test]
fn cwd_flag_accepts_valid_directory() {
    assert!(Path::new(workspace()).is_dir(), "F:\\temp must exist");
    let home = tempfile::tempdir().expect("temp cc-rust home");

    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-C", workspace(), "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn cwd_flag_rejects_nonexistent_directory() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-C", r"F:\this\path\does\not\exist", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("does not exist")
                .or(predicate::str::contains("not a directory")),
        );
}

#[test]
fn cwd_short_flag_works() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-C", workspace(), "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

// =========================================================================
// 3. Print mode (-p) edge cases
// =========================================================================

#[test]
fn print_mode_without_prompt_fails() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .arg("-p")
        .env("CC_RUST_HOME", home.path())
        .assert()
        .failure();
}

#[test]
fn print_mode_no_api_key_reports_error() {
    // With a prompt but no API key, the error is printed to stderr.
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-p", "hello"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("no API client configured")
                .or(predicate::str::contains("API error"))
                .or(predicate::str::contains("No API provider detected")),
        );
}

#[test]
fn json_output_mode_emits_machine_parseable_jsonl_for_prompt_argument() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    let output = clear_auth_env(&mut cmd)
        .args(["--output-format", "json", "-p", "hello"])
        .env("CC_RUST_HOME", home.path())
        .output()
        .expect("run json output mode");

    assert_sdk_jsonl(&output.stdout);
}

#[test]
fn json_output_mode_reads_prompt_from_stdin() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    let output = clear_auth_env(&mut cmd)
        .args(["--output-format", "json", "-p"])
        .env("CC_RUST_HOME", home.path())
        .write_stdin("hello from stdin\n")
        .output()
        .expect("run json output mode with stdin");

    assert_sdk_jsonl(&output.stdout);
}

#[test]
fn stream_json_output_format_currently_follows_plain_print_dispatch() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--output-format", "stream-json", "-p", "hello"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("no API client configured")
                .or(predicate::str::contains("API error"))
                .or(predicate::str::contains("No API provider detected")),
        );
}

// =========================================================================
// 4. Model override (-m)
// =========================================================================

#[test]
fn model_flag_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-m", "gpt-4o", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn dump_system_prompt_with_model_override() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args([
            "--dump-system-prompt",
            "-m",
            "custom-model-123",
            "-C",
            workspace(),
        ])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// =========================================================================
// 5. Verbose flag
// =========================================================================

#[test]
fn verbose_flag_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["-v", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn daemon_management_reports_stopped_state_without_running_daemon() {
    let home = tempfile::tempdir().expect("temp daemon home");

    cli()
        .args(["daemon", "status"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("daemon status: stopped"));

    cli()
        .args(["daemon", "sleep", "1", "e2e"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "daemon command failed: daemon is not running",
        ));
}

// =========================================================================
// 6. System prompt overrides
// =========================================================================

#[test]
fn custom_system_prompt_in_dump() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args([
            "--dump-system-prompt",
            "--system-prompt",
            "You are a test bot.",
            "-C",
            workspace(),
        ])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("You are a test bot."));
}

#[test]
fn append_system_prompt_in_dump() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args([
            "--dump-system-prompt",
            "--append-system-prompt",
            "EXTRA CONTEXT INJECTED",
            "-C",
            workspace(),
        ])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("EXTRA CONTEXT INJECTED"));
}

// =========================================================================
// 7. Permission mode flag
// =========================================================================

#[test]
fn permission_mode_auto_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--permission-mode", "auto", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn permission_mode_bypass_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--permission-mode", "bypass", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn no_network_flag_accepted_for_init_only() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--no-network", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

// =========================================================================
// 8. Max budget / max turns
// =========================================================================

#[test]
fn max_budget_flag_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--max-budget", "5.0", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

#[test]
fn max_turns_flag_accepted() {
    let home = tempfile::tempdir().expect("temp cc-rust home");
    let mut cmd = cli();
    clear_auth_env(&mut cmd)
        .args(["--max-turns", "3", "--init-only"])
        .env("CC_RUST_HOME", home.path())
        .assert()
        .success();
}

// =========================================================================
// 9. Invalid argument handling (clap errors)
// =========================================================================

#[test]
fn unknown_flag_fails() {
    cli().arg("--nonexistent-flag").assert().failure().stderr(
        predicate::str::contains("unexpected argument").or(predicate::str::contains("error")),
    );
}

#[test]
fn max_turns_requires_value() {
    cli().arg("--max-turns").assert().failure();
}

#[test]
fn chrome_and_no_chrome_conflict() {
    cli()
        .args(["--chrome", "--no-chrome", "--init-only"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
