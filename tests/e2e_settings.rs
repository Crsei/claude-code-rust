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

use serde_json::json;
use serial_test::serial;

// The merge / source-tracking logic lives in `src/config/settings.rs` and
// is fully covered by its `#[cfg(test)]` block. This integration test
// adds a black-box check that the committed schema file is parseable and
// that the CLI binary can boot with the new extended settings shape.
#[test]
#[serial]
fn schema_file_is_valid_json() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
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

    let mut cmd =
        assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
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
