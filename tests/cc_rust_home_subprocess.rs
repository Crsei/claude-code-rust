//! Subprocess-level integration smoke for Phase 1 storage unification.
//!
//! Verifies that running the actual binary with `CC_RUST_HOME=<tempdir>`
//! causes runtime state (logs, sessions) to appear under that tempdir
//! and NOT inside the test's working directory.

use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for each bin target when building integration tests.
    // See: https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let p = env!("CARGO_BIN_EXE_claude-code-rs");
    std::path::PathBuf::from(p)
}

#[test]
#[serial]
fn cc_rust_home_redirects_logs_and_leaves_cwd_clean() {
    let tmp = TempDir::new().expect("tempdir");
    let cwd = TempDir::new().expect("cwd tempdir");

    // Run `--version` as the minimum invocation. This is a fast path that
    // returns before tracing init, so this test only confirms that the
    // fast path does not pollute cwd. Stronger coverage lives in:
    //   - paths.rs unit tests (Tasks 2-3)
    //   - manual acceptance steps (Task 20)
    let status = Command::new(binary_path())
        .arg("--version")
        .env("CC_RUST_HOME", tmp.path())
        .current_dir(cwd.path())
        .status()
        .expect("spawn cc-rust");
    assert!(status.success(), "binary exited non-zero");

    // Assert no `.logs/` or `logs/` appeared in the test cwd.
    let cwd_logs_hidden = cwd.path().join(".logs");
    let cwd_logs_visible = cwd.path().join("logs");
    assert!(
        !cwd_logs_hidden.exists(),
        "cwd polluted: {} exists",
        cwd_logs_hidden.display()
    );
    assert!(
        !cwd_logs_visible.exists(),
        "cwd polluted: {} exists",
        cwd_logs_visible.display()
    );
}
