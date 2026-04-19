//! Compatibility-focused integration tests for voice configuration.
//!
//! These tests verify that startup accepts the persisted voice config
//! surface even though this build does not provide real recording or
//! transcription.
//!
//! Black-box checks:
//!
//! - CLI `--init-only` accepts `voiceEnabled: true` in `settings.json`.
//! - CLI `--init-only` accepts `voiceEnabled: false` / default settings.
//! - CLI `--init-only` accepts a user-remapped `voice:pushToTalk`
//!   keybinding without errors.
//! - CLI `--init-only` accepts unsupported `language` values because
//!   normalization is a runtime concern, not a startup failure.
//!
//! Run with: `cargo test --test e2e_voice`.

use serde_json::json;
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
        .env_remove("CC_RUST_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    customize(&mut cmd);
    cmd.assert().success();
}

#[test]
#[serial]
fn cli_init_only_accepts_voice_enabled_setting_for_compatibility() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let settings_path = tmp.path().join("settings.json");
    let body = json!({
        "voiceEnabled": true,
        "language": "en-US"
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", tmp.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CC_RUST_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

#[test]
#[serial]
fn cli_init_only_accepts_voice_disabled_setting() {
    run_init_only(|_| {
        // Default settings keep voice disabled and must still boot cleanly.
    });
}

#[test]
#[serial]
fn cli_init_only_accepts_explicitly_disabled_voice_enabled_flag() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let settings_path = tmp.path().join("settings.json");
    let body = json!({
        "voiceEnabled": false,
        "language": "fr-CA"
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", tmp.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CC_RUST_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

#[test]
#[serial]
fn cli_init_only_accepts_remapped_push_to_talk_keybinding() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let kb_path = tmp.path().join("keybindings.json");
    let body = json!({
        "$schema": "https://cc-rust/keybindings.schema.json",
        "bindings": [
            {
                "context": "Chat",
                "bindings": {
                    "meta+v": "voice:pushToTalk",
                    "ctrl+space": null
                }
            }
        ]
    });
    std::fs::write(&kb_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write keybindings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", tmp.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CC_RUST_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

#[test]
#[serial]
fn cli_init_only_tolerates_unsupported_dictation_language() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let settings_path = tmp.path().join("settings.json");
    let body = json!({
        "language": "klingon"
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", tmp.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CC_RUST_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE")
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}
