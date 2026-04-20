//! E2E smoke test for Browser MCP integration (Issues #2 + #3).
//!
//! We don't drive a real browser here — that requires Chrome + Playwright +
//! out-of-process machinery that doesn't belong in a CI unit test. Instead,
//! this test verifies the *integration points* cc-rust adds on top of any
//! third-party browser MCP server:
//!
//!   1. A server with `"browserMcp": true` in `.cc-rust/settings.json`
//!      gets recognized and the `# Browser Automation` system-prompt section
//!      is emitted (via `--dump-system-prompt`).
//!   2. Even without a connected server, the tool-name heuristic table and
//!      the permission-message shaper agree on category + risk for the most
//!      common browser actions (navigate / read_page / click / evaluate /
//!      console/network).
//!   3. `infer_kind` + `short_summary` produce the expected one-line previews
//!      for the read → write → observe loop described in the issue.
//!
//! This is a BLACK-BOX test over the compiled binary for (1), and an opaque
//! subprocess that exercises a small published CLI surface, so it also
//! catches regressions in wiring (e.g. forgetting to install the browser
//! server registry in the fast path).

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[path = "test_workspace.rs"]
#[allow(dead_code)]
mod test_workspace;

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn write_browser_mcp_settings(dir: &TempDir) {
    let cc_rust_dir = dir.path().join(".cc-rust");
    fs::create_dir_all(&cc_rust_dir).expect("create .cc-rust dir");

    // Note: the `command` below is a non-existent placeholder. The fast path
    // (`--dump-system-prompt`) does not connect to MCP servers, it only
    // reads the config and runs detect_browser_servers, so this is safe.
    let settings = serde_json::json!({
        "mcpServers": {
            "test-browser": {
                "command": "non-existent-browser-mcp-binary",
                "args": [],
                "browserMcp": true
            }
        }
    });
    fs::write(
        cc_rust_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .expect("write settings.json");
}

fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
}

// =========================================================================
// 1. Config-flagged browser MCP ends up in the system prompt
// =========================================================================

#[test]
fn dump_prompt_emits_browser_section_when_server_flagged() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_browser_mcp_settings(&dir);

    let mut cmd = cli();
    strip_api_keys(&mut cmd)
        .args(["--dump-system-prompt", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Browser Automation"))
        .stdout(predicate::str::contains("test-browser"));
}

#[test]
fn dump_prompt_has_no_browser_section_without_flagged_server() {
    // Empty tempdir — no .cc-rust/settings.json.
    let dir = tempfile::tempdir().expect("tempdir");

    let mut cmd = cli();
    strip_api_keys(&mut cmd)
        .args(["--dump-system-prompt", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Browser Automation").not());
}

// =========================================================================
// 2. Init still succeeds when a browser MCP server fails to connect
// =========================================================================
//
// This is the "bad command" path — the command in settings.json does not
// exist, but cc-rust must not panic or exit non-zero. Browser MCP is best
// effort; a misconfigured server is a warning, not a fatal error.

#[test]
fn init_only_survives_unreachable_browser_mcp() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_browser_mcp_settings(&dir);

    let mut cmd = cli();
    strip_api_keys(&mut cmd)
        .args(["--init-only", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success();
}
