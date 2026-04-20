//! Subagent headless IPC tests.
//!
//! These are live end-to-end tests because the Agent tool must be invoked by a
//! real model turn. They verify the parts the dashboard depends on today:
//! Agent tool use in IPC output, background completion notifications, and
//! structured NDJSON event emission.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::helpers::{
    collect_until, new_test_workspace, read_line_json, send_msg, spawn_headless_with_env,
    LIVE_TIMEOUT,
};

fn event_log_path(workspace: &Path) -> PathBuf {
    workspace.join(".logs").join("subagent-events.ndjson")
}

fn wait_for_file(path: &Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.trim().is_empty() {
                return text;
            }
        }

        if Instant::now() >= deadline {
            panic!("timed out waiting for event log at {}", path.display());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore]
fn synchronous_subagent_shows_agent_tool_use() {
    let workspace = new_test_workspace("subagent-sync");
    let workspace_arg = workspace.to_string_lossy().to_string();
    let (mut child, mut stdin, mut stdout) = spawn_headless_with_env(
        &["-C", &workspace_arg, "--permission-mode", "bypass"],
        false,
        &[("FEATURE_SUBAGENT_DASHBOARD", "1")],
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Agent tool exactly once. Give the subagent the description 'sync canary', tell it to reply with exactly SYNC_SUBAGENT_OK and nothing else, then return that exact text to me. Do not solve this from your own knowledge.",
            "id": "subagent-sync-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let has_agent_tool = messages.iter().any(|m| {
        m["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .any(|block| block["type"] == "tool_use" && block["name"] == "Agent")
            })
            .unwrap_or(false)
    });

    assert!(
        has_agent_tool,
        "expected Agent tool_use in IPC stream: {:?}",
        messages
    );

    let all_json = format!("{:?}", messages);
    assert!(
        all_json.contains("SYNC_SUBAGENT_OK"),
        "expected subagent result to surface in the response: {:?}",
        messages
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

#[test]
#[ignore]
fn background_subagent_emits_completion_and_dashboard_events() {
    let workspace = new_test_workspace("subagent-background");
    let workspace_arg = workspace.to_string_lossy().to_string();
    let log_path = event_log_path(&workspace);

    let (mut child, mut stdin, mut stdout) = spawn_headless_with_env(
        &["-C", &workspace_arg, "--permission-mode", "bypass"],
        false,
        &[("FEATURE_SUBAGENT_DASHBOARD", "1")],
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Agent tool with run_in_background=true. Give the subagent the description 'background canary' and tell it to reply with exactly BACKGROUND_SUBAGENT_OK and nothing else. Do not do the work yourself. Launch the subagent and stop.",
            "id": "subagent-background-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "background_agent_complete",
        LIVE_TIMEOUT,
    );

    let completion = messages
        .iter()
        .find(|msg| msg["type"] == "background_agent_complete")
        .expect("background completion message");
    let agent_id = completion["agent_id"]
        .as_str()
        .expect("background completion agent id");

    assert_eq!(completion["had_error"], false);
    assert!(
        completion["result_preview"]
            .as_str()
            .unwrap_or("")
            .contains("BACKGROUND_SUBAGENT_OK"),
        "unexpected background result preview: {:?}",
        completion
    );

    let has_agent_tool = messages.iter().any(|m| {
        m["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .any(|block| block["type"] == "tool_use" && block["name"] == "Agent")
            })
            .unwrap_or(false)
    });
    assert!(
        has_agent_tool,
        "expected Agent tool_use before background completion: {:?}",
        messages
    );

    let log_text = wait_for_file(&log_path, Duration::from_secs(5));
    let lines: Vec<serde_json::Value> = log_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse ndjson line"))
        .collect();

    let spawn = lines
        .iter()
        .find(|line| line["kind"] == "spawn" && line["agent_id"].as_str() == Some(agent_id));
    let complete = lines.iter().find(|line| {
        line["kind"] == "background_complete" && line["agent_id"].as_str() == Some(agent_id)
    });

    assert!(
        spawn.is_some(),
        "missing spawn event for agent {}: {:?}",
        agent_id,
        lines
    );
    assert!(
        complete.is_some(),
        "missing background_complete event for agent {}: {:?}",
        agent_id,
        lines
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
