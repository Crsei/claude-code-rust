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

/// Cross-platform test workspace directory.
/// Uses `E2E_WORKSPACE` env var if set, otherwise platform default.
pub fn workspace() -> &'static str {
    static WS: OnceLock<String> = OnceLock::new();
    WS.get_or_init(|| {
        let dir = std::env::var("E2E_WORKSPACE").unwrap_or_else(|_| {
            if cfg!(windows) {
                r"F:\temp".to_string()
            } else {
                "/tmp/cc-rust-test".to_string()
            }
        });
        std::fs::create_dir_all(&dir).ok();
        dir
    })
}

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

/// Resolve the binary path. Uses cargo_bin() under cargo test, falls back to PATH for Docker.
pub fn binary_path() -> PathBuf {
    match std::panic::catch_unwind(|| assert_cmd::cargo::cargo_bin("claude-code-rs")) {
        Ok(p) if p.exists() => p,
        _ => which::which("claude-code-rs")
            .unwrap_or_else(|_| panic!("claude-code-rs binary not found via cargo_bin or PATH")),
    }
}

// ─── PtySession ──────────────────────────────────────────────────────

/// A pseudo-terminal session wrapping a child process.
pub struct PtySession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    slave: Option<Box<dyn portable_pty::SlavePty + Send>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
    cols: u16,
    rows: u16,
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

        let writer: Box<dyn Write + Send> = pair.master.take_writer().expect("take writer");
        let shared_writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));
        let writer_for_reader = Arc::clone(&shared_writer);

        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(64 * 1024)));
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
            cols,
            rows,
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
    ///
    /// NOTE: This returns ALL accumulated text from all frames overlaid.
    /// For the current screen state, use `current_screen()` instead.
    pub fn current_text(&self) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&buf);
        String::from_utf8_lossy(&plain).into_owned()
    }

    /// Get the current terminal screen content via vt100 emulation.
    ///
    /// Unlike `current_text()` which returns accumulated text from all frames,
    /// this returns the actual current screen state — what you'd see on the
    /// terminal right now. Useful for reading the status bar or checking
    /// the current UI state precisely.
    pub fn current_screen(&self) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&buf);
        parser.screen().contents()
    }

    /// Read a specific row from the current terminal screen (0-indexed).
    pub fn screen_row(&self, row: u16) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&buf);
        let screen = parser.screen();
        let mut line = String::new();
        for col in 0..self.cols {
            let cell = screen.cell(row, col).unwrap();
            let ch = cell.contents();
            if ch.is_empty() {
                line.push(' ');
            } else {
                line.push_str(&ch);
            }
        }
        line.trim_end().to_string()
    }

    /// Read the status bar (last row) from the current terminal screen.
    pub fn status_bar(&self) -> String {
        self.screen_row(self.rows - 1)
    }

    /// Wait until the status bar contains `needle`.
    pub fn wait_status(&self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return false;
            }
            if self.status_bar().contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Wait for the model to finish: status bar transitions to "ready"
    /// with a message count > `min_msgs`.
    pub fn wait_response_done(&self, min_msgs: usize, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return false;
            }
            let bar = self.status_bar();
            if bar.contains("ready") {
                // Parse msg count from "N msgs"
                if let Some(count) = parse_msg_count(&bar) {
                    if count > min_msgs {
                        return true;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
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

    // ── Mid-session snapshot ──────────────────────────────────────────

    /// Take a snapshot of the current terminal state without ending the session.
    /// Saves `.raw`, `.log`, and `.html` files and returns the plain text.
    pub fn snapshot(&self, label: &str) -> String {
        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        let dir = logs_dir();
        let raw_path = dir.join(format!("{label}.raw"));
        let log_path = dir.join(format!("{label}.log"));
        std::fs::write(&raw_path, &raw).expect("write raw");
        std::fs::write(&log_path, &plain).expect("write log");

        let output = CapturedOutput {
            raw,
            plain: plain.clone(),
            cols: self.cols,
            rows: self.rows,
        };
        let html_path = dir.join(format!("{label}.html"));
        let html = output.render_html();
        std::fs::write(&html_path, html.as_bytes()).expect("write html");

        eprintln!(
            "[snapshot] {label}: {} bytes → {}",
            output.raw.len(),
            html_path.display()
        );

        String::from_utf8_lossy(&plain).into_owned()
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

        let output = CapturedOutput {
            raw,
            plain,
            cols: self.cols,
            rows: self.rows,
        };

        // Render terminal screenshot as HTML
        let html_path = dir.join(format!("{test_name}.html"));
        let html = output.render_html();
        std::fs::write(&html_path, html.as_bytes()).expect("write html");

        output
    }
}

// ─── CapturedOutput ──────────────────────────────────────────────────

pub struct CapturedOutput {
    pub raw: Vec<u8>,
    pub plain: Vec<u8>,
    pub cols: u16,
    pub rows: u16,
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

    /// Render raw ANSI output through a vt100 terminal emulator → HTML screenshot.
    ///
    /// Strategy: process ALL data first. If the screen has content, use it
    /// (this handles mid-session snapshots correctly). If the screen is blank
    /// (exit cleanup cleared it), fall back to cleanup marker detection.
    pub fn render_html(&self) -> String {
        // First pass: process everything
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&self.raw);

        if parser.screen().contents().trim().is_empty() {
            // Screen is blank — exit cleanup cleared it. Re-parse, stopping
            // before the cleanup. We search for the cursor-home sequence
            // `\x1b[H` that is followed by line clears, working backwards.
            let markers: &[&[u8]] = &[
                b"\x1b[?1049l", // alternate screen off
                b"\x1b[2J",     // erase entire display
            ];

            // Find the last \x1b[H that leads into a blank screen
            let mut best_end = self.raw.len();

            // Check dedicated markers first
            for m in markers {
                if let Some(pos) = find_last_subsequence(&self.raw, m) {
                    best_end = best_end.min(pos);
                }
            }

            // Check for \x1b[H followed by \x1b[K (cursor home + erase line cleanup)
            // Walk backwards through all \x1b[H positions, try each as a cutoff
            let home_seq = b"\x1b[H";
            let mut search_end = self.raw.len();
            while let Some(pos) = find_last_subsequence(&self.raw[..search_end], home_seq) {
                let mut test_parser = vt100::Parser::new(self.rows, self.cols, 0);
                test_parser.process(&self.raw[..pos]);
                if !test_parser.screen().contents().trim().is_empty() {
                    best_end = best_end.min(pos);
                    break;
                }
                search_end = pos;
            }

            parser = vt100::Parser::new(self.rows, self.cols, 0);
            parser.process(&self.raw[..best_end]);
        }

        let screen = parser.screen();

        let mut html = String::with_capacity(self.raw.len() * 3);
        html.push_str(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Terminal Screenshot</title>
<style>
body {
  background: #1e1e1e;
  margin: 0;
  padding: 16px;
  display: flex;
  justify-content: center;
}
.terminal {
  background: #0c0c0c;
  border: 1px solid #444;
  border-radius: 8px;
  padding: 12px;
  box-shadow: 0 4px 24px rgba(0,0,0,0.5);
}
pre {
  font-family: 'Cascadia Code', 'Cascadia Mono', 'Consolas', 'Courier New', monospace;
  font-size: 14px;
  line-height: 1.3;
  margin: 0;
  color: #cccccc;
}
.row { display: block; height: 1.3em; white-space: pre; }
</style>
</head>
<body>
<div class="terminal">
<pre>
"#,
        );

        for row in 0..self.rows {
            html.push_str("<span class=\"row\">");
            let mut col = 0u16;
            while col < self.cols {
                let cell = screen.cell(row, col).unwrap();
                let ch = cell.contents();
                let fg = color_to_css(cell.fgcolor());
                let bg = color_to_css(cell.bgcolor());
                let bold = cell.bold();
                let underline = cell.underline();
                let inverse = cell.inverse();

                let (fg_css, bg_css) = if inverse {
                    (
                        bg.as_deref().unwrap_or("#0c0c0c"),
                        fg.as_deref().unwrap_or("#cccccc"),
                    )
                } else {
                    (
                        fg.as_deref().unwrap_or("#cccccc"),
                        bg.as_deref().unwrap_or("#0c0c0c"),
                    )
                };

                let mut style = String::new();
                if fg_css != "#cccccc" || inverse {
                    style.push_str(&format!("color:{fg_css};"));
                }
                if bg_css != "#0c0c0c" || inverse {
                    style.push_str(&format!("background:{bg_css};"));
                }
                if bold {
                    style.push_str("font-weight:bold;");
                }
                if underline {
                    style.push_str("text-decoration:underline;");
                }

                let display = if ch.is_empty() { " " } else { &ch };
                let escaped = html_escape(display);

                if style.is_empty() {
                    html.push_str(&escaped);
                } else {
                    html.push_str(&format!("<span style=\"{style}\">{escaped}</span>"));
                }

                // Wide characters take 2 columns
                let width = unicode_width::UnicodeWidthStr::width(ch.as_str());
                col += if width > 1 { width as u16 } else { 1 };
            }
            html.push_str("</span>\n");
        }

        html.push_str("</pre>\n</div>\n</body>\n</html>");
        html
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────

/// Parse "N msgs" from a status bar string, returning N.
fn parse_msg_count(status: &str) -> Option<usize> {
    // Matches patterns like "2 msgs", "10 msgs"
    for word in status.split_whitespace() {
        if let Ok(n) = word.parse::<usize>() {
            // Check if next token is "msgs" (approximate — just check the word after the number)
            if status.contains(&format!("{n} msgs")) || status.contains(&format!("{n} msg")) {
                return Some(n);
            }
        }
    }
    None
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Find the last occurrence of `needle` in `haystack`.
fn find_last_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// Map vt100::Color to CSS color string.
fn color_to_css(color: vt100::Color) -> Option<String> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(idx_to_css(i)),
        vt100::Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
    }
}

/// Standard 256-color palette → CSS hex.
fn idx_to_css(i: u8) -> String {
    // Standard 16 colors (Windows Terminal defaults)
    const PALETTE: [&str; 16] = [
        "#0c0c0c", "#c50f1f", "#13a10e", "#c19c00", "#0037da", "#881798", "#3a96dd", "#cccccc",
        "#767676", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e", "#61d6d6", "#f2f2f2",
    ];
    if (i as usize) < PALETTE.len() {
        return PALETTE[i as usize].to_string();
    }
    if i >= 232 {
        // Grayscale ramp: 232–255
        let v = 8 + (i - 232) as u32 * 10;
        let v = v.min(255) as u8;
        return format!("#{v:02x}{v:02x}{v:02x}");
    }
    // 6×6×6 color cube: 16–231
    let idx = (i - 16) as u32;
    let r = idx / 36;
    let g = (idx % 36) / 6;
    let b = idx % 6;
    let to_val = |c: u32| if c == 0 { 0u8 } else { (55 + c * 40) as u8 };
    format!("#{:02x}{:02x}{:02x}", to_val(r), to_val(g), to_val(b))
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Standard TUI args: `-C <workspace>`, permission bypass.
pub fn default_args() -> &'static [&'static str] {
    static ARGS: OnceLock<Vec<&'static str>> = OnceLock::new();
    ARGS.get_or_init(|| vec!["-C", workspace(), "--permission-mode", "bypass"])
        .as_slice()
}

/// Timeout for quick tests (version, init-only, etc.).
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for waiting for TUI to render.
pub const RENDER_WAIT: Duration = Duration::from_secs(3);

/// Timeout for API tests (network + model latency).
pub const API_TIMEOUT: Duration = Duration::from_secs(60);
