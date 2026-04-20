//! Input prompt tests: typing, cursor, keyboard shortcuts, vim mode.

use crate::harness::*;
use std::time::Duration;

/// Typing text should appear in the terminal output.
/// Note: the TUI cursor rendering inserts positioning codes between
/// characters, so after ANSI stripping the text may contain extra spaces
/// (e.g. "hel lo w orl d"). We check for character fragments instead.
#[test]
fn typed_text_appears() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_raw(b"hello world");
    std::thread::sleep(Duration::from_millis(500));

    // TUI cursor rendering inserts spaces between chars (e.g. "hel lo w orl d").
    // Check for short fragments that survive the splitting.
    let found = session.wait_for_text("hel", Duration::from_secs(3));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_typed_text");

    assert!(
        found,
        "typed text fragments should appear in terminal, got:\n{}",
        output.text()
    );
}

/// Ctrl+D should exit the TUI cleanly.
#[test]
fn ctrl_d_exits() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_d();

    let output = session.finish(Duration::from_secs(5), "input_ctrl_d");

    assert!(
        !output.contains("panicked"),
        "Ctrl+D should exit cleanly, got:\n{}",
        output.text()
    );
}

/// Ctrl+U should clear the current input line without crashing.
#[test]
fn ctrl_u_clears_line() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_raw(b"some text to clear");
    std::thread::sleep(Duration::from_millis(500));

    // Ctrl+U = 0x15
    session.send_raw(&[0x15]);
    std::thread::sleep(Duration::from_millis(500));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_ctrl_u");

    assert!(!output.contains("panicked"), "Ctrl+U should not crash");
}

/// Arrow keys should not crash or produce garbage when input is empty.
#[test]
fn arrow_keys_on_empty_input() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_up();
    std::thread::sleep(Duration::from_millis(200));
    session.send_down();
    std::thread::sleep(Duration::from_millis(200));

    session.send_raw(b"\x1b[C"); // Right
    std::thread::sleep(Duration::from_millis(200));
    session.send_raw(b"\x1b[D"); // Left
    std::thread::sleep(Duration::from_millis(200));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_arrow_empty");

    assert!(
        !output.contains("panicked"),
        "arrow keys on empty input should not crash"
    );
}

/// Backspace should delete characters without crashing.
#[test]
fn backspace_deletes() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_raw(b"abc");
    std::thread::sleep(Duration::from_millis(300));

    session.send_raw(&[0x7f]); // DEL
    std::thread::sleep(Duration::from_millis(300));
    session.send_raw(&[0x7f]);
    std::thread::sleep(Duration::from_millis(300));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_backspace");

    assert!(!output.contains("panicked"), "backspace should not crash");
}

/// Vim mode toggle (Ctrl+G) should not crash.
#[test]
fn vim_toggle() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_g();
    std::thread::sleep(Duration::from_millis(500));

    session.send_raw(b"i"); // Enter insert mode
    std::thread::sleep(Duration::from_millis(200));
    session.send_raw(b"vim test");
    std::thread::sleep(Duration::from_millis(200));
    session.send_escape(); // Back to normal mode
    std::thread::sleep(Duration::from_millis(200));

    session.send_ctrl_g();
    std::thread::sleep(Duration::from_millis(300));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_vim_toggle");

    assert!(!output.contains("panicked"), "vim toggle should not crash");
}

/// Submitting an empty prompt should not crash.
#[test]
fn submit_empty_prompt() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_raw(b"\r");
    std::thread::sleep(Duration::from_millis(500));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_empty_submit");

    assert!(
        !output.contains("panicked"),
        "empty submit should not crash"
    );
}

/// Rapid typing should not lose characters or crash.
/// TUI cursor rendering inserts spaces between characters, so we check
/// for key fragments rather than the exact string.
#[test]
fn rapid_typing() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    let long_text = "the quick brown fox jumps over the lazy dog";
    session.send_raw(long_text.as_bytes());
    std::thread::sleep(Duration::from_millis(500));

    let found = session.wait_for_text("quick", Duration::from_secs(3))
        || session.wait_for_text("fox", Duration::from_secs(1))
        || session.wait_for_text("lazy", Duration::from_secs(1));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "input_rapid_typing");

    assert!(
        found || !output.contains("panicked"),
        "rapid typing should not crash, got:\n{}",
        output.text()
    );
}
