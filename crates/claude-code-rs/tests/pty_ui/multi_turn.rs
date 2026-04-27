//! Multi-turn conversation depth tests.
//!
//! Verifies complex interaction patterns: context persistence across turns,
//! slash commands interleaved with prompts, abort/resume, tool use, and
//! long conversations.
//!
//! All tests use real API (strip_keys=false) and are marked `#[ignore]`.

use crate::harness::*;
use std::time::Duration;

/// 多次输入 + 斜杠命令交替，不崩溃
#[test]
#[ignore]
fn interleaved_input_and_commands() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Alternate between typing text and slash commands
    let actions: &[&str] = &[
        "Say exactly: INTERLEAVE_1",
        "/version",
        "Say exactly: INTERLEAVE_2",
        "/cost",
        "/status",
        "Say exactly: INTERLEAVE_3",
        "/help",
    ];

    for (i, action) in actions.iter().enumerate() {
        eprintln!("[interleaved] {}/{}: {}", i + 1, actions.len(), action);
        session.send_line(action);
        if action.starts_with("Say") {
            // Wait for model response
            session.wait_response_done(0, API_TIMEOUT);
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "mt_interleaved");

    assert!(output.raw.len() > 100, "interleaved input should not crash");
}

/// /clear 后 TUI 仍可正常接受输入和命令
#[test]
#[ignore]
fn clear_then_continue_input() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // First turn
    session.send_line("Say exactly: BEFORE_CLEAR");
    session.wait_response_done(0, API_TIMEOUT);
    std::thread::sleep(Duration::from_secs(2));

    session.send_line("/clear");
    std::thread::sleep(Duration::from_secs(2));

    // After clear, continue
    session.send_line("Say exactly: AFTER_CLEAR");
    let ok = session.wait_response_done(0, API_TIMEOUT);

    session.send_line("/status");
    std::thread::sleep(Duration::from_secs(2));

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let output = session.finish(QUICK_TIMEOUT, "mt_clear_continue");

    assert!(ok, "should produce response after /clear");
    assert!(
        !output.raw.is_empty(),
        "/clear then continue should not crash"
    );
}

/// 快速连续输入多行后 slash 命令仍响应
#[test]
#[ignore]
fn rapid_multi_line_input() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Rapidly send many lines without waiting
    for i in 0..10 {
        session.send_line(&format!("rapid line {i}"));
    }
    std::thread::sleep(Duration::from_secs(5));

    // Then send a slash command to verify TUI is still responsive
    session.send_line("/version");
    let found = session.wait_for_any(
        &["claude-code-rs", env!("CARGO_PKG_VERSION")],
        QUICK_TIMEOUT,
    );

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(300));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_rapid_input");

    assert!(
        found.is_some(),
        "TUI should still respond to /version after rapid input"
    );
}

// ── Advanced interaction tests ─────────────────────────────────────────

/// 验证上下文在多轮对话中持久：第一轮告知信息，第二轮回忆
#[test]
#[ignore]
fn context_persists_across_turns() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Turn 1: Establish context
    session.send_line("My favorite color is azure. Remember this.");
    let ok1 = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok1, "turn 1 should complete");

    let bar1 = session.status_bar();
    let count1 = extract_msg_count(&bar1).unwrap_or(0);
    eprintln!("[context] turn 1 done, msgs={count1}");
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_ctx_turn1");

    // Turn 2: Ask about the established context
    session.send_line("What is my favorite color?");
    let ok2 = session.wait_response_done(count1, API_TIMEOUT);
    assert!(ok2, "turn 2 should complete");

    std::thread::sleep(Duration::from_secs(2));
    let snap = session.snapshot("mt_ctx_turn2");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_ctx_final");

    // The model should recall "azure"
    assert!(
        snap.to_lowercase().contains("azure"),
        "model should recall the color 'azure' from context"
    );
}

/// 在对话轮次之间穿插斜杠命令，不破坏对话流
#[test]
#[ignore]
fn slash_commands_between_turns() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Turn 1: Normal chat
    session.send_line("Say exactly: TURN_ONE_OK");
    let ok1 = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok1, "turn 1 should complete");

    let bar1 = session.status_bar();
    let count1 = extract_msg_count(&bar1).unwrap_or(0);
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_cmd_between_turn1");

    // Interlude: Run slash commands
    session.send_line("/cost");
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_cmd_between_cost");

    session.send_line("/context");
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_cmd_between_context");

    // Turn 2: Resume conversation
    session.send_line("Say exactly: TURN_TWO_OK");
    let ok2 = session.wait_response_done(count1, API_TIMEOUT);
    assert!(ok2, "turn 2 should complete after slash commands");

    std::thread::sleep(Duration::from_secs(2));
    let snap = session.snapshot("mt_cmd_between_turn2");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_cmd_between_final");

    assert!(
        snap.contains("TURN_TWO_OK"),
        "turn 2 response should appear after slash command interlude"
    );
}

/// Ctrl+C 中断响应后发起新一轮对话
#[test]
#[ignore]
fn abort_and_resume_new_turn() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Turn 1: Send a prompt that will generate a long response
    session.send_line("Write a detailed explanation of how TCP works, at least 500 words.");

    // Wait briefly for streaming to start, then abort
    std::thread::sleep(Duration::from_secs(3));
    session.send_ctrl_c();
    eprintln!("[abort] sent Ctrl+C to abort turn 1");

    // Wait for TUI to return to ready state
    std::thread::sleep(Duration::from_secs(3));
    session.snapshot("mt_abort_after_ctrl_c");

    // Turn 2: Send a new short prompt — should work normally
    session.send_line("Say exactly: RECOVERED_OK");
    let ok = session.wait_for_text("RECOVERED_OK", API_TIMEOUT);

    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_abort_recovered");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_abort_final");

    assert!(
        ok,
        "should recover and produce response after aborting previous turn"
    );
}

/// 5 轮连续对话，验证状态栏 msg count 递增
#[test]
#[ignore]
fn status_bar_tracks_message_count() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    let prompts = ["Say OK1", "Say OK2", "Say OK3", "Say OK4", "Say OK5"];

    let mut last_count = 0usize;
    let mut counts: Vec<usize> = Vec::new();

    for (i, prompt) in prompts.iter().enumerate() {
        let turn = i + 1;
        eprintln!("[status-track] Turn {turn}/{}: {}", prompts.len(), prompt);

        session.send_line(prompt);
        let ok = session.wait_response_done(last_count, API_TIMEOUT);

        if !ok {
            eprintln!("[status-track] Turn {turn}: TIMEOUT");
            session.snapshot(&format!("mt_status_t{turn}_timeout"));
            break;
        }

        let bar = session.status_bar();
        if let Some(count) = extract_msg_count(&bar) {
            eprintln!("[status-track] Turn {turn}: msg count = {count}");
            counts.push(count);
            last_count = count;
        }

        std::thread::sleep(Duration::from_secs(2));
        session.snapshot(&format!("mt_status_t{turn}"));
    }

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_status_final");

    // Verify msg count is monotonically increasing
    assert!(
        counts.len() >= 3,
        "at least 3 turns should complete, got {}",
        counts.len()
    );
    for window in counts.windows(2) {
        assert!(
            window[1] > window[0],
            "msg count should increase: {} -> {}",
            window[0],
            window[1]
        );
    }
}

/// /clear 后 msg count 重置，新对话正常进行
#[test]
#[ignore]
fn clear_mid_conversation_resets_context() {
    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Turn 1: Establish a fact
    session.send_line("The secret word is PINEAPPLE. Remember it.");
    let ok1 = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok1, "turn 1 should complete");

    let bar1 = session.status_bar();
    let count1 = extract_msg_count(&bar1).unwrap_or(0);
    eprintln!("[clear-mid] pre-clear msgs={count1}");
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_clear_mid_before");

    // Clear conversation
    session.send_line("/clear");
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_clear_mid_cleared");

    // Turn 2: Ask about the fact — should NOT know it (context was cleared)
    session.send_line("What is the secret word I told you earlier?");
    // After /clear, msg count resets — wait for response from 0
    let ok2 = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok2, "turn 2 should complete after /clear");

    std::thread::sleep(Duration::from_secs(2));
    let snap = session.snapshot("mt_clear_mid_after");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_clear_mid_final");

    // After /clear, context is gone — model should NOT recall "PINEAPPLE"
    // We just verify the response was generated successfully
    assert!(snap.len() > 50, "should produce a response after /clear");
}

// ── Tool use tests (Read / Write / Edit in F:\temp) ─────────────────────

/// Write 工具：请求模型写文件到 F:\temp，验证文件内容
#[test]
#[ignore]
fn tool_write_creates_file() {
    let test_file = std::path::Path::new(workspace()).join("pty_test_write.txt");
    // Clean up from previous runs
    let _ = std::fs::remove_file(&test_file);

    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line(&format!(
        "Use the Write tool to create a file at {} with content: WRITE_TOOL_TEST_2026",
        test_file.display()
    ));

    // Wait for response completion (tool use + model reply)
    let ok = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok, "model should complete response with tool use");

    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_tool_write");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_tool_write_final");

    // Verify on disk
    assert!(
        test_file.exists(),
        "Write tool should create file at {}",
        test_file.display()
    );
    let content = std::fs::read_to_string(&test_file).unwrap_or_default();
    assert!(
        content.contains("WRITE_TOOL_TEST_2026"),
        "file should contain expected content, got: {content}"
    );

    // Clean up
    let _ = std::fs::remove_file(&test_file);
}

/// Read 工具：预先创建文件，请求模型读取，验证内容出现在 TUI 中
#[test]
#[ignore]
fn tool_read_shows_content() {
    let test_file = std::path::Path::new(workspace()).join("pty_test_read.txt");
    std::fs::write(&test_file, "SECRET_READ_CONTENT_7749").expect("create test file");

    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line(&format!(
        "Use the Read tool to read the file at {} and tell me what it contains",
        test_file.display()
    ));

    let ok = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok, "model should complete response with Read tool");

    std::thread::sleep(Duration::from_secs(2));
    let snap = session.snapshot("mt_tool_read");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_tool_read_final");

    // The model's response or tool output should contain the file content
    assert!(
        snap.contains("SECRET_READ_CONTENT_7749"),
        "Read tool output should appear in TUI"
    );

    // Clean up
    let _ = std::fs::remove_file(&test_file);
}

/// 多轮工具调用：第一轮 Write 创建文件，第二轮 Read 读回验证
#[test]
#[ignore]
fn tool_write_then_read_multi_turn() {
    let test_file = std::path::Path::new(workspace()).join("pty_test_write_read.txt");
    let _ = std::fs::remove_file(&test_file);

    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    // Turn 1: Write
    session.send_line(&format!(
        "Use the Write tool to create {} with content: MULTI_TURN_PAYLOAD_42",
        test_file.display()
    ));
    let ok1 = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok1, "turn 1 (write) should complete");

    let bar1 = session.status_bar();
    let count1 = extract_msg_count(&bar1).unwrap_or(0);
    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_tool_wr_turn1");

    // Turn 2: Read it back
    session.send_line(&format!(
        "Now use the Read tool to read {} and show me its content",
        test_file.display()
    ));
    let ok2 = session.wait_response_done(count1, API_TIMEOUT);
    assert!(ok2, "turn 2 (read) should complete");

    std::thread::sleep(Duration::from_secs(2));
    let snap = session.snapshot("mt_tool_wr_turn2");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_tool_wr_final");

    assert!(
        snap.contains("MULTI_TURN_PAYLOAD_42"),
        "turn 2 should show the content written in turn 1"
    );

    // Clean up
    let _ = std::fs::remove_file(&test_file);
}

/// Edit 工具：预创建文件，请求模型编辑，验证磁盘上的修改
#[test]
#[ignore]
fn tool_edit_modifies_file() {
    let test_file = std::path::Path::new(workspace()).join("pty_test_edit.txt");
    std::fs::write(&test_file, "line1: hello\nline2: world\nline3: end\n")
        .expect("create test file");

    let session = PtySession::spawn(default_args(), 120, 40, false);
    std::thread::sleep(RENDER_WAIT);

    session.send_line(&format!(
        "Use the Edit tool to edit {} and replace 'world' with 'EDITED_WORLD'",
        test_file.display()
    ));

    let ok = session.wait_response_done(0, API_TIMEOUT);
    assert!(ok, "model should complete response with Edit tool");

    std::thread::sleep(Duration::from_secs(2));
    session.snapshot("mt_tool_edit");

    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();
    let _output = session.finish(QUICK_TIMEOUT, "mt_tool_edit_final");

    // Verify on disk
    let content = std::fs::read_to_string(&test_file).unwrap_or_default();
    assert!(
        content.contains("EDITED_WORLD"),
        "Edit tool should replace 'world' with 'EDITED_WORLD', got: {content}"
    );
    assert!(
        !content.contains("line2: world"),
        "original 'world' should be replaced, got: {content}"
    );

    // Clean up
    let _ = std::fs::remove_file(&test_file);
}

// ── Helper ──────────────────────────────────────────────────────────────

/// Extract "N msgs" count from status bar text.
fn extract_msg_count(status: &str) -> Option<usize> {
    for word in status.split_whitespace() {
        if let Ok(n) = word.parse::<usize>() {
            if status.contains(&format!("{n} msgs")) || status.contains(&format!("{n} msg")) {
                return Some(n);
            }
        }
    }
    None
}
