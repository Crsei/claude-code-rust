//! Terminal resize tests: verify TUI handles different sizes gracefully.

use crate::harness::*;
use std::time::Duration;

/// TUI should render at minimum viable size (40x10) without panic.
#[test]
fn minimum_size() {
    let session = PtySession::spawn(default_args(), 40, 10, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "resize_minimum");

    assert!(
        !output.contains("panicked"),
        "should not panic at 40x10, got:\n{}",
        output.text()
    );
    assert!(
        !output.raw.is_empty(),
        "should produce some output at 40x10"
    );
}

/// TUI should render at very wide terminal (300 cols) without panic.
#[test]
fn very_wide() {
    let session = PtySession::spawn(default_args(), 300, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "resize_wide");

    assert!(!output.contains("panicked"), "should not panic at 300x40");
}

/// TUI should render at very tall terminal (100 rows) without panic.
#[test]
fn very_tall() {
    let session = PtySession::spawn(default_args(), 80, 100, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "resize_tall");

    assert!(!output.contains("panicked"), "should not panic at 80x100");
}

/// TUI should handle typing at a narrow terminal width.
#[test]
fn typing_at_narrow_width() {
    let session = PtySession::spawn(default_args(), 40, 20, false);
    std::thread::sleep(RENDER_WAIT);

    let long_text = "this text is longer than forty columns wide";
    session.send_raw(long_text.as_bytes());
    std::thread::sleep(Duration::from_millis(500));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "resize_narrow_typing");

    assert!(
        !output.contains("panicked"),
        "typing long text at narrow width should not crash"
    );
}

/// Standard 80x24 terminal should render correctly.
#[test]
fn standard_80x24() {
    let session = PtySession::spawn(default_args(), 80, 24, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "resize_80x24");

    assert!(
        output.raw.len() > 100,
        "standard terminal should produce substantial output, got {} bytes",
        output.raw.len()
    );
}
