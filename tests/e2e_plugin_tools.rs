//! E2E coverage for plugin-contributed executable tools.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::{Builder, TempDir};

#[path = "test_workspace.rs"]
mod test_workspace;

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn workspace() -> &'static str {
    test_workspace::workspace()
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

fn configure_fake_home<'a>(cmd: &'a mut Command, home: &TempDir) -> &'a mut Command {
    let cc_rust_home = home.path().join(".cc-rust");
    let cc_rust_home = cc_rust_home.to_string_lossy().to_string();
    cmd.env("CC_RUST_HOME", &cc_rust_home);
    cmd
}

fn write_plugin_fixture(home: &TempDir) {
    fs::create_dir_all(home.path().join(".cc-rust").join("logs"))
        .expect("create fake global log directory");

    let plugin_dir = home
        .path()
        .join(".cc-rust")
        .join("plugins")
        .join("cache")
        .join("local")
        .join("test-plugin")
        .join("1.0.0");
    fs::create_dir_all(&plugin_dir).expect("create plugin cache directory");

    let runtime = if cfg!(windows) {
        json!({
            "type": "stdio",
            "command": "cmd",
            "args": ["/d", "/s", "/c", "more"]
        })
    } else {
        json!({
            "type": "stdio",
            "command": "sh",
            "args": ["-c", "cat"]
        })
    };

    let plugin_manifest = json!({
        "name": "test-plugin",
        "version": "1.0.0",
        "description": "Fixture plugin",
        "tools": [
            {
                "name": "plugin_echo",
                "description": "Echo JSON input from stdin",
                "read_only": true,
                "runtime": runtime
            }
        ]
    });
    fs::write(
        plugin_dir.join("plugin.json"),
        serde_json::to_string_pretty(&plugin_manifest).expect("serialize plugin manifest"),
    )
    .expect("write plugin manifest");

    let installed_plugins = json!({
        "version": 2,
        "plugins": [
            {
                "id": "test-plugin@local",
                "name": "Test Plugin",
                "version": "1.0.0",
                "description": "Fixture plugin",
                "source": {
                    "source": "local",
                    "path": plugin_dir.to_string_lossy()
                },
                "status": "Installed",
                "marketplace": "local",
                "cache_path": plugin_dir.to_string_lossy(),
                "tools": ["plugin_echo"],
                "skills": [],
                "mcp_servers": [],
                "installed_at": null,
                "updated_at": null
            }
        ]
    });

    let installed_path = home
        .path()
        .join(".cc-rust")
        .join("plugins")
        .join("installed_plugins.json");
    fs::create_dir_all(
        installed_path
            .parent()
            .expect("installed_plugins.json should have parent"),
    )
    .expect("create plugins directory");
    fs::write(
        installed_path,
        serde_json::to_string_pretty(&installed_plugins).expect("serialize installed plugins"),
    )
    .expect("write installed_plugins.json");
}

#[test]
fn dump_system_prompt_includes_plugin_runtime_tool() {
    let home = Builder::new()
        .prefix("plugin-home-")
        .tempdir_in(std::env::current_dir().expect("resolve current dir"))
        .expect("create temp home");
    write_plugin_fixture(&home);

    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    configure_fake_home(&mut cmd, &home);

    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("## plugin_echo")
                .or(predicate::str::contains("\"plugin_echo\"")),
        );
}
