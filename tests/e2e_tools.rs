//! E2E tests for tool registration and system prompt tool sections.
//!
//! These tests verify that tools are correctly registered and appear
//! in the system prompt. They use --dump-system-prompt to inspect
//! the prompt without requiring an API key.
//!
//! Workspace: F:\temp
//! Run with: cargo test --test e2e_tools

use assert_cmd::Command;
use predicates::prelude::*;

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

const WORKSPACE: &str = r"F:\temp";

/// Strips all API keys so no provider is detected — tests run offline.
fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
}

// =========================================================================
// 1. Tool registration — all 13 tools appear in system prompt
// =========================================================================

/// The system prompt from --dump-system-prompt should mention each tool.
/// This is a smoke test that the tool registry is wired correctly.
#[test]
fn system_prompt_contains_bash_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bash"));
}

#[test]
fn system_prompt_contains_read_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Read"));
}

#[test]
fn system_prompt_contains_write_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Write"));
}

#[test]
fn system_prompt_contains_edit_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Edit"));
}

#[test]
fn system_prompt_contains_glob_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Glob"));
}

#[test]
fn system_prompt_contains_grep_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Grep"));
}

#[test]
fn system_prompt_contains_askuser_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("AskUser"));
}

#[test]
fn system_prompt_contains_skill_tool() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Skill"));
}

// =========================================================================
// 2. System prompt structure checks
// =========================================================================

#[test]
fn system_prompt_contains_environment_section() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Environment").or(predicate::str::contains("environment")));
}

#[test]
fn system_prompt_contains_cwd_path() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        // The system prompt should embed the working directory
        .stdout(predicate::str::contains("temp").or(predicate::str::contains("F:")));
}

#[test]
fn system_prompt_mentions_permission_model() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("permission").or(predicate::str::contains("Permission")));
}

// =========================================================================
// 3. Tool-dependent print mode (requires API — expected to fail gracefully)
// =========================================================================

/// Print mode with a prompt but no API key: should not crash/panic,
/// and should report the API error in output.
#[test]
fn print_mode_bash_tool_no_crash_without_api() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["-p", "-C", WORKSPACE, "run ls in bash"])
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(predicate::str::contains("no API client configured").or(predicate::str::contains("API error")));
}

#[test]
fn print_mode_read_tool_no_crash_without_api() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["-p", "-C", WORKSPACE, "read the file test.txt"])
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(predicate::str::contains("no API client configured").or(predicate::str::contains("API error")));
}

// =========================================================================
// 4. Multiple flags combined
// =========================================================================

#[test]
fn dump_system_prompt_with_verbose_and_model() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args([
        "--dump-system-prompt",
        "-v",
        "-m",
        "test-model",
        "-C",
        WORKSPACE,
    ])
    .assert()
    .success()
    .stdout(predicate::str::is_empty().not());
}

#[test]
fn init_only_with_cwd_and_permission_mode() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args([
        "--init-only",
        "-C",
        WORKSPACE,
        "--permission-mode",
        "auto",
    ])
    .assert()
    .success();
}
