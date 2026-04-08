//! Tool call display tests for headless IPC.
//!
//! Verifies that tool_use and tool_result information is visible in
//! the IPC message stream, so the frontend can render them.

use std::path::Path;
use std::time::Duration;

use crate::helpers::{collect_until, read_line_json, send_msg, spawn_headless, LIVE_TIMEOUT};

const WORKSPACE: &str = r"F:\temp";

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

// =========================================================================
//  Live: tool_use appears in IPC output when reading a file
// =========================================================================

/// When the model calls the Read tool, the assistant_message content should
/// contain a tool_use block with `name: "Read"` so the frontend can render it.
#[test]
#[ignore]
fn read_file_shows_tool_use_in_content() {
    let test_file = Path::new(WORKSPACE).join("_e2e_ipc_read.txt");
    std::fs::write(&test_file, "CANARY_IPC_READ_42").expect("write test file");

    let result = std::panic::catch_unwind(|| {
        let (mut child, mut stdin, mut stdout) = spawn_headless(
            &["-C", WORKSPACE, "--permission-mode", "bypass"],
            false,
        );

        let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
        assert_eq!(ready["type"], "ready");

        send_msg(
            &mut stdin,
            &serde_json::json!({
                "type": "submit_prompt",
                "text": format!(
                    "Use the Read tool to read the file at {}. Show me the contents.",
                    test_file.display()
                ),
                "id": "tool-display-001"
            }),
        );

        let messages = collect_until(
            &mut stdout,
            |msg| msg["type"] == "usage_update",
            LIVE_TIMEOUT,
        );

        // Check for tool_use in assistant_message content
        let assistant_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m["type"] == "assistant_message")
            .collect();

        assert!(
            !assistant_msgs.is_empty(),
            "should have at least one assistant_message, got types: {:?}",
            messages.iter().map(|m| m["type"].as_str()).collect::<Vec<_>>()
        );

        // The content array should contain a tool_use block
        let has_tool_use = assistant_msgs.iter().any(|m| {
            if let Some(content) = m["content"].as_array() {
                content.iter().any(|block| block["type"] == "tool_use")
            } else {
                false
            }
        });

        assert!(
            has_tool_use,
            "assistant_message content should contain a tool_use block, got: {:?}",
            assistant_msgs
        );

        // The tool_use block should name "Read"
        let has_read_tool = assistant_msgs.iter().any(|m| {
            m["content"]
                .as_array()
                .map(|arr| arr.iter().any(|b| b["name"] == "Read"))
                .unwrap_or(false)
        });

        assert!(
            has_read_tool,
            "tool_use block should name 'Read', got: {:?}",
            assistant_msgs
        );

        // The file content should appear somewhere in the response
        let all_json = format!("{:?}", messages);
        assert!(
            all_json.contains("CANARY_IPC_READ_42"),
            "file content should appear in response"
        );

        send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
        let status = child.wait().expect("wait");
        assert!(status.success());
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// When the model writes a file via the Write tool, tool_use with name "Write"
/// should appear in the content so the frontend can display it.
#[test]
#[ignore]
fn write_file_shows_tool_use_in_content() {
    let test_file = Path::new(WORKSPACE).join("_e2e_ipc_write.txt");
    cleanup(&test_file);

    let result = std::panic::catch_unwind(|| {
        let (mut child, mut stdin, mut stdout) = spawn_headless(
            &["-C", WORKSPACE, "--permission-mode", "bypass"],
            false,
        );

        let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
        assert_eq!(ready["type"], "ready");

        send_msg(
            &mut stdin,
            &serde_json::json!({
                "type": "submit_prompt",
                "text": format!(
                    "Use the Write tool to create the file {} with content: WRITE_IPC_OK",
                    test_file.display()
                ),
                "id": "tool-display-002"
            }),
        );

        let messages = collect_until(
            &mut stdout,
            |msg| msg["type"] == "usage_update",
            LIVE_TIMEOUT,
        );

        let has_write_tool = messages.iter().any(|m| {
            m["content"]
                .as_array()
                .map(|arr| arr.iter().any(|b| b["name"] == "Write"))
                .unwrap_or(false)
        });

        assert!(
            has_write_tool,
            "should have Write tool_use in content, types: {:?}",
            messages.iter().map(|m| m["type"].as_str()).collect::<Vec<_>>()
        );

        send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
        let status = child.wait().expect("wait");
        assert!(status.success());
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Bash tool execution should produce tool_use block with the command visible
/// in the input, so the frontend can render "Running: echo ..."
#[test]
#[ignore]
fn bash_tool_shows_command_in_input() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", WORKSPACE, "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo COMMAND_VISIBLE_TEST",
            "id": "tool-display-003"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    // Find tool_use blocks in assistant_messages
    let tool_uses: Vec<_> = messages
        .iter()
        .filter(|m| m["type"] == "assistant_message")
        .filter_map(|m| m["content"].as_array())
        .flat_map(|arr| arr.iter())
        .filter(|b| b["type"] == "tool_use" && b["name"] == "Bash")
        .collect();

    assert!(
        !tool_uses.is_empty(),
        "should have Bash tool_use blocks, messages: {:?}",
        messages.iter().map(|m| m["type"].as_str()).collect::<Vec<_>>()
    );

    // The input should contain the command
    let has_command = tool_uses.iter().any(|tu| {
        let input_str = format!("{}", tu["input"]);
        input_str.contains("echo") && input_str.contains("COMMAND_VISIBLE_TEST")
    });

    assert!(
        has_command,
        "Bash tool_use input should contain the command, got: {:?}",
        tool_uses
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

// =========================================================================
//  Live: large file truncation
// =========================================================================

/// When reading a file larger than the tool_result_budget (100K chars),
/// the output should be truncated with "characters omitted" marker.
#[test]
#[ignore]
fn large_file_read_shows_truncation() {
    let test_file = Path::new(WORKSPACE).join("_e2e_ipc_large.txt");

    // Create a file > 100K chars
    let large_content = "X".repeat(150_000);
    std::fs::write(&test_file, &large_content).expect("write large file");

    let result = std::panic::catch_unwind(|| {
        let (mut child, mut stdin, mut stdout) = spawn_headless(
            &["-C", WORKSPACE, "--permission-mode", "bypass"],
            false,
        );

        let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
        assert_eq!(ready["type"], "ready");

        send_msg(
            &mut stdin,
            &serde_json::json!({
                "type": "submit_prompt",
                "text": format!(
                    "Use the Read tool to read the file at absolute path {}. Tell me how many characters you see.",
                    test_file.display()
                ),
                "id": "large-file-001"
            }),
        );

        let messages = collect_until(
            &mut stdout,
            |msg| msg["type"] == "usage_update",
            Duration::from_secs(90),
        );

        // The full message dump should contain truncation markers
        let all_json = format!("{:?}", messages);

        let has_truncation = all_json.contains("characters omitted")
            || all_json.contains("truncated")
            || all_json.contains("omitted");

        assert!(
            has_truncation,
            "large file (150K chars) should show truncation marker, output size: {}",
            all_json.len()
        );

        send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
        let status = child.wait().expect("wait");
        assert!(status.success());
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// A normal-sized file should NOT show truncation markers.
#[test]
#[ignore]
fn small_file_read_no_truncation() {
    let test_file = Path::new(WORKSPACE).join("_e2e_ipc_small.txt");
    std::fs::write(&test_file, "This is a small file.\nNo truncation needed.").expect("write");

    let result = std::panic::catch_unwind(|| {
        let (mut child, mut stdin, mut stdout) = spawn_headless(
            &["-C", WORKSPACE, "--permission-mode", "bypass"],
            false,
        );

        let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
        assert_eq!(ready["type"], "ready");

        send_msg(
            &mut stdin,
            &serde_json::json!({
                "type": "submit_prompt",
                "text": format!(
                    "Use the Read tool to read {}. Show me the contents.",
                    test_file.display()
                ),
                "id": "small-file-001"
            }),
        );

        let messages = collect_until(
            &mut stdout,
            |msg| msg["type"] == "usage_update",
            LIVE_TIMEOUT,
        );

        let all_json = format!("{:?}", messages);

        assert!(
            all_json.contains("small file"),
            "small file content should appear in output"
        );
        assert!(
            !all_json.contains("characters omitted"),
            "small file should NOT be truncated"
        );

        send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
        let status = child.wait().expect("wait");
        assert!(status.success());
    });

    cleanup(&test_file);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
