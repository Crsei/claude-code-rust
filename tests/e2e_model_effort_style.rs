//! E2E tests for issue #9: model restriction, effort → thinking budget,
//! output_style and language injection into the system prompt.
//!
//! These tests are hermetic: they pin `CC_RUST_HOME` to a per-test
//! tempdir and use `--dump-system-prompt` to inspect the assembled
//! prompt without needing any API access.
//!
//! Run with: `cargo test --test e2e_model_effort_style`

use serde_json::json;
use serial_test::serial;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn strip_keys(cmd: &mut assert_cmd::Command) -> &mut assert_cmd::Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
}

#[test]
#[serial]
fn dump_prompt_injects_language_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = json!({ "language": "Chinese" });
    std::fs::write(
        dir.path().join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();

    let mut cmd = cli();
    let assert = strip_keys(&mut cmd)
        .args(["--dump-system-prompt", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success();

    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        out.contains("# Language"),
        "expected language section, got prompt without it"
    );
    assert!(out.contains("Chinese"), "language name missing");
}

#[test]
#[serial]
fn dump_prompt_injects_explanatory_output_style() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = json!({ "outputStyle": "explanatory" });
    std::fs::write(
        dir.path().join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();

    let mut cmd = cli();
    let assert = strip_keys(&mut cmd)
        .args(["--dump-system-prompt", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success();

    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        out.contains("# Output Style: Explanatory"),
        "expected explanatory output style header in prompt"
    );
}

#[test]
#[serial]
fn dump_prompt_loads_custom_output_style_from_project_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = tempfile::tempdir().expect("project tmpdir");
    let styles_dir = project.path().join(".cc-rust/output-styles");
    std::fs::create_dir_all(&styles_dir).unwrap();
    std::fs::write(
        styles_dir.join("brevity.md"),
        "Be ruthlessly concise. One sentence per response.",
    )
    .unwrap();

    let settings = json!({ "outputStyle": "brevity" });
    std::fs::write(
        dir.path().join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();

    let mut cmd = cli();
    let assert = strip_keys(&mut cmd)
        .args([
            "--dump-system-prompt",
            "-C",
            project.path().to_str().unwrap(),
        ])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success();

    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        out.contains("# Output Style: brevity"),
        "expected custom output style header"
    );
    assert!(
        out.contains("ruthlessly concise"),
        "expected custom style body in prompt"
    );
}

#[test]
#[serial]
fn dump_prompt_omits_language_and_output_style_when_unset() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No settings.json at all.

    let mut cmd = cli();
    let assert = strip_keys(&mut cmd)
        .args(["--dump-system-prompt", "-C", dir.path().to_str().unwrap()])
        .env("CC_RUST_HOME", dir.path().to_str().unwrap())
        .assert()
        .success();

    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        !out.contains("# Language"),
        "language section should not appear without language setting"
    );
    assert!(
        !out.contains("# Output Style"),
        "output style section should not appear without outputStyle setting"
    );
}
