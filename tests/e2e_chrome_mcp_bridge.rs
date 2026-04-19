//! E2E smoke test for the Claude-in-Chrome MCP bridge without a native host.
//!
//! The bridge must stay alive long enough to answer `initialize` and
//! `tools/list` even when Chrome has not launched the native host yet.

use assert_cmd::cargo::CommandCargoExt;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn send_json_line(stdin: &mut impl Write, payload: &str) {
    stdin.write_all(payload.as_bytes()).expect("write request");
    stdin.write_all(b"\n").expect("write newline");
    stdin.flush().expect("flush request");
}

#[test]
fn bridge_initializes_before_native_host_exists() {
    let mut child = Command::cargo_bin("claude-code-rs")
        .expect("binary not found")
        .arg("--claude-in-chrome-mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn --claude-in-chrome-mcp");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    break;
                }
            }
        }
    });

    send_json_line(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    let init = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("initialize response timeout")
        .expect("initialize response read failed");
    assert!(init.contains(r#""jsonrpc":"2.0""#), "unexpected init: {init}");
    assert!(init.contains(r#""claude-in-chrome""#), "unexpected init: {init}");

    send_json_line(&mut stdin, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    let tools = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("tools/list response timeout")
        .expect("tools/list response read failed");
    assert!(tools.contains(r#""tools""#), "unexpected tools/list: {tools}");
    assert!(tools.contains(r#""navigate""#), "unexpected tools/list: {tools}");

    drop(stdin);
    let exit = child.wait().expect("wait for bridge");
    assert!(exit.success(), "bridge exited with failure: {exit:?}");
}
