//! Shared PTY test harness.
//!
//! Provides `PtySession` — a pseudo-terminal wrapper that:
//! - Spawns `claude-code-rs` in a real ConPTY
//! - Captures all terminal output (ANSI + plain text)
//! - Auto-responds to DSR `\x1b[6n` queries
//! - Saves timestamped logs to `logs/YYYYMMDDHHMM/`

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Timestamped log directory — created once per test process.
pub fn logs_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let now = chrono::Local::now();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("logs")
            .join(now.format("%Y%m%d%H%M").to_string());
        std::fs::create_dir_all(&dir).expect("create logs dir");
        dir
    })
}

/// Path to the compiled binary.
pub fn binary_path() -> PathBuf {
    assert_cmd::cargo::cargo_bin("claude-code-rs")
}

// ─── PtySession ──────────────────────────────────────────────────────

/// A pseudo-terminal session wrapping a child process.
pub struct PtySession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    slave: Option<Box<dyn portable_pty::SlavePty + Send>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl PtySession {
    /// Spawn `claude-code-rs` in a PTY with the given args and terminal size.
    ///
    /// If `strip_keys` is true, all API key env vars are cleared so the binary
    /// runs in offline mode (useful for UI-only tests).
    pub fn spawn(args: &[&str], cols: u16, rows: u16, strip_keys: bool) -> Self {
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

        let writer: Box<dyn Write + Send> =
            pair.master.take_writer().expect("take writer");
        let shared_writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(writer));
        let writer_for_reader = Arc::clone(&shared_writer);

        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let buffer: Arc<Mutex<Vec<u8>>> =
            Arc::new(Mutex::new(Vec::with_capacity(64 * 1024)));
        let buf_clone = Arc::clone(&buffer);

        let reader_thread = std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            let mut tail = Vec::with_capacity(16);
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &chunk[..n];
                        buf_clone.lock().unwrap().extend_from_slice(data);

                        // Auto-respond to DSR \x1b[6n → \x1b[1;1R
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
                            eprintln!("[pty reader] error: {e}");
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

    // ── Input simulation ────────────────────────────────────────────

    /// Send raw bytes to the PTY (child's stdin).
    pub fn send_raw(&self, bytes: &[u8]) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes).expect("write raw");
        w.flush().expect("flush");
    }

    /// Send a line of text followed by Enter (`\r`).
    pub fn send_line(&self, line: &str) {
        self.send_raw(format!("{}\r", line).as_bytes());
    }

    /// Send Ctrl+C (ETX 0x03).
    pub fn send_ctrl_c(&self) {
        self.send_raw(&[0x03]);
    }

    /// Send Ctrl+D (EOT 0x04).
    pub fn send_ctrl_d(&self) {
        self.send_raw(&[0x04]);
    }

    /// Send Ctrl+G (BEL 0x07 — vim toggle in cc-rust).
    pub fn send_ctrl_g(&self) {
        self.send_raw(&[0x07]);
    }

    /// Send Up arrow (ANSI: ESC [ A).
    pub fn send_up(&self) {
        self.send_raw(b"\x1b[A");
    }

    /// Send Down arrow (ANSI: ESC [ B).
    pub fn send_down(&self) {
        self.send_raw(b"\x1b[B");
    }

    /// Send Escape key.
    pub fn send_escape(&self) {
        self.send_raw(&[0x1b]);
    }

    // ── Output inspection ───────────────────────────────────────────

    /// Get current captured output as plain text (ANSI stripped), non-blocking.
    pub fn current_text(&self) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&buf);
        String::from_utf8_lossy(&plain).into_owned()
    }

    /// Wait until captured output contains `needle` (ANSI stripped).
    pub fn wait_for_text(&self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return false;
            }
            if self.current_text().contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Wait for any of the given needles. Returns the index that matched, or None.
    pub fn wait_for_any(&self, needles: &[&str], timeout: Duration) -> Option<usize> {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return None;
            }
            let text = self.current_text();
            for (i, needle) in needles.iter().enumerate() {
                if text.contains(needle) {
                    return Some(i);
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // ── Finish & save ───────────────────────────────────────────────

    /// Wait for child to exit, save logs, return captured output.
    pub fn finish(mut self, timeout: Duration, test_name: &str) -> CapturedOutput {
        let start = Instant::now();
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

        std::thread::sleep(Duration::from_millis(200));
        drop(self.slave.take());
        drop(self.writer);
        if let Some(h) = self.reader_thread.take() {
            let _ = h.join();
        }

        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        let dir = logs_dir();
        let raw_path = dir.join(format!("{test_name}.raw"));
        let log_path = dir.join(format!("{test_name}.log"));
        std::fs::write(&raw_path, &raw).expect("write raw");
        std::fs::write(&log_path, &plain).expect("write log");

        eprintln!(
            "[pty] {test_name}: {} bytes raw, {} bytes plain → {}",
            raw.len(),
            plain.len(),
            log_path.display()
        );

        CapturedOutput { raw, plain }
    }
}

// ─── CapturedOutput ──────────────────────────────────────────────────

pub struct CapturedOutput {
    pub raw: Vec<u8>,
    pub plain: Vec<u8>,
}

impl CapturedOutput {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.plain).into_owned()
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.text().contains(needle)
    }

    /// Print a Unicode-safe preview of the plain text (up to `max_bytes`).
    pub fn preview(&self, max_bytes: usize) {
        let text = self.text();
        let end = text
            .char_indices()
            .take_while(|(i, _)| *i < max_bytes)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(text.len());
        eprintln!("[preview]\n{}", &text[..end]);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

/// Standard TUI args: `-C F:\temp`, permission bypass.
pub const DEFAULT_ARGS: &[&str] = &["-C", r"F:\temp", "--permission-mode", "bypass"];

/// Timeout for quick tests (version, init-only, etc.).
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for waiting for TUI to render.
pub const RENDER_WAIT: Duration = Duration::from_secs(3);

/// Timeout for API tests (network + model latency).
pub const API_TIMEOUT: Duration = Duration::from_secs(60);
