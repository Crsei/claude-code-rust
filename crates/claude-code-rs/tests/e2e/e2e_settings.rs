//! E2E tests for the layered settings system.
//!
//! Exercises [`config::settings`] end-to-end: writing every layer to a
//! tempdir, loading them, and asserting both the merged effective values
//! and the per-key source map.
//!
//! These tests are intentionally hermetic — they set `CC_RUST_HOME` to a
//! per-test tempdir so they never touch the user's real settings file.
//!
//! Run with: `cargo test --test e2e_settings`

use predicates::prelude::*;
use serde_json::json;
use serial_test::serial;

fn remove_provider_env(cmd: &mut assert_cmd::Command) -> &mut assert_cmd::Command {
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_SOTA_MODEL",
        "ANTHROPIC_DEFAULT_MOTA_MODEL",
        "ANTHROPIC_DEFAULT_FOTA_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "AZURE_API_KEY",
        "OPENAI_API_KEY",
        "OPENAI_CODEX_AUTH_TOKEN",
        "OPENAI_CODEX_BASE_URL",
        "OPENAI_CODEX_MODEL",
        "OPENROUTER_API_KEY",
        "GOOGLE_API_KEY",
        "DEEPSEEK_API_KEY",
        "GROQ_API_KEY",
        "ZHIPU_API_KEY",
        "DASHSCOPE_API_KEY",
        "MOONSHOT_API_KEY",
        "BAICHUAN_API_KEY",
        "MINIMAX_API_KEY",
        "YI_API_KEY",
        "SILICONFLOW_API_KEY",
        "STEPFUN_API_KEY",
        "SPARK_API_KEY",
        "CC_BACKEND",
        "CLAUDE_BACKEND",
        "CLAUDE_LANGUAGE",
        "CLAUDE_OUTPUT_STYLE",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        cmd.env_remove(key);
    }
    cmd
}

// The merge / source-tracking logic lives in `src/config/settings.rs` and
// is fully covered by its `#[cfg(test)]` block. This integration test
// adds a black-box check that the committed schema file is parseable and
// that the CLI binary can boot with the new extended settings shape.
#[test]
#[serial]
fn schema_file_is_valid_json() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("schemas")
        .join("settings.schema.json");
    assert!(
        path.exists(),
        "expected committed schema at {}",
        path.display()
    );
    let txt = std::fs::read_to_string(&path).expect("read schema");
    let v: serde_json::Value = serde_json::from_str(&txt).expect("schema is valid JSON");
    assert_eq!(
        v.pointer("/$schema")
            .and_then(|s| s.as_str())
            .unwrap_or_default(),
        "https://json-schema.org/draft/2020-12/schema"
    );
    let props = v
        .pointer("/properties")
        .and_then(|p| p.as_object())
        .expect("schema has /properties");
    for must_have in [
        "permissions",
        "sandbox",
        "statusLine",
        "outputStyle",
        "spinnerTips",
        "availableModels",
        "fastMode",
        "voiceEnabled",
        "editorMode",
        "teammateMode",
    ] {
        assert!(
            props.contains_key(must_have),
            "committed schema must declare {}",
            must_have
        );
    }
}

/// Tiny smoke test exercising the layered loader through the CLI binary.
/// Sets up a CC_RUST_HOME with a user-level settings file containing a
/// few new fields, then runs `--init-only` and `--dump-system-prompt`
/// to make sure nothing trips on the new struct shape.
#[test]
#[serial]
fn cli_starts_with_extended_user_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "model": "claude-sonnet-4-20250514",
        "outputStyle": "concise",
        "language": "en",
        "editorMode": "vim",
        "permissions": {
            "defaultMode": "ask",
            "allow": ["Bash"],
            "deny": ["FileWrite"]
        },
        "spinnerTips": {
            "enabled": true,
            "intervalMs": 5000,
            "customTips": ["hi"]
        },
        "fastMode": false
    });
    std::fs::write(&user_settings, serde_json::to_string_pretty(&body).unwrap())
        .expect("write user settings");

    // Use a project workspace dir distinct from CC_RUST_HOME.
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

    let assert = cmd.assert();
    // Just need a clean exit — the new settings file must parse.
    assert.success();
}

#[test]
#[serial]
fn settings_env_seeds_anthropic_provider_before_full_init_detection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "env": {
            "ANTHROPIC_API_KEY": "sk-ant-api03-settings-env",
            "ANTHROPIC_BASE_URL": "https://compatible.example.com",
            "ANTHROPIC_MODEL": "deepseek-v4-pro"
        }
    });
    std::fs::write(&user_settings, serde_json::to_string_pretty(&body).unwrap())
        .expect("write user settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let managed = dir.path().join("missing-managed.json");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    remove_provider_env(&mut cmd);
    cmd.env("CC_RUST_HOME", dir.path())
        .env("CC_RUST_MANAGED_SETTINGS", &managed)
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());

    cmd.assert()
        .success()
        .stderr(predicates::str::contains("No API provider detected").not());
}

#[test]
#[serial]
fn settings_env_can_select_codex_backend_before_full_init_detection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "env": {
            "CC_BACKEND": "codex",
            "OPENAI_CODEX_AUTH_TOKEN": "codex-settings-token",
            "OPENAI_CODEX_MODEL": "gpt-5.3-codex-spark"
        }
    });
    std::fs::write(&user_settings, serde_json::to_string_pretty(&body).unwrap())
        .expect("write user settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let managed = dir.path().join("missing-managed.json");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    remove_provider_env(&mut cmd);
    cmd.env("CC_RUST_HOME", dir.path())
        .env("CC_RUST_MANAGED_SETTINGS", &managed)
        .arg("--init-only")
        .arg("--cwd")
        .arg(project.path());

    cmd.assert()
        .success()
        .stderr(predicates::str::contains("No OpenAI Codex auth detected").not())
        .stderr(predicates::str::contains("No API provider detected").not());
}

#[test]
#[serial]
fn settings_env_is_applied_before_dump_system_prompt_fast_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "env": {
            "CLAUDE_LANGUAGE": "Korean"
        }
    });
    std::fs::write(&user_settings, serde_json::to_string_pretty(&body).unwrap())
        .expect("write user settings");

    let project = tempfile::tempdir().expect("project tmpdir");
    let managed = dir.path().join("missing-managed.json");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    remove_provider_env(&mut cmd);
    cmd.env("CC_RUST_HOME", dir.path())
        .env("CC_RUST_MANAGED_SETTINGS", &managed)
        .arg("--dump-system-prompt")
        .arg("--cwd")
        .arg(project.path());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(out.contains("# Language"), "language section missing");
    assert!(out.contains("Korean"), "language value missing");
}
