//! Slash command tests for headless IPC.
//!
//! Tests that various slash commands are accepted and executed by the
//! headless backend. These are protocol-level regression tests for the
//! JSONL messages consumed by the OpenTUI frontend.

use crate::helpers::{read_line_json, send_msg, spawn_headless, workspace, LINE_TIMEOUT};

/// Helper: send a slash command and return the response message.
fn send_slash_and_get_response(raw: &str) -> serde_json::Value {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

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

/// /help should produce a command list response.
#[test]
fn slash_help() {
    let msg = send_slash_and_get_response("/help");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
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
    assert_eq!(msg["level"], "info");
    assert!(msg["text"]
        .as_str()
        .unwrap_or("")
        .contains("claude-code-rs"));
}

/// /compact should produce a response.
#[test]
fn slash_compact() {
    let msg = send_slash_and_get_response("/compact");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"].as_str().unwrap_or("").contains("compact"));
}

/// /model should produce a response.
#[test]
fn slash_model() {
    let msg = send_slash_and_get_response("/model");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"].as_str().unwrap_or("").contains("Current model"));
}

/// /clear should produce a response.
#[test]
fn slash_clear() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({"type": "slash_command", "raw": "/clear"}),
    );

    let replaced = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(replaced["type"], "conversation_replaced");
    assert_eq!(replaced["messages"].as_array().map(Vec::len), Some(0));

    let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(
        msg["text"].as_str().unwrap_or("").contains("cleared"),
        "/clear should confirm the conversation was cleared: {:?}",
        msg
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// /config should produce a response.
#[test]
fn slash_config() {
    let msg = send_slash_and_get_response("/config");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"]
        .as_str()
        .unwrap_or("")
        .contains("Effective configuration"));
}

/// /cost should produce a response.
#[test]
fn slash_cost() {
    let msg = send_slash_and_get_response("/cost");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"].as_str().unwrap_or("").contains("API calls"));
}

/// /quit should produce a response (but not cause exit — that's the quit message type).
#[test]
fn slash_quit() {
    let msg = send_slash_and_get_response("/quit");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"].as_str().unwrap_or("").contains("Goodbye"));
}

/// Empty slash command should produce a response without crash.
#[test]
fn slash_empty() {
    let msg = send_slash_and_get_response("/");
    assert_eq!(msg["type"], "error");
    assert_eq!(msg["recoverable"], true);
    assert!(msg["message"]
        .as_str()
        .unwrap_or("")
        .contains("unknown command"));
}

/// Unknown slash command should produce a response without crash.
#[test]
fn slash_unknown() {
    let msg = send_slash_and_get_response("/nonexistent_command_xyz");
    assert_eq!(msg["type"], "error");
    assert_eq!(msg["recoverable"], true);
    assert!(msg["message"]
        .as_str()
        .unwrap_or("")
        .contains("unknown command"));
}

/// Slash command with arguments should be accepted.
#[test]
fn slash_with_args() {
    let msg = send_slash_and_get_response("/model claude-sonnet-4-20250514");
    assert_eq!(msg["type"], "system_info");
    assert_eq!(msg["level"], "info");
    assert!(msg["text"].as_str().unwrap_or("").contains("Model changed"));
}

/// Output-only slash commands should return an info-level system message.
#[test]
fn output_slash_commands_return_info_level() {
    for cmd in &["/help", "/version", "/compact", "/cost", "/model"] {
        let msg = send_slash_and_get_response(cmd);
        assert_eq!(
            msg["level"], "info",
            "{} should return info level: {:?}",
            cmd, msg
        );
    }
}

/// Multiple slash commands in sequence in one session should all work.
#[test]
fn multiple_slash_commands_in_session() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(&["-C", workspace()], true);

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    let commands = ["/help", "/version", "/compact", "/model", "/clear"];
    for cmd in &commands {
        send_msg(
            &mut stdin,
            &serde_json::json!({"type": "slash_command", "raw": cmd}),
        );

        let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
        if *cmd == "/clear" {
            assert_eq!(
                msg["type"], "conversation_replaced",
                "{} should replace the conversation first: {:?}",
                cmd, msg
            );

            let msg = read_line_json(&mut stdout, LINE_TIMEOUT);
            assert_eq!(
                msg["type"], "system_info",
                "{} should then confirm the clear: {:?}",
                cmd, msg
            );
            assert_eq!(msg["level"], "info");
        } else {
            assert_eq!(
                msg["type"], "system_info",
                "{} should produce system_info: {:?}",
                cmd, msg
            );
            assert_eq!(msg["level"], "info");
        }
    }

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
