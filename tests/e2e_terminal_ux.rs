//! E2E tests for the terminal UX polish (issue #12).
//!
//! Black-box checks:
//!
//! - CLI `--init-only` accepts each of the three `CLAUDE_CODE_*` env
//!   toggles without crashing (NO_FLICKER, DISABLE_MOUSE, SCROLL_SPEED).
//! - Garbage values still boot cleanly (parser falls back to defaults).
//!
//! Unit-level behaviour (env parsing / clamping, transcript state machine,
//! search, welcome-height tiers, resize cache invalidation) lives in
//! `src/ui/terminal_env.rs`, `src/ui/transcript.rs`,
//! `src/ui/virtual_scroll.rs`, `src/ui/welcome.rs`, and
//! `src/commands/terminal_setup.rs`.
//!
//! Run with: `cargo test --test e2e_terminal_ux`

use serial_test::serial;

fn run_init_only<F>(customize: F)
where
    F: FnOnce(&mut assert_cmd::Command),
{
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
    customize(&mut cmd);
    cmd.assert().success();
}

#[test]
#[serial]
fn cli_init_only_accepts_no_flicker_env() {
    for value in ["0", "1", "true", "false"] {
        run_init_only(|cmd| {
            cmd.env("CLAUDE_CODE_NO_FLICKER", value);
        });
    }
}

#[test]
#[serial]
fn cli_init_only_accepts_disable_mouse_env() {
    run_init_only(|cmd| {
        cmd.env("CLAUDE_CODE_DISABLE_MOUSE", "1");
    });
}

#[test]
#[serial]
fn cli_init_only_accepts_scroll_speed_env() {
    for value in ["3", "15", "50"] {
        run_init_only(|cmd| {
            cmd.env("CLAUDE_CODE_SCROLL_SPEED", value);
        });
    }
}

/// Parser should recover from garbage values — startup must not fail.
#[test]
#[serial]
fn cli_init_only_tolerates_bogus_terminal_env_values() {
    run_init_only(|cmd| {
        cmd.env("CLAUDE_CODE_NO_FLICKER", "maybe-later");
        cmd.env("CLAUDE_CODE_DISABLE_MOUSE", "???");
        cmd.env("CLAUDE_CODE_SCROLL_SPEED", "banana");
    });
}

/// Setting all three together should still boot without errors.
#[test]
#[serial]
fn cli_init_only_accepts_all_three_env_toggles_together() {
    run_init_only(|cmd| {
        cmd.env("CLAUDE_CODE_NO_FLICKER", "1")
            .env("CLAUDE_CODE_DISABLE_MOUSE", "0")
            .env("CLAUDE_CODE_SCROLL_SPEED", "8");
    });
}
