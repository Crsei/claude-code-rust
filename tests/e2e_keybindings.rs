//! E2E tests for the keybinding subsystem (issue #10).
//!
//! Black-box checks:
//! - committed schema file declares the expected properties
//! - CLI `--init-only` accepts a user keybindings file without error
//! - CLI `--init-only` tolerates a malformed keybindings file (the user's
//!   TUI must boot even when the config is broken)
//!
//! Run with: `cargo test --test e2e_keybindings`

use serde_json::json;
use serial_test::serial;

#[test]
#[serial]
fn schema_file_declares_expected_properties() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("schemas")
        .join("keybindings.schema.json");
    assert!(
        path.exists(),
        "expected schema at {}",
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
    for must_have in ["$schema", "$docs", "bindings"] {
        assert!(
            props.contains_key(must_have),
            "keybindings schema missing key '{}'",
            must_have
        );
    }

    // Check the context enum covers the spec list.
    let enum_vals = v
        .pointer("/$defs/bindingBlock/properties/context/enum")
        .and_then(|e| e.as_array())
        .expect("context enum");
    let names: Vec<&str> = enum_vals.iter().filter_map(|x| x.as_str()).collect();
    for required in [
        "Global",
        "Chat",
        "Autocomplete",
        "Confirmation",
        "Transcript",
        "HistorySearch",
        "Scroll",
    ] {
        assert!(
            names.contains(&required),
            "context enum missing '{}'",
            required
        );
    }
}

/// CLI init-only should accept a well-formed user keybindings file.
#[test]
#[serial]
fn cli_init_only_accepts_user_keybindings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let kb_path = dir.path().join("keybindings.json");
    let body = json!({
        "$schema": "https://cc-rust/keybindings.schema.json",
        "bindings": [
            {
                "context": "Chat",
                "bindings": {
                    "ctrl+e": "chat:externalEditor",
                    "ctrl+u": null
                }
            },
            {
                "context": "Scroll",
                "bindings": {
                    "ctrl+shift+c": "selection:copy"
                }
            }
        ]
    });
    std::fs::write(&kb_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write keybindings");

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
    cmd.assert().success();
}

/// A malformed user keybindings file must NOT prevent startup — the
/// registry logs the issue and falls back to the defaults.
#[test]
#[serial]
fn cli_init_only_tolerates_malformed_keybindings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let kb_path = dir.path().join("keybindings.json");
    std::fs::write(&kb_path, "{ this is not json").expect("write keybindings");

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
    cmd.assert().success();
}

/// `editorMode: vim` in settings should not cause init errors. The Rust
/// TUI consumes `editorMode` via `AppState.settings.editor_mode`; the e2e
/// test just confirms the init path tolerates a vim profile.
#[test]
#[serial]
fn cli_init_only_accepts_editor_mode_vim() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    let body = json!({ "editorMode": "vim" });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&body).unwrap())
        .expect("write settings");

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
    cmd.assert().success();
}
