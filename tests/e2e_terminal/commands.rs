//! Slash command tests for headless IPC.
//!
//! Tests that various slash commands are accepted by the headless backend.
//! Currently, all slash commands return a "not yet supported in headless mode"
//! warning — these tests document the expected responses and serve as
//! regression tests for when slash commands are implemented.

use crate::helpers::{read_line_json, send_msg, spawn_headless, LINE_TIMEOUT};

/// Helper: send a slash command and return the response message.
fn send_slash_and_get_response(raw: &str) -> serde_json::Value {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", r"F:\temp"], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "slash_command", "raw": raw}),
    );

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());

    msg
}

// =========================================================================
//  Offline: slash command handling
// =========================================================================

/// /help should produce a response (system_info with warning).
#[test]
fn slash_help() {
    let msg = send_slash_and_get_response("/help");
    assert_eq!(msg["type"], "system_info");
    assert!(
        msg["text"].as_str().unwrap_or("").contains("/help"),
        "/help response should echo the command: {:?}",
        msg
    );
}

/// /version should produce a response.
#[test]
fn slash_version() {
    let msg = send_slash_and_get_response("/version");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/version"));
}

/// /compact should produce a response.
#[test]
fn slash_compact() {
    let msg = send_slash_and_get_response("/compact");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/compact"));
}

/// /model should produce a response.
#[test]
fn slash_model() {
    let msg = send_slash_and_get_response("/model");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/model"));
}

/// /clear should produce a response.
#[test]
fn slash_clear() {
    let msg = send_slash_and_get_response("/clear");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/clear"));
}

/// /config should produce a response.
#[test]
fn slash_config() {
    let msg = send_slash_and_get_response("/config");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/config"));
}

/// /cost should produce a response.
#[test]
fn slash_cost() {
    let msg = send_slash_and_get_response("/cost");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/cost"));
}

/// /quit should produce a response (but not cause exit — that's the quit message type).
#[test]
fn slash_quit() {
    let msg = send_slash_and_get_response("/quit");
    assert_eq!(msg["type"], "system_info");
}

/// Empty slash command should produce a response without crash.
#[test]
fn slash_empty() {
    let msg = send_slash_and_get_response("/");
    assert_eq!(msg["type"], "system_info");
}

/// Unknown slash command should produce a response without crash.
#[test]
fn slash_unknown() {
    let msg = send_slash_and_get_response("/nonexistent_command_xyz");
    assert_eq!(msg["type"], "system_info");
}

/// Slash command with arguments should be accepted.
#[test]
fn slash_with_args() {
    let msg = send_slash_and_get_response("/model claude-sonnet-4-20250514");
    assert_eq!(msg["type"], "system_info");
    assert!(msg["text"].as_str().unwrap_or("").contains("/model"));
}

/// All slash commands should return "warning" level (currently unsupported in headless).
#[test]
fn all_slash_commands_return_warning_level() {
    for cmd in &["/help", "/version", "/compact", "/clear", "/cost", "/model"] {
        let msg = send_slash_and_get_response(cmd);
        assert_eq!(
            msg["level"], "warning",
            "{} should return warning level: {:?}",
            cmd, msg
        );
    }
}

/// Multiple slash commands in sequence in one session should all work.
#[test]
fn multiple_slash_commands_in_session() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", r"F:\temp"], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    let commands = ["/help", "/version", "/compact", "/model", "/clear"];
    for cmd in &commands {
        send_msg(
            &mut stdin,
            &serde_json::json!({"type": "slash_command", "raw": cmd}),
        );

        let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
        assert_eq!(
            msg["type"], "system_info",
            "{} should produce system_info: {:?}",
            cmd, msg
        );
    }

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
