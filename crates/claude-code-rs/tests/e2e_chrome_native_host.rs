//! E2E smoke test for the Chrome native-host transport (Issue #5).
//!
//! Spawns `claude-code-rs --chrome-native-host` as a subprocess, sends a
//! framed `ping` message to its stdin, reads a framed `pong` response from
//! its stdout. Then closes stdin and confirms the process exits cleanly.
//!
//! This test does NOT exercise the MCP bridge or the Chrome extension — it
//! verifies only that:
//!
//! - The framing (4-byte LE length + JSON) is implemented correctly on both
//!   read and write.
//! - `ping` → `pong` round-trips.
//! - The process cleanly exits when Chrome closes stdin.
//!
//! Skipped on CI when file-system socket permissions can't be set; the test
//! is a startup sanity check, not the acceptance test for real browser
//! automation (that needs the Anthropic extension installed — see
//! `docs/reference/chrome-native-host.md`).

use assert_cmd::cargo::CommandCargoExt;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Write a framed Chrome-native-messaging message (4-byte LE length + JSON).
fn write_frame(writer: &mut impl Write, payload: &[u8]) -> std::io::Result<()> {
    let len = (payload.len() as u32).to_le_bytes();
    writer.write_all(&len)?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read one framed message from stdout. Times out after `deadline`.
fn read_frame(reader: &mut impl Read, deadline: Instant) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    read_exact_with_deadline(reader, &mut len_buf, deadline)?;
    let len = u32::from_le_bytes(len_buf);
    assert!(len < 1_000_000, "frame length {len} wildly oversized");
    let mut buf = vec![0u8; len as usize];
    read_exact_with_deadline(reader, &mut buf, deadline)?;
    Ok(buf)
}

fn read_exact_with_deadline(
    reader: &mut impl Read,
    buf: &mut [u8],
    deadline: Instant,
) -> std::io::Result<()> {
    let mut read_total = 0;
    while read_total < buf.len() {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "deadline reached while reading frame",
            ));
        }
        match reader.read(&mut buf[read_total..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "stdout closed mid-frame",
                ));
            }
            Ok(n) => read_total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[test]
fn ping_pong_round_trips_over_framed_stdio() {
    let mut child = Command::cargo_bin("claude-code-rs")
        .expect("binary not found")
        .arg("--chrome-native-host")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn --chrome-native-host");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout = child.stdout.take().expect("stdout piped");

    // Send ping.
    let ping = br#"{"type":"ping"}"#;
    write_frame(&mut stdin, ping).expect("write ping");

    // Read pong (give it 5 seconds — spawning a cargo-built binary can be slow on Windows).
    let deadline = Instant::now() + Duration::from_secs(5);
    let response = read_frame(&mut stdout, deadline).expect("read pong frame");
    let text = String::from_utf8(response).expect("utf8");
    assert!(
        text.contains(r#""type":"pong""#),
        "unexpected response: {text}"
    );
    assert!(
        text.contains(r#""timestamp""#),
        "pong missing timestamp: {text}"
    );

    // Close stdin; native host should exit cleanly.
    drop(stdin);
    let exit = child.wait().expect("wait for child");
    assert!(exit.success(), "native host exited with failure: {exit:?}");
}

#[test]
fn get_status_returns_version() {
    let mut child = Command::cargo_bin("claude-code-rs")
        .expect("binary not found")
        .arg("--chrome-native-host")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn --chrome-native-host");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout = child.stdout.take().expect("stdout piped");

    write_frame(&mut stdin, br#"{"type":"get_status"}"#).expect("write status");

    let deadline = Instant::now() + Duration::from_secs(5);
    let response = read_frame(&mut stdout, deadline).expect("read status_response");
    let text = String::from_utf8(response).expect("utf8");
    assert!(
        text.contains(r#""type":"status_response""#),
        "unexpected response: {text}"
    );
    assert!(
        text.contains("native_host_version"),
        "status_response missing version: {text}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn unknown_type_returns_error_message() {
    let mut child = Command::cargo_bin("claude-code-rs")
        .expect("binary not found")
        .arg("--chrome-native-host")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn --chrome-native-host");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout = child.stdout.take().expect("stdout piped");

    write_frame(&mut stdin, br#"{"type":"not_a_real_type"}"#).expect("write unknown");

    let deadline = Instant::now() + Duration::from_secs(5);
    let response = read_frame(&mut stdout, deadline).expect("read error frame");
    let text = String::from_utf8(response).expect("utf8");
    assert!(
        text.contains(r#""type":"error""#),
        "unexpected response: {text}"
    );

    drop(stdin);
    let _ = child.wait();
}
