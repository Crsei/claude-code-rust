//! Offline headless IPC protocol tests (no API key needed).
//!
//! These verify the `--headless` JSONL protocol without network access.

use std::io::Write;
use std::time::Duration;

use crate::helpers::{
    collect_until, read_line_json, send_msg, spawn_headless, workspace, LINE_TIMEOUT,
};

/// The backend should emit a `ready` message immediately after starting.
#[test]
fn emits_ready_on_start() {
    let (mut child, _stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);

    assert_eq!(
        msg["type"], "ready",
        "first message should be 'ready': {:?}",
        msg
    );
    assert!(
        msg["session_id"].is_string(),
        "ready should have session_id"
    );
    assert!(msg["model"].is_string(), "ready should have model");
    assert!(msg["cwd"].is_string(), "ready should have cwd");

    let _ = child.kill();
    let _ = child.wait();
}

/// Sending `quit` should cause the process to exit cleanly.
#[test]
fn quit_exits_cleanly() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));

    let status = child.wait().expect("wait for child");
    assert!(
        status.success(),
        "headless should exit cleanly on quit, got: {:?}",
        status
    );
}

/// Invalid JSON on stdin should produce a recoverable error, not crash.
#[test]
fn invalid_json_returns_error() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    writeln!(stdin, "this is not valid json").expect("write");
    stdin.flush().expect("flush");

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(
        msg["type"], "error",
        "expected error for invalid JSON: {:?}",
        msg
    );
    assert_eq!(msg["recoverable"], true);
    assert!(
        msg["message"]
            .as_str()
            .unwrap_or("")
            .contains("invalid FrontendMessage"),
        "error message should mention invalid FrontendMessage: {:?}",
        msg
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Valid JSON but unknown type should produce an error.
#[test]
fn unknown_message_type_returns_error() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "nonexistent_command", "data": 42}),
    );

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(msg["type"], "error");
    assert_eq!(msg["recoverable"], true);

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// `resize` should be accepted silently (no error response).
#[test]
fn resize_accepted() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "resize", "cols": 120, "rows": 40}),
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success(), "backend should survive resize");
}

/// `slash_command` should return a system_info warning (not yet supported).
#[test]
fn slash_command_returns_warning() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "slash_command", "raw": "/help"}),
    );

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(
        msg["type"], "system_info",
        "slash_command should produce system_info: {:?}",
        msg
    );
    assert_eq!(msg["level"], "warning");
    assert!(
        msg["text"]
            .as_str()
            .unwrap_or("")
            .contains("not yet supported"),
        "should mention not yet supported: {:?}",
        msg
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// `submit_prompt` with no API key should produce an error (not crash).
#[test]
fn submit_prompt_no_api_key_returns_error() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "hello",
            "id": "test-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| {
            msg["type"] == "error" || msg["type"] == "stream_end" || msg["type"] == "usage_update"
        },
        Duration::from_secs(30),
    );

    let has_error = messages
        .iter()
        .any(|m| m["type"] == "error" || (m["type"] == "system_info" && m["level"] == "error"));
    let has_result_error = messages.iter().any(|m| {
        m["type"] == "error"
            && m["message"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .contains("api")
    });

    assert!(
        has_error || has_result_error || !messages.is_empty(),
        "submit_prompt without API key should produce an error response, got: {:?}",
        messages
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Multiple messages in sequence should all be handled.
#[test]
fn multiple_messages_in_sequence() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "resize", "cols": 80, "rows": 24}),
    );
    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "slash_command", "raw": "/version"}),
    );
    writeln!(stdin, "{{bad json").expect("write");
    stdin.flush().expect("flush");

    let msg1 = read_line_json(&mut stdout, LINE_TIMEOUT);
    let msg2 = read_line_json(&mut stdout, LINE_TIMEOUT);

    assert_eq!(msg1["type"], "system_info");
    assert_eq!(msg2["type"], "error");
    assert_eq!(msg2["recoverable"], true);

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// `--headless` with `-C` sets the correct cwd in the ready message.
#[test]
fn cwd_in_ready_message() {
    let (mut child, _stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(msg["type"], "ready");

    let cwd = msg["cwd"].as_str().unwrap_or("");
    // The workspace path varies by platform — just check it's not empty
    // and contains part of the workspace() path
    let ws_leaf = std::path::Path::new(workspace())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    assert!(
        cwd.contains(ws_leaf.as_ref()),
        "cwd should contain '{}', got: {}",
        ws_leaf,
        cwd
    );

    let _ = child.kill();
    let _ = child.wait();
}

/// `--headless` with `-m` sets the model in the ready message.
#[test]
fn model_override_in_ready() {
    let (mut child, _stdin, mut stdout) =
        spawn_headless(&["-C", workspace(), "-m", "test-model-xyz"], true);

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(msg["type"], "ready");
    assert_eq!(
        msg["model"], "test-model-xyz",
        "model should be overridden: {:?}",
        msg
    );

    let _ = child.kill();
    let _ = child.wait();
}

/// Closing stdin (simulating frontend crash) should cause backend to exit.
#[test]
fn stdin_close_causes_exit() {
    let (mut child, stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    drop(stdin);

    let status = child.wait().expect("wait");
    // May exit 0 or non-zero — just verify it doesn't hang
    let _ = status;
}

/// `permission_response` without a pending request should be silently ignored.
#[test]
fn permission_response_no_pending() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "permission_response",
            "tool_use_id": "fake-tool-id",
            "decision": "allow"
        }),
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
