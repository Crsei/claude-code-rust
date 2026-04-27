//! E2E tests using a real PTY (pseudo-terminal) to capture full terminal output.
//!
//! Unlike `e2e_terminal.rs` (which uses `--headless` JSONL), these tests spawn
//! the binary in a real PTY via `portable-pty`, capturing everything the
//! terminal would render — including ANSI escape sequences, TUI layout, etc.
//!
//! All captured output is saved to `logs/` for post-mortem debugging:
//! - `raw/*.raw` — raw bytes including ANSI escape sequences
//! - `log/*.log` — stripped plain text
//! - `aggregated/<test_name>.logs` — per-test combined logs
//! - `aggregated/all.logs` — aggregated logs across all PTY tests in this run
//!
//! The reader thread auto-responds to `\x1b[6n` (Device Status Report) queries
//! that crossterm sends on startup, preventing the process from blocking.
//!
//! Run:  cargo test --test e2e_pty
//! Live: cargo test --test e2e_pty -- --ignored

#[path = "test_workspace.rs"]
mod test_workspace;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

fn workspace() -> &'static str {
    test_workspace::workspace()
}

/// Timestamped log directories for this test run.
/// Format: `logs/YYYYMMDDHHMM/{raw,log}/` + `logs/aggregated/` — created once per process.
struct LogDirs {
    raw_dir: PathBuf,
    log_dir: PathBuf,
    aggregated_dir: PathBuf,
}

fn logs_dirs() -> &'static LogDirs {
    static DIRS: OnceLock<LogDirs> = OnceLock::new();
    DIRS.get_or_init(|| {
        let now = chrono::Local::now();
        let logs_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("logs");
        let base = logs_root.join(now.format("%Y%m%d%H%M").to_string());
        let raw_dir = base.join("raw");
        let log_dir = base.join("log");
        let aggregated_dir = logs_root.join("aggregated");

        std::fs::create_dir_all(&raw_dir).expect("create raw logs dir");
        std::fs::create_dir_all(&log_dir).expect("create plain logs dir");
        std::fs::create_dir_all(&aggregated_dir).expect("create aggregated logs dir");

        LogDirs {
            raw_dir,
            log_dir,
            aggregated_dir,
        }
    })
}

fn aggregated_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn write_aggregated_logs(
    dirs: &LogDirs,
    test_name: &str,
    raw_path: &Path,
    log_path: &Path,
    raw: &[u8],
    plain: &[u8],
) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let raw_text = String::from_utf8_lossy(raw);
    let plain_text = String::from_utf8_lossy(plain);

    let section = format!(
        "\n=== {test_name} ===\n\
timestamp: {timestamp}\n\
raw_file: {raw_file}\n\
log_file: {log_file}\n\
----- RAW (utf8-lossy) -----\n\
{raw_text}\n\
----- LOG (plain text) -----\n\
{plain_text}\n\
=== end {test_name} ===\n",
        raw_file = raw_path.display(),
        log_file = log_path.display(),
    );

    let _guard = aggregated_write_lock()
        .lock()
        .expect("lock aggregated logs mutex");

    let per_test_path = dirs.aggregated_dir.join(format!("{test_name}.logs"));
    std::fs::write(&per_test_path, section.as_bytes()).expect("write per-test aggregated log");

    let all_path = dirs.aggregated_dir.join("all.logs");
    let mut all_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&all_path)
        .expect("open aggregated all.logs");
    all_file
        .write_all(section.as_bytes())
        .expect("append aggregated all.logs");
}

/// Resolve the binary path. Uses cargo_bin() under cargo test, falls back to PATH for Docker.
fn binary_path() -> PathBuf {
    match std::panic::catch_unwind(|| assert_cmd::cargo::cargo_bin("claude-code-rs")) {
        Ok(p) if p.exists() => p,
        _ => which::which("claude-code-rs")
            .unwrap_or_else(|_| panic!("claude-code-rs binary not found via cargo_bin or PATH")),
    }
}

/// A PTY session that captures all terminal output while auto-responding
/// to terminal queries (DSR `\x1b[6n`).
///
/// Holds the slave handle to prevent premature ConPTY teardown on Windows.
/// The slave is dropped in `finish()` after the child exits, ensuring all
/// buffered output is flushed to the reader.
struct PtySession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    slave: Option<Box<dyn portable_pty::SlavePty + Send>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl PtySession {
    fn spawn(args: &[&str], cols: u16, rows: u16, strip_keys: bool) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");

        let mut cmd = CommandBuilder::new(binary_path());
        for arg in args {
            cmd.arg(*arg);
        }

        if strip_keys {
            cmd.env("ANTHROPIC_API_KEY", "");
            cmd.env("AZURE_API_KEY", "");
            cmd.env("OPENAI_API_KEY", "");
            cmd.env("OPENROUTER_API_KEY", "");
            cmd.env("GOOGLE_API_KEY", "");
            cmd.env("DEEPSEEK_API_KEY", "");
        }

        let child = pair.slave.spawn_command(cmd).expect("spawn in pty");
        // Keep slave alive — dropping it prematurely on Windows ConPTY
        // can cause output to be lost before the reader drains it.

        let writer: Box<dyn Write + Send> = pair.master.take_writer().expect("take pty writer");
        let shared_writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));
        let writer_for_reader = Arc::clone(&shared_writer);

        let mut reader = pair.master.try_clone_reader().expect("clone pty reader");
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(64 * 1024)));
        let buf_clone = Arc::clone(&buffer);

        // Background thread: drain PTY output and auto-respond to DSR queries.
        let reader_thread = std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            let mut tail = Vec::with_capacity(16);
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &chunk[..n];
                        buf_clone.lock().unwrap().extend_from_slice(data);

                        // Detect \x1b[6n (Device Status Report) and respond
                        // with \x1b[1;1R (cursor at row 1, col 1).
                        tail.extend_from_slice(data);
                        while let Some(pos) = find_subsequence(&tail, b"\x1b[6n") {
                            if let Ok(mut w) = writer_for_reader.lock() {
                                let _ = w.write_all(b"\x1b[1;1R");
                                let _ = w.flush();
                            }
                            tail.drain(..pos + 4);
                        }
                        if tail.len() > 16 {
                            let start = tail.len() - 16;
                            tail.drain(..start);
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::BrokenPipe {
                            eprintln!("[pty reader] error: {}", e);
                        }
                        break;
                    }
                }
            }
        });

        Self {
            writer: shared_writer,
            child,
            slave: Some(pair.slave),
            buffer,
            reader_thread: Some(reader_thread),
        }
    }

    /// Send a line of text followed by Enter (\r).
    fn send_line(&self, line: &str) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(format!("{}\r", line).as_bytes())
            .expect("write line");
        w.flush().expect("flush");
    }

    /// Send Ctrl+C (ETX byte 0x03).
    fn send_ctrl_c(&self) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(&[0x03]).expect("write ctrl-c");
        w.flush().expect("flush");
    }

    /// Wait for child to exit (with timeout) and return captured output.
    fn finish(mut self, timeout: Duration, test_name: &str) -> CapturedOutput {
        let start = Instant::now();

        // Poll child exit with timeout
        loop {
            if start.elapsed() > timeout {
                eprintln!("[pty] timeout — killing child");
                let _ = self.child.kill();
                break;
            }
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }

        // Give ConPTY time to flush remaining output
        std::thread::sleep(Duration::from_millis(200));

        // Drop slave to signal EOF to reader
        drop(self.slave.take());

        // Drop writer to unblock reader if it's waiting
        drop(self.writer);

        // Wait for reader thread to finish
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }

        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        let dirs = logs_dirs();
        let raw_path = dirs.raw_dir.join(format!("{}.raw", test_name));
        let log_path = dirs.log_dir.join(format!("{}.log", test_name));
        std::fs::write(&raw_path, &raw).expect("write raw log");
        std::fs::write(&log_path, &plain).expect("write plain log");
        write_aggregated_logs(dirs, test_name, &raw_path, &log_path, &raw, &plain);

        eprintln!(
            "[pty] captured {} bytes raw, {} bytes plain → {}",
            raw.len(),
            plain.len(),
            log_path.display()
        );

        CapturedOutput { raw, plain }
    }

    /// Wait until captured output contains `needle` (plain text), or timeout.
    fn wait_for_text(&self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return false;
            }
            let buf = self.buffer.lock().unwrap().clone();
            let plain = strip_ansi_escapes::strip(&buf);
            let text = String::from_utf8_lossy(&plain);
            if text.contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

/// Find the first occurrence of `needle` in `haystack`.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct CapturedOutput {
    /// Raw bytes including ANSI escape sequences.
    raw: Vec<u8>,
    /// Plain text with ANSI sequences stripped.
    plain: Vec<u8>,
}

impl CapturedOutput {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.plain).into_owned()
    }

    fn contains(&self, needle: &str) -> bool {
        self.text().contains(needle)
    }
}

// =========================================================================
//  Tests
// =========================================================================

/// `--version` in a PTY should print version info and exit.
#[test]
fn pty_version_flag() {
    let session = PtySession::spawn(&["-V"], 120, 40, false);
    let output = session.finish(Duration::from_secs(10), "pty_version_flag");

    assert!(
        output.contains("claude-code-rs"),
        "should contain version string, got: [{}]",
        output.text()
    );
}

/// `--init-only` should exit cleanly and produce log files.
#[test]
fn pty_init_only() {
    let session = PtySession::spawn(&["--init-only"], 120, 40, false);
    let output = session.finish(Duration::from_secs(10), "pty_init_only");

    // Verify log files were created
    let dirs = logs_dirs();
    assert!(dirs.raw_dir.join("pty_init_only.raw").exists());
    assert!(dirs.log_dir.join("pty_init_only.log").exists());
    assert!(dirs.aggregated_dir.join("pty_init_only.logs").exists());
    assert!(dirs.aggregated_dir.join("all.logs").exists());

    // Should not have panicked
    assert!(
        !output.contains("panicked"),
        "should not panic, got: {}",
        output.text()
    );
}

/// `--dump-system-prompt` captures the full system prompt in the log.
#[test]
fn pty_dump_system_prompt() {
    let session = PtySession::spawn(&["--dump-system-prompt", "-C", workspace()], 200, 50, false);
    let output = session.finish(Duration::from_secs(10), "pty_dump_system_prompt");

    assert!(
        output.contains("tool") || output.contains("Tool"),
        "system prompt should mention tools, got {} bytes: [{}]",
        output.plain.len(),
        &output.text()[..output.text().len().min(200)]
    );
}

/// The TUI should start and render something when launched without --headless.
#[test]
fn pty_tui_starts_and_captures_output() {
    let session = PtySession::spawn(
        &["-C", workspace(), "--permission-mode", "bypass"],
        120,
        40,
        false,
    );

    // Wait for TUI to render
    std::thread::sleep(Duration::from_secs(3));

    // Send Ctrl+C to quit
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();

    let output = session.finish(Duration::from_secs(10), "pty_tui_starts");

    assert!(
        output.raw.len() > 10,
        "PTY should have captured terminal output, got {} bytes",
        output.raw.len()
    );

    let text = output.text();
    let preview_end = text
        .char_indices()
        .take_while(|(i, _)| *i < 500)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(text.len());
    eprintln!("TUI output preview:\n{}", &text[..preview_end]);
}

/// Launch TUI, send a simple prompt via PTY input, capture the full session.
#[test]
fn live_pty_simple_chat() {
    let session = PtySession::spawn(
        &["-C", workspace(), "--permission-mode", "bypass"],
        120,
        40,
        false,
    );

    // Wait for TUI to be ready
    std::thread::sleep(Duration::from_secs(3));

    // Type a prompt and press Enter
    session.send_line("Say exactly: PTY_TEST_OK");

    // Wait for the response to appear
    let found = session.wait_for_text("PTY_TEST_OK", Duration::from_secs(60));

    // Quit
    session.send_ctrl_c();
    std::thread::sleep(Duration::from_millis(500));
    session.send_ctrl_c();

    let output = session.finish(Duration::from_secs(10), "live_pty_simple_chat");

    assert!(
        found,
        "should find PTY_TEST_OK in output, got:\n{}",
        output.text()
    );
}

/// Launch in print mode (-p), capture full output to log.
#[test]
fn live_pty_print_mode() {
    let session = PtySession::spawn(
        &["-p", "Say exactly: PTY_PRINT_OK", "-C", workspace()],
        120,
        40,
        false,
    );

    let output = session.finish(Duration::from_secs(60), "live_pty_print_mode");

    assert!(
        output.contains("PTY_PRINT_OK"),
        "print mode output should contain PTY_PRINT_OK, got:\n{}",
        output.text()
    );
}
