//! E2E tests for the claude-code-rs CLI.
//!
//! Tests exercise the compiled binary as a black box via `assert_cmd`.
//! Workspace for file-side-effect tests: F:\temp
//!
//! Run with: cargo test --test e2e_cli

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

#[path = "test_workspace.rs"]
mod test_workspace;

/// Helper: build a Command pointing at the compiled binary.
fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
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
fn init_only_exits_successfully() {
    cli()
        .arg("--init-only")
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

#[test]
fn dump_system_prompt_outputs_prompt_and_exits() {
    cli()
        .args(["--dump-system-prompt", "-C", workspace()])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
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

    cli()
        .args(["-C", workspace(), "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

#[test]
fn cwd_flag_rejects_nonexistent_directory() {
    cli()
        .args(["-C", r"F:\this\path\does\not\exist", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .failure()
        // Error may go to stdout (tracing ERROR macro) or stderr
        .stdout(
            predicate::str::contains("does not exist")
                .or(predicate::str::contains("not a directory")),
        );
}

#[test]
fn cwd_short_flag_works() {
    cli()
        .args(["-C", workspace(), "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

// =========================================================================
// 3. Print mode (-p) edge cases
// =========================================================================

#[test]
fn print_mode_without_prompt_fails() {
    cli()
        .arg("-p")
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .failure();
}

#[test]
fn print_mode_no_api_key_reports_error() {
    // With a prompt but no API key, the error is printed to stdout.
    // The process may exit 0 (error is in output, not exit code).
    cli()
        .args(["-p", "hello"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(
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
    cli()
        .args(["-m", "gpt-4o", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

#[test]
fn dump_system_prompt_with_model_override() {
    cli()
        .args([
            "--dump-system-prompt",
            "-m",
            "custom-model-123",
            "-C",
            workspace(),
        ])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// =========================================================================
// 5. Verbose flag
// =========================================================================

#[test]
fn verbose_flag_accepted() {
    cli()
        .args(["-v", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

// =========================================================================
// 6. System prompt overrides
// =========================================================================

#[test]
fn custom_system_prompt_in_dump() {
    cli()
        .args([
            "--dump-system-prompt",
            "--system-prompt",
            "You are a test bot.",
            "-C",
            workspace(),
        ])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success()
        .stdout(predicate::str::contains("You are a test bot."));
}

#[test]
fn append_system_prompt_in_dump() {
    cli()
        .args([
            "--dump-system-prompt",
            "--append-system-prompt",
            "EXTRA CONTEXT INJECTED",
            "-C",
            workspace(),
        ])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success()
        .stdout(predicate::str::contains("EXTRA CONTEXT INJECTED"));
}

// =========================================================================
// 7. Permission mode flag
// =========================================================================

#[test]
fn permission_mode_auto_accepted() {
    cli()
        .args(["--permission-mode", "auto", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

#[test]
fn permission_mode_bypass_accepted() {
    cli()
        .args(["--permission-mode", "bypass", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

// =========================================================================
// 8. Max budget / max turns
// =========================================================================

#[test]
fn max_budget_flag_accepted() {
    cli()
        .args(["--max-budget", "5.0", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .assert()
        .success();
}

#[test]
fn max_turns_flag_accepted() {
    cli()
        .args(["--max-turns", "3", "--init-only"])
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
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
