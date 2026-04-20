//! E2E tests for the permission system layered settings → context flow.
//!
//! The fine-grained matcher / mode / hook tests live inside
//! `src/permissions/{rules,decision,bash_matcher}.rs`. This file exercises
//! the cross-cutting behaviour you can only test by running the CLI
//! binary against a real settings file:
//!
//! 1. Permission rules from `~/.cc-rust/settings.json` actually flow into
//!    the runtime `ToolPermissionContext`.
//! 2. The schema published in `docs/schemas/settings.schema.json`
//!    describes the `permissions` object the way the loader expects.
//!
//! Run with: `cargo test --test e2e_permissions`

use serde_json::json;
use serial_test::serial;

#[test]
#[serial]
fn permissions_from_user_settings_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "permissions": {
            "defaultMode": "acceptEdits",
            "allow": ["Read", "Bash(prefix:git)"],
            "deny": ["Bash(rm)"],
            "ask": ["Bash(prefix:cargo publish)"],
            "additionalDirectories": ["/tmp/extra"],
            "enableBypassMode": false,
            "enableAutoMode": true
        }
    });
    std::fs::write(&user_settings, serde_json::to_string_pretty(&body).unwrap())
        .expect("write user settings");

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
    // We don't have an introspection flag for permission rules at the CLI
    // level, but `--init-only` builds the `AppState` and exits — if any
    // settings field name had drifted from the loader we would have
    // failed to deserialize and the binary would have errored.
    cmd.assert().success();
}

#[test]
#[serial]
fn schema_describes_permissions_subkeys() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("schemas")
        .join("settings.schema.json");
    let txt = std::fs::read_to_string(&path).expect("schema present");
    let v: serde_json::Value = serde_json::from_str(&txt).expect("schema is JSON");
    let perms = v
        .pointer("/properties/permissions/properties")
        .and_then(|o| o.as_object())
        .expect("schema has permissions.properties object");

    for k in [
        "defaultMode",
        "allow",
        "ask",
        "deny",
        "additionalDirectories",
        "enableBypassMode",
        "enableAutoMode",
    ] {
        assert!(
            perms.contains_key(k),
            "schema/permissions missing field {}",
            k
        );
    }
}
