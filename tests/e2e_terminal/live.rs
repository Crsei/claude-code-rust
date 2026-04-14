//! Live headless IPC tests (require real API key, `#[ignore]` by default).
//!
//! Run with: cargo test --test e2e_terminal -- --ignored

use std::time::Duration;

use crate::helpers::{collect_until, read_line_json, send_msg, spawn_headless, LIVE_TIMEOUT};

/// Submit a simple prompt and verify the full streaming lifecycle:
/// stream_start → stream_delta+ → stream_end → assistant_message → usage_update
#[test]
#[ignore]
fn simple_chat() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "bypass"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: HEADLESS_TEST_OK",
            "id": "live-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let types: Vec<&str> = messages.iter().filter_map(|m| m["type"].as_str()).collect();

    assert!(
        types.contains(&"stream_start"),
        "should have stream_start: {:?}",
        types
    );
    assert!(
        types.contains(&"stream_end"),
        "should have stream_end: {:?}",
        types
    );
    assert!(
        types.contains(&"usage_update"),
        "should have usage_update: {:?}",
        types
    );

    let all_deltas: String = messages
        .iter()
        .filter(|m| m["type"] == "stream_delta")
        .filter_map(|m| m["text"].as_str())
        .collect();

    assert!(
        all_deltas.contains("HEADLESS_TEST_OK"),
        "deltas should contain expected text, got: {}",
        all_deltas
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Submit a prompt that requires tool use and verify tool_use + tool_result messages.
#[test]
#[ignore]
fn tool_use_bash() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "bypass"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo IPC_TOOL_OK",
            "id": "live-002"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let types: Vec<&str> = messages.iter().filter_map(|m| m["type"].as_str()).collect();

    assert!(
        types.contains(&"tool_use") || types.contains(&"assistant_message"),
        "should have tool_use or assistant_message: {:?}",
        types
    );

    let has_tool_ok = messages.iter().any(|m| {
        let output = m["output"].as_str().unwrap_or("");
        let text = m["text"].as_str().unwrap_or("");
        let content = format!("{:?}", m["content"]);
        output.contains("IPC_TOOL_OK")
            || text.contains("IPC_TOOL_OK")
            || content.contains("IPC_TOOL_OK")
    });

    assert!(
        has_tool_ok,
        "should find IPC_TOOL_OK in messages: {:?}",
        messages
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Two consecutive prompts in one session — verify both produce responses.
#[test]
#[ignore]
fn two_prompts() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "bypass"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    // First prompt
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: FIRST_REPLY",
            "id": "multi-001"
        }),
    );

    let msgs1 = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let deltas1: String = msgs1
        .iter()
        .filter(|m| m["type"] == "stream_delta")
        .filter_map(|m| m["text"].as_str())
        .collect();
    assert!(
        deltas1.contains("FIRST_REPLY"),
        "first prompt deltas: {}",
        deltas1
    );

    // Second prompt
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: SECOND_REPLY",
            "id": "multi-002"
        }),
    );

    let msgs2 = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let deltas2: String = msgs2
        .iter()
        .filter(|m| m["type"] == "stream_delta")
        .filter_map(|m| m["text"].as_str())
        .collect();
    assert!(
        deltas2.contains("SECOND_REPLY"),
        "second prompt deltas: {}",
        deltas2
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Abort a streaming response and verify the backend survives.
#[test]
#[ignore]
fn abort_during_stream() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "bypass"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Write a very long essay about the history of computing. Make it at least 2000 words.",
            "id": "abort-001"
        }),
    );

    let first = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert!(
        first["type"] == "stream_start" || first["type"] == "stream_delta",
        "expected stream_start or stream_delta, got: {:?}",
        first
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "abort_query"}));

    let _ = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update" || msg["type"] == "stream_end",
        Duration::from_secs(10),
    );

    // Backend should still be alive
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: AFTER_ABORT_OK",
            "id": "abort-002"
        }),
    );

    let msgs = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let deltas: String = msgs
        .iter()
        .filter(|m| m["type"] == "stream_delta")
        .filter_map(|m| m["text"].as_str())
        .collect();

    assert!(
        deltas.contains("AFTER_ABORT_OK"),
        "should recover after abort, got deltas: {}",
        deltas
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
