//! Permission mode tests for headless IPC.
//!
//! Verifies that the default permission mode is NOT bypass,
//! and that permission enforcement works correctly in headless mode.

use crate::helpers::{
    collect_until, read_line_json, send_msg, spawn_headless, workspace, LINE_TIMEOUT, LIVE_TIMEOUT,
};

// =========================================================================
//  Offline: permission mode defaults
// =========================================================================

/// Default mode (no --permission-mode flag) should NOT be bypass.
/// We verify by asking the model to run a Bash command without bypass —
/// in default mode the tool execution should be denied with a "Permission required"
/// error, since headless is non-interactive.
#[test]
#[ignore]
fn default_mode_denies_tool_in_headless() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp"],
        false, // use real API keys, but no --permission-mode bypass
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo PERMISSION_TEST",
            "id": "perm-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update" || msg["type"] == "error",
        LIVE_TIMEOUT,
    );

    // In default mode (non-interactive headless), the tool call should be denied.
    // The assistant_message content should contain a tool_result with is_error=true
    // mentioning "Permission required" or "Permission denied".
    let all_json = format!("{:?}", messages);
    let has_permission_block = all_json.contains("Permission required")
        || all_json.contains("Permission denied")
        || all_json.contains("permission");

    assert!(
        has_permission_block,
        "default mode should deny tool call in headless (non-interactive), messages: {:?}",
        messages
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Explicit --permission-mode bypass should allow tools without asking.
#[test]
#[ignore]
fn bypass_mode_allows_tool() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "bypass"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo BYPASS_OK",
            "id": "perm-002"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    // In bypass mode, the tool should execute successfully
    let all_text: String = messages
        .iter()
        .filter_map(|m| {
            m["text"]
                .as_str()
                .or_else(|| m["output"].as_str())
                .map(String::from)
        })
        .collect();
    let all_json = format!("{:?}", messages);

    let has_output = all_text.contains("BYPASS_OK") || all_json.contains("BYPASS_OK");
    assert!(
        has_output,
        "bypass mode should allow tool execution, messages: {:?}",
        messages
    );

    // Should NOT contain "Permission denied/required"
    assert!(
        !all_json.contains("Permission denied") && !all_json.contains("Permission required"),
        "bypass mode should not produce permission errors"
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// --permission-mode auto should also allow tools without asking.
#[test]
#[ignore]
fn auto_mode_allows_tool() {
    let (mut child, mut stdin, mut stdout) =
        spawn_headless(&["-C", r"F:\temp", "--permission-mode", "auto"], false);

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo AUTO_OK",
            "id": "perm-003"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let all_json = format!("{:?}", messages);
    let has_output = all_json.contains("AUTO_OK");

    assert!(
        has_output,
        "auto mode should allow tool execution, messages: {:?}",
        messages
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Verify that permission mode flags are accepted at CLI level (offline).
#[test]
fn permission_mode_flags_accepted_headless() {
    for mode in &["default", "auto", "bypass"] {
        let (mut child, mut stdin, mut stdout) =
            spawn_headless(&["-C", workspace(), "--permission-mode", mode], true);
        let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
        assert_eq!(
            ready["type"], "ready",
            "headless should start with --permission-mode {}",
            mode
        );

        send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
        let status = child.wait().expect("wait");
        assert!(
            status.success(),
            "should exit cleanly with --permission-mode {}",
            mode
        );
    }
}
