//! E2E tests for services integration (tool_use_summary, prompt_suggestion).
//!
//! Offline tests verify that the new service integration doesn't break
//! existing CLI flows (init, system prompt, print mode).
//!
//! Live tests (IGNORED) verify that tool-use sessions with summary injection
//! and multi-tool workflows still complete correctly.
//!
//! Run offline:   cargo test --test e2e_services
//! Run live:      cargo test --test e2e_services -- --ignored

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::time::Duration;

const WORKSPACE: &str = r"F:\temp";
const TOOL_TIMEOUT_SECS: u64 = 120;

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
}

fn tool_cli() -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(r"F:\AIclassmanager\cc\rust");
    cmd.timeout(Duration::from_secs(TOOL_TIMEOUT_SECS));
    cmd
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

// =========================================================================
//  Offline: tool_use_summary integration doesn't break init / system prompt
// =========================================================================

/// --init-only still succeeds after tool_use_summary integration into the
/// query loop. This catches import errors / type mismatches at link time.
#[test]
fn init_succeeds_with_summary_integration() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--init-only", "-C", WORKSPACE])
        .assert()
        .success();
}

/// System prompt still renders correctly after query loop changes.
#[test]
fn system_prompt_unaffected_by_summary_integration() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        // Core tools still present
        .stdout(predicate::str::contains("Bash"))
        .stdout(predicate::str::contains("Read"))
        .stdout(predicate::str::contains("Edit"));
}

/// Print mode with no API key still fails gracefully (doesn't panic from
/// the new services code path).
#[test]
fn print_mode_graceful_without_api() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["-p", "-C", WORKSPACE, "hello"])
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .assert()
        .stdout(
            predicate::str::contains("no API client configured")
                .or(predicate::str::contains("API error")),
        );
}

// =========================================================================
//  Offline: prompt_suggestion integration doesn't break init
// =========================================================================

/// --init-only verifies prompt_suggestion types compile and link correctly.
#[test]
fn init_succeeds_with_suggestion_integration() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--init-only", "-C", WORKSPACE])
        .assert()
        .success();
}

/// Multiple combined flags still work after App struct gained the new
/// `suggestions` field.
#[test]
fn combined_flags_work_with_suggestions_field() {
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

// =========================================================================
//  Live: tool_use_summary — regression tests (IGNORED, need API key)
// =========================================================================

/// Single Bash tool use — the summary should be generated internally
/// (we can't see it in print-mode output, but the session must complete
/// without crashing).
#[test]
#[ignore]
fn live_summary_single_bash_tool() {
    tool_cli()
        .args([
            "-p",
            "-C",
            WORKSPACE,
            "--permission-mode",
            "bypass",
            "Use the Bash tool to run: echo SUMMARY_TEST_OK",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("SUMMARY_TEST_OK"));
}

/// Multi-tool workflow exercises the summary accumulation across multiple
/// tool calls in a single turn. The summary is injected as a system message
/// for the next turn — this tests that it doesn't corrupt the conversation.
#[test]
#[ignore]
fn live_summary_multi_tool_no_corruption() {
    let file_a = Path::new(WORKSPACE).join("_e2e_summary_a.txt");
    let file_b = Path::new(WORKSPACE).join("_e2e_summary_b.txt");
    cleanup(&file_a);
    cleanup(&file_b);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C",
                WORKSPACE,
                "--permission-mode",
                "bypass",
                &format!(
                    "Do these steps:\n\
                     1. Use Bash to run: echo HELLO\n\
                     2. Write '111' to {}\n\
                     3. Write '222' to {}\n\
                     4. Read both files and tell me the contents.",
                    file_a.display(),
                    file_b.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("111").and(predicate::str::contains("222")));
    });

    cleanup(&file_a);
    cleanup(&file_b);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Two consecutive turns — the summary from the first turn should be
/// injected before the second API call. We verify both turns produce
/// correct results (the injected summary didn't confuse the model).
#[test]
#[ignore]
fn live_summary_injected_across_turns() {
    let test_file = Path::new(WORKSPACE).join("_e2e_summary_turns.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C",
                WORKSPACE,
                "--permission-mode",
                "bypass",
                "--max-turns",
                "5",
                &format!(
                    "Write 'TURN_ONE' to {}. Then read it back and confirm the content.",
                    test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("TURN_ONE"));
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

// =========================================================================
//  Live: prompt_suggestion — regression (IGNORED)
// =========================================================================

/// After a tool-use response completes, the suggestion generation should
/// not crash. We can't inspect TUI suggestions from print mode, but we
/// can verify the session completes successfully.
#[test]
#[ignore]
fn live_suggestions_no_crash_after_tool_use() {
    tool_cli()
        .args([
            "-p",
            "-C",
            WORKSPACE,
            "--permission-mode",
            "bypass",
            "Use the Bash tool to run: echo SUGGESTION_TEST_OK",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("SUGGESTION_TEST_OK"));
}

/// A pure chat response (no tools) should also complete without the
/// suggestion service crashing, even though there are no tool names
/// to base suggestions on.
#[test]
#[ignore]
fn live_suggestions_no_crash_chat_only() {
    tool_cli()
        .args(["-p", "-C", WORKSPACE, "Say exactly: CHAT_SUGGEST_OK"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CHAT_SUGGEST_OK"));
}
