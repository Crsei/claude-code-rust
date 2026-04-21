//! e2e tests for `/plan` slash command (issue #46).
//!
//! `claude-code-rs` is a binary crate with no `lib.rs`, so full handler
//! exercising lives in the bin-internal unit tests. Here we verify the
//! externally-observable contract: registry wiring, source-level module
//! layout, and cross-crate path-helper surface.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read_source(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

#[test]
fn plan_command_module_is_declared() {
    let src = read_source("crates/claude-code-rs/src/commands/mod.rs");
    assert!(
        src.contains("pub mod plan;"),
        "commands::plan module must be declared in commands/mod.rs"
    );
}

#[test]
fn plan_command_is_registered_in_get_all_commands() {
    let src = read_source("crates/claude-code-rs/src/commands/mod.rs");
    assert!(
        src.contains("name: \"plan\".into(),"),
        "/plan command must have a registry entry"
    );
    assert!(
        src.contains("plan::PlanHandler"),
        "/plan entry must wire PlanHandler"
    );
}

#[test]
fn plan_handler_covers_expected_subcommands() {
    let src = read_source("crates/claude-code-rs/src/commands/plan.rs");
    for token in [
        "\"show\"",
        "\"view\"",
        "\"open\"",
        "\"edit\"",
        "\"path\"",
    ] {
        assert!(
            src.contains(token),
            "/plan handler must dispatch on subcommand {}",
            token
        );
    }
}

#[test]
fn plan_handler_uses_pre_plan_mode_handshake() {
    let src = read_source("crates/claude-code-rs/src/commands/plan.rs");
    assert!(
        src.contains("pre_plan_mode"),
        "/plan handler must save pre_plan_mode so ExitPlanMode can restore"
    );
    assert!(
        src.contains("PermissionMode::Plan"),
        "/plan handler must set mode to PermissionMode::Plan"
    );
}

#[test]
fn plan_path_helpers_are_publicly_exposed() {
    let src = read_source("crates/cc-config/src/paths.rs");
    for fn_name in [
        "pub fn plan_file_path_project",
        "pub fn plan_file_path_global",
        "pub fn current_plan_file_path",
    ] {
        assert!(
            src.contains(fn_name),
            "cc-config::paths must expose `{}`",
            fn_name
        );
    }
}

#[test]
fn plan_handler_delegates_to_external_editor_util() {
    let src = read_source("crates/claude-code-rs/src/commands/plan.rs");
    assert!(
        src.contains("ensure_and_open"),
        "/plan open must delegate to the shared ensure_and_open editor util"
    );
    assert!(
        src.contains("format_open_outcome"),
        "/plan open must format the OpenOutcome for the user"
    );
}
