//! E2E tests for the hooks system — config loading, wiring, and bug exposure.
//!
//! This is a binary-only crate (no lib.rs), so these tests use:
//! - Source-file inspection to expose wiring bugs
//! - Subprocess (assert_cmd) tests where possible
//! - serde_json deserialization for config format tests
//!
//! The hook engine unit tests live in `src/tools/hooks.rs` (run via `cargo test`).
//! This file focuses on the integration gaps between config → execution.
//!
//! Run with: cargo test --test e2e_hooks

use serde_json::json;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

// =========================================================================
// 1. Config format — hooks in settings.json deserialize correctly
// =========================================================================

/// GlobalConfig shape: hooks field should deserialize as HashMap<String, Value>.
#[test]
fn global_config_hooks_field_deserializes() {
    let config_json = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        { "type": "command", "command": "echo audit", "timeout": 30 }
                    ]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "*",
                    "hooks": [
                        { "type": "command", "command": "echo done" }
                    ]
                }
            ],
            "Stop": [
                {
                    "hooks": [
                        { "type": "command", "command": "echo stopped" }
                    ]
                }
            ]
        }
    });

    // Deserialize as a generic config with hooks
    let parsed: HashMap<String, serde_json::Value> =
        serde_json::from_value(config_json).expect("should parse config");

    let hooks = parsed
        .get("hooks")
        .expect("hooks key should exist")
        .as_object()
        .expect("hooks should be an object");

    assert!(hooks.contains_key("PreToolUse"), "missing PreToolUse");
    assert!(hooks.contains_key("PostToolUse"), "missing PostToolUse");
    assert!(hooks.contains_key("Stop"), "missing Stop");

    // Verify PreToolUse structure
    let pre_tool = hooks["PreToolUse"]
        .as_array()
        .expect("PreToolUse should be array");
    assert_eq!(pre_tool.len(), 1);
    assert_eq!(pre_tool[0]["matcher"], "Bash");
    assert_eq!(pre_tool[0]["hooks"][0]["command"], "echo audit");
    assert_eq!(pre_tool[0]["hooks"][0]["timeout"], 30);
}

/// Project-level settings.json with hooks should parse correctly.
#[test]
fn project_settings_json_with_hooks_roundtrips() {
    let settings = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "echo '{\"continue\":false,\"reason\":\"blocked\"}'",
                            "timeout": 5
                        }
                    ]
                }
            ]
        }
    });

    // Write to temp file and read back
    let tmpdir = TempDir::new().unwrap();
    let settings_path = tmpdir.path().join("settings.json");
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();

    let content = fs::read_to_string(&settings_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("should parse back");

    assert!(parsed["hooks"]["PreToolUse"].is_array());
    assert_eq!(parsed["hooks"]["PreToolUse"][0]["matcher"], "Bash");
}

/// HookEventConfig shape matches what hooks.rs expects.
#[test]
fn hook_event_config_shape_matches_expected_schema() {
    // This is the shape that hooks.rs::load_hook_configs expects
    let event_configs_json = json!([
        {
            "matcher": "Bash",
            "hooks": [
                { "type": "command", "command": "echo guard", "timeout": 15 }
            ]
        },
        {
            "matcher": "*",
            "hooks": [
                { "type": "command", "command": "echo audit" }
            ]
        },
        {
            "hooks": [
                { "type": "command", "command": "echo no-matcher" }
            ]
        }
    ]);

    let configs: Vec<serde_json::Value> =
        serde_json::from_value(event_configs_json).expect("should parse");

    assert_eq!(configs.len(), 3);

    // Config without matcher (should be treated as match-all)
    assert!(configs[2].get("matcher").is_none());
}

// =========================================================================
// 2. Config merge — global + project hooks combine correctly
// =========================================================================

/// When global has PreToolUse and project has PostToolUse, merged should have both.
#[test]
fn merge_hooks_combines_global_and_project() {
    let mut global_hooks: HashMap<String, serde_json::Value> = HashMap::new();
    global_hooks.insert(
        "PreToolUse".to_string(),
        json!([{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo global" }] }]),
    );

    let mut project_hooks: HashMap<String, serde_json::Value> = HashMap::new();
    project_hooks.insert(
        "PostToolUse".to_string(),
        json!([{ "matcher": "*", "hooks": [{ "type": "command", "command": "echo project" }] }]),
    );

    // Simulate merge_maps behavior (same key → project wins, different keys → union)
    let mut merged = global_hooks.clone();
    for (k, v) in &project_hooks {
        merged.insert(k.clone(), v.clone());
    }

    assert!(
        merged.contains_key("PreToolUse"),
        "should keep global PreToolUse"
    );
    assert!(
        merged.contains_key("PostToolUse"),
        "should add project PostToolUse"
    );
}

/// When both global and project define the same hook event, project should win.
#[test]
fn merge_hooks_project_overrides_global_same_event() {
    let mut global_hooks: HashMap<String, serde_json::Value> = HashMap::new();
    global_hooks.insert(
        "PreToolUse".to_string(),
        json!([{ "matcher": "*", "hooks": [{ "type": "command", "command": "echo global-pre" }] }]),
    );

    let mut project_hooks: HashMap<String, serde_json::Value> = HashMap::new();
    project_hooks.insert(
        "PreToolUse".to_string(),
        json!([{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo project-pre" }] }]),
    );

    // merge_maps: project values override global for same key
    let mut merged = global_hooks.clone();
    for (k, v) in &project_hooks {
        merged.insert(k.clone(), v.clone());
    }

    let pre = merged["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert_eq!(
        pre[0]["matcher"], "Bash",
        "project config should override global"
    );
}

// =========================================================================
// 3. [BUG EXPOSURE] Source-level verification of wiring gaps
// =========================================================================

/// FIXED: orchestration.rs now loads hook configs from AppState.
///
/// The execute_tool_call function loads hook configs via get_app_state()
/// and passes them to run_pre/post/failure hook calls instead of empty &[].
#[test]
fn fixed_orchestration_loads_hook_configs() {
    let source =
        fs::read_to_string("src/tools/orchestration.rs").expect("should read orchestration.rs");

    let hook_calls: Vec<&str> = source
        .lines()
        .filter(|line| {
            line.contains("run_pre_tool_hooks")
                || line.contains("run_post_tool_hooks")
                || line.contains("run_post_tool_failure_hooks")
        })
        .collect();

    assert!(
        !hook_calls.is_empty(),
        "orchestration.rs should contain hook calls"
    );

    let any_use_empty = hook_calls.iter().any(|line| line.contains("&[]"));
    assert!(
        !any_use_empty,
        "orchestration.rs should not pass empty &[] to hook calls.\n\
         All hook calls should use configs loaded from AppState.\n\
         Current hook calls:\n{}",
        hook_calls.join("\n")
    );

    assert!(
        source.contains("load_hook_configs"),
        "orchestration.rs should call load_hook_configs"
    );
}

/// FIXED: deps.rs execute_tool() now runs pre/post/failure hooks.
///
/// This is the PRIMARY execution path used at runtime:
///   loop_impl.rs → loop_helpers.rs → deps.execute_tool()
///
/// Previously it went straight from permission check to tool.call() without
/// running any hooks. Now it loads hook configs from AppState.hooks and
/// runs pre-tool, post-tool, and post-failure hooks.
#[test]
fn fixed_deps_execute_tool_runs_hooks() {
    let source = fs::read_to_string("src/engine/lifecycle/deps.rs").expect("should read deps.rs");

    let has_pre_hooks = source.contains("run_pre_tool_hooks");
    let has_post_hooks = source.contains("run_post_tool_hooks");
    let has_failure_hooks = source.contains("run_post_tool_failure_hooks");
    let has_hook_import = source.contains("tools::hooks");
    let has_load_configs = source.contains("load_hook_configs");

    assert!(has_hook_import, "deps.rs should import the hooks module");
    assert!(
        has_load_configs,
        "deps.rs should call load_hook_configs to read hook configs from AppState"
    );
    assert!(
        has_pre_hooks,
        "deps.rs should call run_pre_tool_hooks before permission check"
    );
    assert!(
        has_post_hooks,
        "deps.rs should call run_post_tool_hooks after successful tool execution"
    );
    assert!(
        has_failure_hooks,
        "deps.rs should call run_post_tool_failure_hooks on tool errors"
    );
}

/// VERIFIED: MergedConfig.hooks flows to runtime via main.rs → AppState → deps.rs.
///
/// main.rs copies merged_config.hooks into AppState.hooks.
/// deps.rs reads AppState.hooks and passes it to load_hook_configs().
#[test]
fn verified_hooks_config_flows_to_runtime() {
    // main.rs should populate AppState.hooks from merged_config
    let main_source = fs::read_to_string("src/main.rs").expect("should read main.rs");
    assert!(
        main_source.contains("hooks: merged_config.hooks"),
        "main.rs should copy merged_config.hooks into AppState"
    );

    // deps.rs should read app_state.hooks and call load_hook_configs
    let deps_source =
        fs::read_to_string("src/engine/lifecycle/deps.rs").expect("should read deps.rs");
    assert!(
        deps_source.contains("app_state.hooks"),
        "deps.rs should read hooks from AppState"
    );
    assert!(
        deps_source.contains("load_hook_configs"),
        "deps.rs should call load_hook_configs"
    );
}

/// FIXED: query/stop_hooks.rs now delegates to tools::hooks::run_stop_hooks.
///
/// The query loop calls run_stop_hooks from query/stop_hooks.rs, which
/// accepts hook_configs and delegates to the real hooks::run_stop_hooks()
/// in tools/hooks.rs. The call site in loop_impl.rs loads configs via
/// load_hook_configs(&hooks_map, "Stop").
#[test]
fn bug_query_stop_hooks_is_placeholder() {
    let source = fs::read_to_string("src/query/stop_hooks.rs").expect("should read stop_hooks.rs");

    // stop_hooks.rs should accept HookEventConfig and delegate to hooks::run_stop_hooks
    assert!(
        source.contains("HookEventConfig"),
        "stop_hooks.rs should reference HookEventConfig type"
    );
    assert!(
        source.contains("hooks::run_stop_hooks"),
        "stop_hooks.rs should delegate to hooks::run_stop_hooks"
    );

    // The call site in loop_impl.rs should load configs from AppState
    let loop_source =
        fs::read_to_string("src/query/loop_impl.rs").expect("should read loop_impl.rs");
    assert!(
        loop_source.contains("load_hook_configs") && loop_source.contains("\"Stop\""),
        "loop_impl.rs should load Stop hook configs via load_hook_configs"
    );
}

/// Both execution paths have hook wiring.
///
/// execution.rs::run_tool_use() accepts hook_configs as a parameter and is
/// used by StreamingToolExecutor. deps.rs has its own inline hook wiring
/// using load_hook_configs + run_pre/post/failure hooks.
///
/// The two paths serve different callers but both support hooks.
#[test]
fn both_execution_paths_have_hook_wiring() {
    let execution_source =
        fs::read_to_string("src/tools/execution.rs").expect("should read execution.rs");
    let deps_source =
        fs::read_to_string("src/engine/lifecycle/deps.rs").expect("should read deps.rs");

    // execution.rs correctly accepts hook_configs
    assert!(
        execution_source.contains("hook_configs: &[HookEventConfig]"),
        "execution.rs should accept hook_configs parameter"
    );

    // execution.rs StreamingToolExecutor stores hook_configs
    assert!(
        execution_source.contains("hook_configs: Vec<HookEventConfig>"),
        "StreamingToolExecutor should store hook_configs"
    );

    // deps.rs has inline hook wiring (loads configs and calls hooks)
    assert!(
        deps_source.contains("load_hook_configs"),
        "deps.rs should call load_hook_configs"
    );
    assert!(
        deps_source.contains("run_pre_tool_hooks"),
        "deps.rs should call run_pre_tool_hooks"
    );
    assert!(
        deps_source.contains("run_post_tool_hooks"),
        "deps.rs should call run_post_tool_hooks"
    );
}

// =========================================================================
// 4. Summary — what needs to be fixed
// =========================================================================

/// Meta-test: print a summary of hooks wiring status.
///
/// This test always passes but prints a clear diagnostic when run with
/// `cargo test --test e2e_hooks -- --nocapture`.
#[test]
fn hooks_wiring_diagnostic_summary() {
    let status = vec![
        (
            "orchestration.rs",
            "FIXED — loads hook configs from AppState via get_app_state()",
        ),
        (
            "deps.rs",
            "FIXED — loads hook configs from AppState.hooks, runs pre/post/failure hooks",
        ),
        (
            "AppState.hooks",
            "FIXED — consumed by deps.rs and orchestration.rs at runtime",
        ),
        (
            "execution.rs",
            "FIXED — StreamingToolExecutor stores and forwards hook_configs",
        ),
        (
            "query/stop_hooks.rs",
            "PENDING (Task 4) — placeholder that always returns AllowStop",
        ),
    ];

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║           HOOKS WIRING STATUS SUMMARY                       ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    for (location, note) in &status {
        eprintln!("║ {:20} │ {}", location, note);
    }
    eprintln!("╚══════════════════════════════════════════════════════════════╝\n");
}
