//! Welcome screen tests: verify the TUI renders the initial view correctly.

use crate::harness::*;
use std::time::Duration;

/// The welcome screen should display the application header with version.
#[test]
fn shows_header_with_version() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Use current_screen() to check what the terminal actually renders right now,
    // as opposed to accumulated text which may contain stale frames.
    let screen = session.current_screen();
    assert!(
        screen.contains("Claude Code") || screen.contains("cc-rust"),
        "current screen should show app name in header, got:\n{}",
        screen
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "welcome_header");
}

/// The welcome screen should display the ASCII art logo.
#[test]
fn shows_ascii_logo() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();

    let output = session.finish(QUICK_TIMEOUT, "welcome_logo");

    assert!(
        output.raw.len() > 500,
        "welcome screen should produce substantial output (logo + tips), got {} bytes",
        output.raw.len()
    );
    output.preview(600);
}

/// The welcome screen should display usage tips.
#[test]
fn shows_tips() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Check the actual rendered screen for tips content
    let screen = session.current_screen();
    let has_tips = screen.contains("Tips")
        || screen.contains("Enter")
        || screen.contains("Ctrl")
        || screen.contains("Type a message");

    assert!(
        has_tips,
        "current screen should show usage tips, got:\n{}",
        screen
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "welcome_tips");
}

/// The welcome screen at different terminal sizes should still render.
#[test]
fn renders_at_small_terminal() {
    let session = PtySession::spawn(default_args(), 60, 20, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();

    let output = session.finish(QUICK_TIMEOUT, "welcome_small");

    assert!(
        output.raw.len() > 100,
        "should render even at 60x20, got {} bytes",
        output.raw.len()
    );
}

/// The welcome screen at a wide terminal should render without crash.
#[test]
fn renders_at_wide_terminal() {
    let session = PtySession::spawn(default_args(), 200, 50, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();

    let output = session.finish(QUICK_TIMEOUT, "welcome_wide");

    assert!(
        output.raw.len() > 100,
        "should render at 200x50, got {} bytes",
        output.raw.len()
    );
}
