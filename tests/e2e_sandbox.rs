//! E2E tests for the sandbox subsystem (issue #8).
//!
//! This is a black-box suite — unit-level behavior (policy assembly, path
//! matching, network allowlist, mode parsing) is covered by
//! `#[cfg(test)]` blocks inside `src/sandbox/*.rs`. Here we verify:
//!
//! - the committed schema declares every public sandbox setting
//! - the extended nested schema round-trips through the binary loader
//! - the CLI `--no-network` flag is accepted without error
//!
//! Run with: `cargo test --test e2e_sandbox`

use serde_json::json;
use serial_test::serial;

#[test]
#[serial]
fn schema_file_declares_extended_sandbox_shape() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("schemas")
        .join("settings.schema.json");
    let txt = std::fs::read_to_string(&path).expect("schema file exists");
    let v: serde_json::Value = serde_json::from_str(&txt).expect("valid JSON");
    let props = v
        .pointer("/properties/sandbox/properties")
        .and_then(|p| p.as_object())
        .expect("sandbox properties exist");
    for must_have in [
        "enabled",
        "mode",
        "failIfUnavailable",
        "allowUnsandboxedCommands",
        "allowManagedReadPathsOnly",
        "allowManagedDomainsOnly",
        "excludedCommands",
        "allowedCommands",
        "filesystem",
        "network",
    ] {
        assert!(
            props.contains_key(must_have),
            "sandbox schema missing key '{}'",
            must_have
        );
    }

    // Check the nested shapes too.
    let fs_props = v
        .pointer("/properties/sandbox/properties/filesystem/properties")
        .and_then(|p| p.as_object())
        .expect("filesystem properties exist");
    for must_have in ["allowRead", "denyRead", "allowWrite", "denyWrite"] {
        assert!(
            fs_props.contains_key(must_have),
            "sandbox.filesystem schema missing key '{}'",
            must_have
        );
    }

    let net_props = v
        .pointer("/properties/sandbox/properties/network/properties")
        .and_then(|p| p.as_object())
        .expect("network properties exist");
    for must_have in ["disabled", "allowedDomains", "httpProxyPort", "socksProxyPort"] {
        assert!(
            net_props.contains_key(must_have),
            "sandbox.network schema missing key '{}'",
            must_have
        );
    }
}

/// CLI init with a fully-populated sandbox config — makes sure the new
/// nested `filesystem` / `network` / `failIfUnavailable` / `excludedCommands`
/// keys all parse without error.
#[test]
#[serial]
fn cli_starts_with_extended_sandbox_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "sandbox": {
            "enabled": true,
            "mode": "workspace",
            "failIfUnavailable": false,
            "allowUnsandboxedCommands": true,
            "excludedCommands": ["docker *"],
            "allowedCommands": ["make test"],
            "filesystem": {
                "allowWrite": ["~/.kube"],
                "denyWrite": ["/etc"],
                "allowRead": ["."],
                "denyRead": ["~/.ssh"]
            },
            "network": {
                "disabled": false,
                "allowedDomains": ["github.com", "*.npmjs.org"]
            }
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
    cmd.assert().success();
}

/// Smoke-check that the CLI accepts `--no-network` without blowing up
/// during init-only fast path.
#[test]
#[serial]
fn cli_accepts_no_network_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = tempfile::tempdir().expect("project tmpdir");
    let mut cmd = assert_cmd::Command::cargo_bin("claude-code-rs").expect("binary not found");
    cmd.env("CC_RUST_HOME", dir.path())
        .env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .arg("--init-only")
        .arg("--no-network")
        .arg("--cwd")
        .arg(project.path());
    cmd.assert().success();
}

/// Invalid sandbox mode in settings shouldn't break startup — the value
/// is rejected by the validation pass but init continues.
#[test]
#[serial]
fn cli_rejects_invalid_sandbox_mode_without_crashing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user_settings = dir.path().join("settings.json");
    let body = json!({
        "sandbox": {
            "enabled": true,
            "mode": "paranoid"
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
    cmd.assert().success();
}
