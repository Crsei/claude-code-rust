//! E2E tests that make REAL API calls using credentials from .env.
//!
//! These tests are IGNORED by default. They only run when:
//!   cargo test --test e2e_live_api -- --ignored
//!
//! Prerequisites:
//!   - F:\AIclassmanager\cc\rust\.env must contain valid API keys
//!   - Network access to the provider endpoints
//!   - F:\temp must exist as the workspace
//!
//! Each test is gated with `#[ignore]` so `cargo test` skips them.
//! Run selectively:
//!   cargo test --test e2e_live_api -- --ignored                    # all live tests
//!   cargo test --test e2e_live_api azure -- --ignored              # only azure
//!   cargo test --test e2e_live_api simple_question -- --ignored    # single test
//!
//! Test tiers:
//!   - Tier 1 (chat): Pure text Q&A, no tool use. Should always pass.
//!   - Tier 2 (tool): Require working tool execution pipeline.
//!     These may fail if the provider doesn't support tool_use properly
//!     (e.g. OpenAI-compat providers that don't forward tool schemas).

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::time::Duration;

const WORKSPACE: &str = r"F:\temp";

/// Timeout for simple chat tests (no tool use).
const CHAT_TIMEOUT_SECS: u64 = 60;

/// Timeout for tool-use tests (model calls tools, multiple round-trips).
const TOOL_TIMEOUT_SECS: u64 = 120;

/// Build a command that inherits the real .env by running from the project dir.
fn live_cli() -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(r"F:\AIclassmanager\cc\rust");
    cmd.timeout(Duration::from_secs(CHAT_TIMEOUT_SECS));
    cmd
}

/// Build a command configured for tool-use tests (longer timeout, bypass perms).
fn tool_cli() -> Command {
    let mut cmd = Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.current_dir(r"F:\AIclassmanager\cc\rust");
    cmd.timeout(Duration::from_secs(TOOL_TIMEOUT_SECS));
    cmd
}

/// Cleanup helper: remove a file, ignoring errors.
fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

// =========================================================================
//  TIER 1: Chat tests (no tool use, pure text Q&A)
// =========================================================================

#[test]
#[ignore]
fn t1_simple_question_returns_answer() {
    live_cli()
        .args(["-p", "-C", WORKSPACE, "What is 2+2? Reply with just the number."])
        .assert()
        .success()
        .stdout(predicate::str::contains("4"));
}

#[test]
#[ignore]
fn t1_simple_chinese_question() {
    live_cli()
        .args(["-p", "-C", WORKSPACE, "1+1等于几？只回复数字"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
#[ignore]
fn t1_say_exact_phrase() {
    live_cli()
        .args(["-p", "-C", WORKSPACE, "Say exactly: HELLO_TEST_OK"])
        .assert()
        .success()
        .stdout(predicate::str::contains("HELLO_TEST_OK"));
}

#[test]
#[ignore]
fn t1_custom_system_prompt() {
    live_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--system-prompt", "You are a calculator. Only output numbers, nothing else.",
            "What is 10 times 5?",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("50"));
}

#[test]
#[ignore]
fn t1_append_system_prompt() {
    live_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--append-system-prompt", "Always end your response with the word ENDMARK.",
            "Say hi.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ENDMARK"));
}

#[test]
#[ignore]
fn t1_max_turns_one() {
    live_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--max-turns", "1",
            "What is 2+3? Reply with just the number.",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("5"));
}

#[test]
#[ignore]
fn t1_print_mode_clean_text() {
    let output = live_cli()
        .args(["-p", "-C", WORKSPACE, "Say exactly: CLEAN_OUTPUT_TEST"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CLEAN_OUTPUT_TEST"), "stdout: {}", stdout);
    // Should not leak raw tool_use JSON
    assert!(
        !stdout.contains("\"type\":\"tool_use\""),
        "raw tool_use JSON leaked: {}",
        stdout
    );
}

#[test]
#[ignore]
fn t1_env_file_provides_working_credentials() {
    live_cli()
        .args(["-p", "-C", WORKSPACE, "Reply with exactly: ENV_AUTH_OK"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ENV_AUTH_OK"));
}

// =========================================================================
//  TIER 2: Tool-use tests (require working tool execution pipeline)
// =========================================================================

#[test]
#[ignore]
fn t2_bash_echo() {
    tool_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--permission-mode", "bypass",
            "Use the Bash tool to run: echo TOOL_WORKS_OK",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("TOOL_WORKS_OK"));
}

#[test]
#[ignore]
fn t2_bash_pwd_shows_workspace() {
    tool_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--permission-mode", "bypass",
            "Use the Bash tool to run: pwd",
        ])
        .assert()
        .success()
        // pwd on Windows/MSYS may show /f/temp or F:\temp
        .stdout(predicate::str::contains("temp"));
}

#[test]
#[ignore]
fn t2_read_file() {
    let test_file = Path::new(WORKSPACE).join("_e2e_read_test.txt");
    std::fs::write(&test_file, "CANARY_READ_12345").expect("write test file");

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "Use the Read tool to read the file at the absolute path {}. Show me the exact contents.",
                    test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("CANARY_READ_12345"));
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_write_file() {
    let test_file = Path::new(WORKSPACE).join("_e2e_write_test.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "Use the Write tool to write the file at absolute path {} with exactly this content: WRITE_TEST_67890",
                    test_file.display()
                ),
            ])
            .assert()
            .success();

        // Verify on disk
        assert!(test_file.exists(), "file was not created");
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(
            content.contains("WRITE_TEST_67890"),
            "unexpected content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_edit_file() {
    let test_file = Path::new(WORKSPACE).join("_e2e_edit_test.txt");
    std::fs::write(&test_file, "Hello OLD_VALUE World").unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "First use Read to read {path}, then use Edit to replace 'OLD_VALUE' with 'NEW_VALUE' in {path}.",
                    path = test_file.display()
                ),
            ])
            .assert()
            .success();

        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(
            content.contains("NEW_VALUE"),
            "edit did not apply, content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_glob_finds_files() {
    let dir = Path::new(WORKSPACE);
    let file_a = dir.join("_e2e_glob_a.txt");
    let file_b = dir.join("_e2e_glob_b.txt");
    std::fs::write(&file_a, "a").unwrap();
    std::fs::write(&file_b, "b").unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "Use the Glob tool with pattern '_e2e_glob_*.txt' and path '{}'. List the files found.",
                    WORKSPACE
                ),
            ])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("_e2e_glob_a")
                    .and(predicate::str::contains("_e2e_glob_b")),
            );
    });

    cleanup(&file_a);
    cleanup(&file_b);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_grep_searches_content() {
    let test_file = Path::new(WORKSPACE).join("_e2e_grep_test.txt");
    std::fs::write(
        &test_file,
        "line1: nothing here\nline2: NEEDLE_FOUND_42\nline3: also nothing\n",
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "Use the Grep tool to search for the pattern 'NEEDLE_FOUND' in the file at absolute path {}. Show me the matching line.",
                    test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("NEEDLE_FOUND_42"));
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_multi_tool_write_read_edit() {
    let test_file = Path::new(WORKSPACE).join("_e2e_multi_test.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        tool_cli()
            .args([
                "-p",
                "-C", WORKSPACE,
                "--permission-mode", "bypass",
                &format!(
                    "Do these steps in order using tools:\n\
                     1. Write the file {path} with content 'STEP1_DONE'\n\
                     2. Read {path} to confirm\n\
                     3. Edit {path}: replace 'STEP1_DONE' with 'ALL_STEPS_COMPLETE'\n\
                     4. Read {path} again and show the final content.",
                    path = test_file.display()
                ),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("ALL_STEPS_COMPLETE"));

        let content = std::fs::read_to_string(&test_file).unwrap_or_default();
        assert!(
            content.contains("ALL_STEPS_COMPLETE"),
            "file content: {}",
            content
        );
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
#[ignore]
fn t2_read_nonexistent_file_graceful() {
    tool_cli()
        .args([
            "-p",
            "-C", WORKSPACE,
            "--permission-mode", "bypass",
            r"Use the Read tool to read F:\temp\_this_file_does_not_exist_xyz.txt and tell me what error you get.",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("not found")
                .or(predicate::str::contains("does not exist"))
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("Error"))
                .or(predicate::str::contains("No such file"))
                .or(predicate::str::contains("cannot")),
        );
}
