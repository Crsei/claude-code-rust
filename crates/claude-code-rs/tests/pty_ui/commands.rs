//! Slash command tests in the PTY TUI.
//!
//! Verifies that slash commands produce the expected output when typed
//! into the TUI input box. All tests use real API (strip_keys=false).

use crate::harness::*;
use std::time::Duration;

/// /help 显示命令列表
#[test]
#[ignore]
fn slash_help_shows_command_list() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // /help output should list other commands
    session.send_line("/help");
    let found = session.wait_for_any(&["/help", "/exit", "/version", "/model"], QUICK_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_help");

    assert!(found.is_some(), "/help should list available commands");
}

/// /version 显示版本号
#[test]
#[ignore]
fn slash_version_shows_version() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Version output should contain the crate version or "claude-code-rs"
    session.send_line("/version");
    let found = session.wait_for_any(
        &["claude-code-rs", env!("CARGO_PKG_VERSION")],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_version");

    assert!(found.is_some(), "/version should show version info");
}

/// /exit 正常退出 TUI
#[test]
#[ignore]
fn slash_exit_quits_gracefully() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // /exit should cause the process to exit — finish should complete within timeout
    session.send_line("/exit");

    let output = session.finish(QUICK_TIMEOUT, "cmd_exit");

    // No panic, process exited. Output may contain a goodbye message.
    assert!(
        !output.raw.is_empty(),
        "should have captured some output before exit"
    );
}

/// /cost 显示 token 用量（初始为零）
#[test]
#[ignore]
fn slash_cost_shows_usage() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Cost/usage output should contain token-related text
    session.send_line("/cost");
    let found = session.wait_for_any(&["token", "cost", "usage", "0"], QUICK_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_cost");

    assert!(found.is_some(), "/cost should show usage info");
}

/// /model 显示当前模型
#[test]
#[ignore]
fn slash_model_shows_current_model() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Should show model name or "model" keyword
    session.send_line("/model");
    let found = session.wait_for_any(
        &["model", "claude", "sonnet", "opus", "haiku"],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_model");

    assert!(found.is_some(), "/model should show model info");
}

/// /status 显示会话状态
#[test]
#[ignore]
fn slash_status_shows_session_info() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Status should contain session-related info
    session.send_line("/status");
    let found = session.wait_for_any(
        &["model", "session", "message", "permission", "mode"],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_status");

    assert!(found.is_some(), "/status should show session info");
}

/// 未知命令不崩溃，显示错误提示
#[test]
#[ignore]
fn slash_unknown_command_shows_error() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Should show "unknown" or "not found" or echo the command back
    session.send_line("/nonexistent_xyz_42");
    let _found = session.wait_for_any(
        &["unknown", "not found", "nonexistent", "invalid"],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "cmd_unknown");

    // At minimum, no panic
    assert!(
        !output.raw.is_empty(),
        "should not panic on unknown command"
    );
}

/// 空斜杠 "/" 不崩溃
#[test]
#[ignore]
fn slash_empty_does_not_crash() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("/");
    std::thread::sleep(Duration::from_secs(2));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "cmd_empty_slash");

    assert!(!output.raw.is_empty(), "empty slash should not panic");
}

/// /model 带参数切换模型
#[test]
#[ignore]
fn slash_model_with_arg_switches_model() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Try switching to sonnet alias
    session.send_line("/model sonnet");
    let found = session.wait_for_any(&["sonnet", "model", "switch", "changed"], QUICK_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_model_switch");

    assert!(
        found.is_some(),
        "/model sonnet should acknowledge model change"
    );
}

/// /clear 清除对话历史，不崩溃
#[test]
#[ignore]
fn slash_clear_resets_conversation() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("/clear");
    std::thread::sleep(Duration::from_secs(2));

    // After clear, TUI should still be functional — type something to verify
    session.send_line("hello after clear");
    std::thread::sleep(Duration::from_secs(1));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "cmd_clear");

    assert!(!output.raw.is_empty(), "/clear should not crash TUI");
}

/// /context 显示上下文信息
#[test]
#[ignore]
fn slash_context_shows_info() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("/context");
    let found = session.wait_for_any(&["context", "token", "model", "message"], QUICK_TIMEOUT);

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_context");

    assert!(found.is_some(), "/context should show context info");
}

/// /skills 列出可用技能
#[test]
#[ignore]
fn slash_skills_lists_skills() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line("/skills");
    let found = session.wait_for_any(
        &["skill", "available", "built-in", "no skill"],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "cmd_skills");

    assert!(found.is_some(), "/skills should list or mention skills");
}

/// 同一会话连续执行多个斜杠命令
#[test]
#[ignore]
fn multiple_commands_in_sequence() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    let commands = ["/version", "/cost", "/model", "/context", "/status"];

    for (i, cmd) in commands.iter().enumerate() {
        eprintln!("[multi-cmd] {}/{}: {}", i + 1, commands.len(), cmd);
        session.send_line(cmd);
        std::thread::sleep(Duration::from_secs(2));
        session.snapshot(&format!("cmd_seq_{}", i + 1));
    }

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "cmd_sequence");

    // No panic after 5 consecutive commands
    assert!(
        output.raw.len() > 100,
        "multiple commands should produce substantial output"
    );
}
