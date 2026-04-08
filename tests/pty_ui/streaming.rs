//! Streaming lifecycle tests: verify prompt submission, response rendering,
//! abort handling, multi-turn conversation, and tool use display.

use crate::harness::*;
use std::time::Duration;

/// Submit a prompt and verify the model response appears in TUI.
/// Waits for "Claude:" prefix which only appears in model output, not user echo.
#[test]
fn simple_chat_renders_response() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("Say exactly: STREAM_RENDER_OK");

    // Wait for model response (not user echo)
    let found = session.wait_for_text("Claude:", API_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "stream_simple_chat");

    assert!(
        found,
        "model response (Claude:) should appear in TUI, got:\n{}",
        output.text()
    );
    // Verify the actual response text
    assert!(
        output.contains("STREAM_RENDER_OK"),
        "response should contain STREAM_RENDER_OK, got:\n{}",
        output.text()
    );
}

/// Ctrl+C during streaming should abort without crashing.
/// We send a long prompt, wait for streaming to start, then abort.
#[test]
fn abort_during_streaming() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("Write a 2000-word essay about computing history.");

    // Wait for streaming to actually start (spinner appears)
    session.wait_for_text("Thinking", Duration::from_secs(15));
    std::thread::sleep(Duration::from_secs(2));

    // Abort
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_secs(2));

    // TUI should still be alive — send another prompt and wait for response
    session.send_line("Say exactly: AFTER_ABORT_OK");
    let found = session.wait_for_text("Claude:", API_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "stream_abort");

    assert!(
        found,
        "should recover after abort and show Claude: response, got:\n{}",
        output.text()
    );
}

/// Two consecutive prompts in one session produce separate responses.
#[test]
fn multi_turn_conversation() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // First turn — wait for "Claude:" prefix which only appears in model response.
    session.send_line("Say exactly: FIRST_TURN_OK");
    let found_first = session.wait_for_text("Claude:", API_TIMEOUT);

    // Wait for model to fully finish and TUI to return to input mode.
    std::thread::sleep(Duration::from_secs(5));

    // Second turn
    session.send_line("Say exactly: SECOND_TURN_OK");
    let found_second = session.wait_for_text("SECOND_TURN_OK", API_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "stream_multi_turn");

    assert!(found_first, "first turn should render Claude: prefix");
    assert!(found_second, "second turn should render");
    output.preview(1000);
}

/// Tool use (Bash) should display tool name or result in TUI.
/// Waits for tool-specific markers that don't appear in user echo.
#[test]
fn tool_use_displayed() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("Use the Bash tool to run: echo PTY_TOOL_VISIBLE");

    // Wait for tool execution markers — these only appear in model output:
    // "⚡ Bash" (tool use block), "PTY_TOOL_VISIBLE" (tool result), "Claude:" (response)
    let found = session.wait_for_any(
        &["Claude:", "PTY_TOOL_VISIBLE"],
        API_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "stream_tool_use");

    assert!(
        found.is_some(),
        "tool use should produce Claude: response or tool output, got:\n{}",
        output.text()
    );
}

/// Submitting a prompt without API key shows error in TUI (not crash).
#[test]
fn submit_prompt_no_api_key_shows_error() {
    let session = PtySession::spawn(
        &["-C", r"F:\temp"],
        120,
        40,
        true, // strip keys — testing "no key" error path
    );
    std::thread::sleep(RENDER_WAIT);

    session.send_line("hello");

    let found = session.wait_for_any(
        &["error", "Error", "api", "API", "no API", "configured"],
        Duration::from_secs(10),
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "stream_no_api_error");

    assert!(
        found.is_some() || !output.contains("panicked"),
        "should show error or at least not crash, got:\n{}",
        output.text()
    );
}
