//! Phase 6 regression coverage for coordinator/team/tasks headless flows.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::helpers::{
    collect_until, new_test_workspace, read_line_json, send_msg, spawn_headless_with_env,
    LINE_TIMEOUT,
};

fn send_slash_and_wait<F>(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    raw: &str,
    timeout: Duration,
    predicate: F,
) -> serde_json::Value
where
    F: Fn(&serde_json::Value) -> bool,
{
    send_msg(
        stdin,
        &serde_json::json!({
            "type": "slash_command",
            "raw": raw,
        }),
    );

    let messages = collect_until(stdout, predicate, timeout);
    messages
        .last()
        .cloned()
        .expect("expected slash command response")
}

fn response_text(message: &serde_json::Value) -> &str {
    message["text"].as_str().unwrap_or("")
}

fn mailbox_path(home: &Path, team_name: &str, agent_name: &str) -> PathBuf {
    home.join("teams")
        .join(team_name)
        .join("inboxes")
        .join(format!("{}.json", agent_name))
}

fn wait_for_json_array(path: &Path, timeout: Duration) -> serde_json::Value {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = fs::read_to_string(path) {
            if !text.trim().is_empty() {
                return serde_json::from_str(&text)
                    .unwrap_or_else(|err| panic!("failed to parse {}: {}", path.display(), err));
            }
        }

        if Instant::now() >= deadline {
            panic!("timed out waiting for {}", path.display());
        }

        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn coordinator_team_tasks_and_mailbox_lifecycle_are_wired() {
    let home = tempfile::tempdir().expect("create temp home");
    let workspace = new_test_workspace("phase6-coordinator");
    let home_arg = home.path().to_string_lossy().to_string();
    let workspace_arg = workspace.to_string_lossy().to_string();

    let (mut child, mut stdin, mut stdout) = spawn_headless_with_env(
        &["-C", &workspace_arg, "--permission-mode", "bypass"],
        true,
        &[("ALLTHECODES_HOME", &home_arg)],
    );

    let ready = read_line_json(&mut stdout, LINE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    let team_name = "phase6-team";
    let worker_name = "worker-one";

    let start = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        &format!("/coordinator start {team_name}"),
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Coordinator mode enabled and team")
        },
    );
    assert!(response_text(&start).contains(team_name));

    let status = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        "/coordinator status",
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Coordinator mode: ON")
        },
    );
    let status_text = response_text(&status);
    assert!(status_text.contains("Coordinator mode: ON"));
    assert!(status_text.contains("Active team: phase6-team"));

    let spawn = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        &format!("/team spawn {worker_name} Return exactly WORKER_OK and nothing else."),
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Spawned 'worker-one'")
        },
    );
    let spawn_text = response_text(&spawn);
    assert!(spawn_text.contains("Spawned 'worker-one'"));
    assert!(spawn_text.contains("task_id="));

    let tasks = send_slash_and_wait(&mut stdin, &mut stdout, "/tasks", LINE_TIMEOUT, |msg| {
        msg["type"] == "system_info"
            && msg["text"]
                .as_str()
                .unwrap_or("")
                .contains("Team tasks (1)")
    });
    let tasks_text = response_text(&tasks);
    assert!(tasks_text.contains("Team tasks (1)"));
    assert!(tasks_text.contains("worker-one (phase6-team)"));
    assert!(tasks_text.contains("team:running"));

    let send = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        &format!("/team send {worker_name} ping-from-lead"),
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Message queued for 'worker-one'")
        },
    );
    assert!(response_text(&send).contains("Message queued for 'worker-one'."));

    let inbox_path = mailbox_path(home.path(), team_name, worker_name);
    let inbox = wait_for_json_array(&inbox_path, Duration::from_secs(5));
    let messages = inbox.as_array().expect("mailbox array");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["from"], "team-lead");
    assert_eq!(messages[0]["text"], "ping-from-lead");

    let kill = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        &format!("/team kill {worker_name}"),
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Killed 'worker-one'")
        },
    );
    assert!(response_text(&kill).contains("Killed 'worker-one'"));

    let tasks_after = send_slash_and_wait(&mut stdin, &mut stdout, "/tasks", LINE_TIMEOUT, |msg| {
        msg["type"] == "system_info" && msg["text"].as_str().unwrap_or("").contains("team:stopped")
    });
    let tasks_after_text = response_text(&tasks_after);
    assert!(tasks_after_text.contains("worker-one (phase6-team)"));
    assert!(tasks_after_text.contains("team:stopped"));

    let stop = send_slash_and_wait(
        &mut stdin,
        &mut stdout,
        "/coordinator stop",
        LINE_TIMEOUT,
        |msg| {
            msg["type"] == "system_info"
                && msg["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Coordinator mode disabled")
        },
    );
    assert!(response_text(&stop).contains("Coordinator mode disabled"));

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait for headless child");
    assert!(status.success(), "headless process should exit cleanly");
}
