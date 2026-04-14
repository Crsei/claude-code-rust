//! Screenshot tests: capture terminal screenshots at key moments for visual inspection.
//!
//! Pipeline: PTY spawn → ANSI capture → vt100 terminal emulator → HTML render → browser screenshot
//!
//! After running: open `logs/YYYYMMDDHHMM/screenshot_*.html` in a browser.

use crate::harness::*;
use std::time::Duration;

// ─── Single-shot screenshots ────────────────────────────────────────

/// Welcome screen at 120x40.
#[test]
fn screenshot_welcome_screen() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "screenshot_welcome");

    assert!(logs_dir().join("screenshot_welcome.html").exists());
    assert!(output.contains("cc-rust") || output.contains("Claude Code"));
}

/// Single chat response at 120x40.
#[test]
fn screenshot_chat_response() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("Say exactly: SCREENSHOT_TEST_OK");
    let found = session.wait_for_text("Claude:", API_TIMEOUT);
    std::thread::sleep(Duration::from_secs(2));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "screenshot_chat");

    assert!(logs_dir().join("screenshot_chat.html").exists());
    assert!(found, "no model response, got:\n{}", output.text());
}

/// Narrow terminal (60x20) — single-column layout.
#[test]
fn screenshot_narrow_terminal() {
    let session = PtySession::spawn(default_args(), 60, 20, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _ = session.finish(QUICK_TIMEOUT, "screenshot_narrow");
    assert!(logs_dir().join("screenshot_narrow.html").exists());
}

// ─── Multi-turn realistic conversation ──────────────────────────────

/// Realistic 5-turn conversation with mid-session snapshots.
///
/// Uses `wait_response_done()` which reads the CURRENT screen state via
/// vt100 emulation to detect when the status bar shows "ready" with an
/// increased message count. This correctly handles tool-use turns where
/// the message count jumps by more than 2.
#[test]
fn screenshot_multi_turn_conversation() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Pure knowledge questions — no tool triggers (avoid file/project references)
    let turns: &[(&str, &str)] = &[
        (
            "What are the three primary colors? Answer in one short sentence.",
            "colors",
        ),
        (
            "Now list 5 programming languages sorted by age, oldest first. Keep it brief.",
            "languages",
        ),
        ("Write a haiku about coding.", "haiku"),
        ("What is the capital of France? One word answer.", "capital"),
        (
            "Summarize our conversation so far in one sentence.",
            "summary",
        ),
    ];

    let mut completed = 0;
    let mut last_msg_count = 0usize;

    for (i, (prompt, label)) in turns.iter().enumerate() {
        let turn = i + 1;

        eprintln!(
            "[multi-turn] Turn {turn}/{}: sending '{}' (baseline: {last_msg_count} msgs)",
            turns.len(),
            &prompt[..prompt.len().min(50)]
        );

        session.send_line(prompt);

        // Wait for status bar: "ready" with msg count > last_msg_count
        let ok = session.wait_response_done(last_msg_count, API_TIMEOUT);

        if ok {
            // Read the new msg count from status bar
            let bar = session.status_bar();
            if let Some(count) = extract_msg_count(&bar) {
                last_msg_count = count;
            }

            // Settle time for final rendering
            std::thread::sleep(Duration::from_secs(2));
            session.snapshot(&format!("mt_{turn}_{label}"));
            completed += 1;
            eprintln!("[multi-turn] Turn {turn}: done (now {last_msg_count} msgs)");
        } else {
            session.snapshot(&format!("mt_{turn}_{label}_timeout"));
            eprintln!(
                "[multi-turn] Turn {turn}: TIMEOUT (status bar: '{}')",
                session.status_bar()
            );
        }
    }

    // Exit
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _final = session.finish(QUICK_TIMEOUT, "mt_final");

    assert!(
        completed >= 3,
        "at least 3/5 turns should complete, got {completed}/5"
    );

    eprintln!("\n[multi-turn] Screenshots in: {}", logs_dir().display());
}

/// Extract message count from status bar text like "gpt-5 | 12 msgs | ready | Ctrl+C quit"
fn extract_msg_count(status: &str) -> Option<usize> {
    for (i, word) in status.split_whitespace().enumerate() {
        if word == "msgs" || word == "msg" {
            // The number is the previous word
            if i > 0 {
                let words: Vec<&str> = status.split_whitespace().collect();
                return words.get(i - 1).and_then(|w| w.parse().ok());
            }
        }
    }
    None
}
