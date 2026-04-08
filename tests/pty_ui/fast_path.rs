//! Fast-path tests: commands that exit immediately without entering TUI.

use crate::harness::*;

/// `--version` prints version string and exits.
#[test]
fn version_flag() {
    let session = PtySession::spawn(&["-V"], 120, 40, false);
    let output = session.finish(QUICK_TIMEOUT, "fp_version");

    assert!(
        output.contains("claude-code-rs"),
        "should contain version, got: [{}]",
        output.text()
    );
}

/// `--init-only` initializes and exits without error.
#[test]
fn init_only() {
    let session = PtySession::spawn(&["--init-only"], 120, 40, false);
    let output = session.finish(QUICK_TIMEOUT, "fp_init_only");

    assert!(!output.contains("panicked"), "should not panic");

    let dir = logs_dir();
    assert!(dir.join("fp_init_only.raw").exists());
    assert!(dir.join("fp_init_only.log").exists());
}

/// `--dump-system-prompt` outputs the full system prompt.
#[test]
fn dump_system_prompt() {
    let session = PtySession::spawn(
        &["--dump-system-prompt", "-C", r"F:\temp"],
        200,
        50,
        false,
    );
    let output = session.finish(QUICK_TIMEOUT, "fp_dump_prompt");

    assert!(
        output.contains("tool") || output.contains("Tool"),
        "system prompt should mention tools, got {} bytes",
        output.plain.len()
    );
}

/// `--dump-system-prompt` with `--system-prompt` override includes custom text.
#[test]
fn dump_custom_system_prompt() {
    let session = PtySession::spawn(
        &[
            "--dump-system-prompt",
            "--system-prompt",
            "You are a PTY test bot.",
            "-C",
            r"F:\temp",
        ],
        200,
        50,
        false,
    );
    let output = session.finish(QUICK_TIMEOUT, "fp_dump_custom_prompt");

    assert!(
        output.contains("PTY test bot"),
        "should contain custom prompt, got: [{}]",
        &output.text()[..output.text().len().min(200)]
    );
}

/// `-p` print mode without API key reports an error (doesn't crash).
#[test]
fn print_mode_no_api_key() {
    let session = PtySession::spawn(
        &["-p", "hello", "-C", r"F:\temp"],
        120,
        40,
        true, // strip keys — testing "no key" error path
    );
    let output = session.finish(QUICK_TIMEOUT, "fp_print_no_key");

    let text = output.text().to_lowercase();
    assert!(
        text.contains("api") || text.contains("error") || text.contains("no"),
        "should report API error, got: [{}]",
        output.text()
    );
}

/// `-p` print mode captures response.
#[test]
fn print_mode_live() {
    let session = PtySession::spawn(
        &["-p", "Say exactly: PTY_PRINT_OK", "-C", r"F:\temp"],
        120,
        40,
        false,
    );
    let output = session.finish(API_TIMEOUT, "fp_print_live");

    assert!(
        output.contains("PTY_PRINT_OK"),
        "print mode should output response, got: [{}]",
        output.text()
    );
}
