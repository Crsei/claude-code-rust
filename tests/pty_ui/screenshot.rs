//! Screenshot demo: captures terminal screenshots at key moments.
//!
//! This test demonstrates the full screenshot pipeline:
//! 1. Spawn cc-rust in a PTY (pseudo-terminal)
//! 2. The terminal renders the TUI (welcome screen, status bar, etc.)
//! 3. Raw ANSI output is captured by the reader thread
//! 4. On finish, `vt100` emulates a virtual terminal to parse ANSI → 2D grid
//! 5. The grid is rendered to an HTML file preserving colors and layout
//! 6. The HTML can be opened in a browser or screenshotted for visual inspection
//!
//! After running: open `logs/YYYYMMDDHHMM/screenshot_*.html` in a browser
//! to see exactly what the terminal looked like.

use crate::harness::*;
use std::time::Duration;

/// Capture the welcome screen — the first thing users see.
///
/// This screenshot shows: ASCII logo, model name, session ID, tips, keybindings,
/// status bar, and the input prompt at the bottom.
#[test]
fn screenshot_welcome_screen() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);

    // Wait for TUI to fully render
    std::thread::sleep(RENDER_WAIT);

    // Take screenshot by finishing (this generates .html)
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "screenshot_welcome");

    // The HTML file is now at logs/YYYYMMDDHHMM/screenshot_welcome.html
    let html_path = logs_dir().join("screenshot_welcome.html");
    assert!(html_path.exists(), "HTML screenshot should be generated");

    eprintln!("[screenshot] Welcome screen saved to: {}", html_path.display());
    eprintln!("[screenshot] Open in browser to view the terminal screenshot");

    // Basic sanity: the plain text should contain cc-rust branding
    assert!(output.contains("cc-rust") || output.contains("Claude Code"));
}

/// Capture a mid-conversation screenshot showing model response.
///
/// Shows: user message, model streaming indicator, model response text,
/// updated message count in status bar.
#[test]
fn screenshot_chat_response() {
    let session = PtySession::spawn(DEFAULT_ARGS, 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Send a simple prompt
    session.send_line("Say exactly: SCREENSHOT_TEST_OK");

    // Wait for model response
    let found = session.wait_for_text("Claude:", API_TIMEOUT);

    // Give time for the full response to render
    std::thread::sleep(Duration::from_secs(2));

    // Exit
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "screenshot_chat");

    let html_path = logs_dir().join("screenshot_chat.html");
    assert!(html_path.exists(), "HTML screenshot should be generated");

    eprintln!("[screenshot] Chat response saved to: {}", html_path.display());

    assert!(
        found,
        "model response should appear, got:\n{}",
        output.text()
    );
}

/// Capture a narrow terminal (60x20) to check responsive layout.
///
/// Shows how the TUI adapts to constrained terminal dimensions.
#[test]
fn screenshot_narrow_terminal() {
    let session = PtySession::spawn(DEFAULT_ARGS, 60, 20, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "screenshot_narrow");

    let html_path = logs_dir().join("screenshot_narrow.html");
    assert!(html_path.exists(), "HTML screenshot should be generated");

    eprintln!("[screenshot] Narrow terminal saved to: {}", html_path.display());
    let _ = output;
}
