//! PTY 测试 harness — 封装伪终端的创建、输入输出和屏幕断言。
//!
//! 提供 `PtySession`：在真实 PTY 中启动 `claude-code-rs`，模拟键盘输入，
//! 通过 `vt100` 解析器读取当前屏幕内容，用于 TUI 行为断言。

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// ─── 路径与配置 ──────────────────────────────────────────────────────

/// 测试工作区目录。优先使用 `E2E_WORKSPACE` 环境变量。
pub fn workspace() -> &'static str {
    static WS: OnceLock<String> = OnceLock::new();
    WS.get_or_init(|| {
        let dir =
            std::env::var("E2E_WORKSPACE").unwrap_or_else(|_| "/tmp/cc-rust-e2e-test".to_string());
        std::fs::create_dir_all(&dir).ok();
        dir
    })
}

/// 日志输出目录（按时间戳命名，每轮测试进程共享一个）。
pub fn logs_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let now = chrono::Local::now();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("logs")
            .join(format!("pty_tui_e2e_{}", now.format("%Y%m%d%H%M")));
        std::fs::create_dir_all(&dir).expect("create logs dir");
        dir
    })
}

/// 创建并返回测试专属子目录：`logs/pty_tui_e2e_{timestamp}/{test_name}/`。
pub fn test_subdir(test_name: &str) -> PathBuf {
    let dir = logs_dir().join(test_name);
    std::fs::create_dir_all(&dir).expect("create test subdir");
    dir
}

/// 解析二进制路径：cargo test 环境用 cargo_bin()，否则查 PATH。
pub fn binary_path() -> PathBuf {
    match std::panic::catch_unwind(|| assert_cmd::cargo::cargo_bin("claude-code-rs")) {
        Ok(p) if p.exists() => p,
        _ => which::which("claude-code-rs")
            .unwrap_or_else(|_| panic!("claude-code-rs binary not found via cargo_bin or PATH")),
    }
}

/// 标准 TUI 启动参数：指定工作区 + bypass 权限（UI 测试不需要权限拦截）。
pub fn default_args() -> Vec<&'static str> {
    vec!["-C", workspace(), "--permission-mode", "bypass"]
}

/// 跳过 workspace trust gate（首次运行时显示的信任确认界面）。
/// 如果屏幕上出现 "trust" 相关文本，按 Enter 接受。
pub fn skip_trust_gate(session: &PtySession) {
    // 给 TUI 一些时间渲染
    std::thread::sleep(Duration::from_millis(500));
    let screen = session.current_screen();
    if screen.contains("trust") || screen.contains("Trust") || screen.contains("safety check") {
        session.send_raw(b"\r");
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// 快速超时：UI 渲染等待。
pub const RENDER_WAIT: Duration = Duration::from_secs(3);

/// 快速超时：退出等待。
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(10);

/// API 超时：网络 + 模型延迟。
pub const API_TIMEOUT: Duration = Duration::from_secs(60);

// ─── PtySession ──────────────────────────────────────────────────────

/// 伪终端会话：在真实 PTY 中启动二进制，捕获输出，模拟输入。
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
    /// 启动 cc-rust TUI 进程。
    ///
    /// - `args`: 命令行参数（不含二进制名）
    /// - `cols`, `rows`: 终端尺寸
    /// - `strip_keys`: 是否清除 API key（离线模式）
    pub fn spawn(args: &[&str], cols: u16, rows: u16, strip_keys: bool) -> Self {
        Self::spawn_with_env(args, cols, rows, strip_keys, &[])
    }

    /// 启动 cc-rust TUI 进程，附加自定义环境变量。
    pub fn spawn_with_env(
        args: &[&str],
        cols: u16,
        rows: u16,
        strip_keys: bool,
        envs: &[(&str, &str)],
    ) -> Self {
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

        // 离线模式：清除所有 API key
        if strip_keys {
            cmd.env("ANTHROPIC_API_KEY", "");
            cmd.env("AZURE_API_KEY", "");
            cmd.env("OPENAI_API_KEY", "");
            cmd.env("OPENROUTER_API_KEY", "");
            cmd.env("GOOGLE_API_KEY", "");
            cmd.env("DEEPSEEK_API_KEY", "");
        }

        for (key, value) in envs {
            cmd.env(key, value);
        }

        let child = pair.slave.spawn_command(cmd).expect("spawn in pty");

        // 读写器
        let writer: Box<dyn Write + Send> = pair.master.take_writer().expect("take writer");
        let shared_writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));
        let writer_for_reader = Arc::clone(&shared_writer);

        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(64 * 1024)));
        let buf_clone = Arc::clone(&buffer);

        // 读取线程：持续捕获 PTY 输出 + 自动回复 DSR 查询
        let reader_thread = std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            let mut tail = Vec::with_capacity(16);
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &chunk[..n];
                        buf_clone.lock().unwrap().extend_from_slice(data);

                        // 自动回复 DSR \x1b[6n → \x1b[1;1R
                        // crossterm 在初始化时会发送此查询，不回复会导致阻塞
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

    // ── 输入模拟 ─────────────────────────────────────────────

    /// 发送原始字节到 PTY stdin。
    pub fn send_raw(&self, bytes: &[u8]) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes).expect("write raw");
        w.flush().expect("flush");
    }

    /// 发送一行文本 + Enter。
    pub fn send_line(&self, line: &str) {
        self.send_raw(format!("{}\r", line).as_bytes());
    }

    /// Ctrl+C (ETX 0x03)。
    pub fn send_ctrl_c(&self) {
        self.send_raw(&[0x03]);
    }

    /// Ctrl+D (EOT 0x04) — 退出。
    pub fn send_ctrl_d(&self) {
        self.send_raw(&[0x04]);
    }

    /// Ctrl+G (BEL 0x07) — vim 模式切换。
    #[allow(dead_code)]
    pub fn send_ctrl_g(&self) {
        self.send_raw(&[0x07]);
    }

    /// Up arrow。
    pub fn send_up(&self) {
        self.send_raw(b"\x1b[A");
    }

    /// Down arrow。
    pub fn send_down(&self) {
        self.send_raw(b"\x1b[B");
    }

    /// Escape 键。
    pub fn send_escape(&self) {
        self.send_raw(&[0x1b]);
    }

    /// Tab 键。
    #[allow(dead_code)]
    pub fn send_tab(&self) {
        self.send_raw(&[0x09]);
    }

    /// Ctrl+U (0x15) — 清除当前行。
    #[allow(dead_code)]
    pub fn send_ctrl_u(&self) {
        self.send_raw(&[0x15]);
    }

    /// Ctrl+L (0x0C) — 清屏。
    #[allow(dead_code)]
    pub fn send_ctrl_l(&self) {
        self.send_raw(&[0x0C]);
    }

    /// Ctrl+R (0x12) — 反向搜索。
    #[allow(dead_code)]
    pub fn send_ctrl_r(&self) {
        self.send_raw(&[0x12]);
    }

    /// F12 键序列 — 触发 TUI debug snapshot。
    #[allow(dead_code)]
    pub fn send_f12(&self) {
        self.send_raw(b"\x1b[24~");
    }

    // ── 屏幕读取 ─────────────────────────────────────────────

    /// 获取当前终端屏幕内容（通过 vt100 仿真）。
    /// 返回的是当前屏幕的可见文本，不是累积输出。
    pub fn current_screen(&self) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&buf);
        parser.screen().contents()
    }

    /// 读取指定行（0-indexed）。
    #[allow(dead_code)]
    pub fn screen_row(&self, row: u16) -> String {
        self.screen_rows()
            .get(row as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn screen_rows(&self) -> Vec<String> {
        let buf = self.buffer.lock().unwrap().clone();
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&buf);
        let screen = parser.screen();
        let mut rows = Vec::with_capacity(self.rows as usize);
        for row in 0..self.rows {
            let mut line = String::new();
            for col in 0..self.cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                let ch = cell.contents();
                if ch.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(&ch);
                }
            }
            rows.push(line.trim_end().to_string());
        }
        rows
    }

    /// 读取状态栏。
    ///
    /// vt100 的当前 screen buffer 在真实 PTY 下偶尔会把最后一行解析为空，
    /// 但状态栏仍保留在底部附近的有效行中。这里先扫描底部可见行，再从
    /// 累积纯文本中回退提取最近一次状态栏片段，避免 model_flow 测试因为
    /// harness 取错行而跳过状态栏断言。
    pub fn status_bar(&self) -> String {
        let rows = self.screen_rows();
        for line in rows.iter().rev() {
            if is_status_bar_candidate(line) {
                return line.trim().to_string();
            }
        }

        let last_row = rows.last().cloned().unwrap_or_default();
        if !last_row.trim().is_empty() {
            return last_row.trim().to_string();
        }

        extract_status_from_text(&self.current_text()).unwrap_or_default()
    }

    /// 获取已捕获的原始输出（ANSI 去除后的纯文本）。
    pub fn current_text(&self) -> String {
        let buf = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&buf);
        String::from_utf8_lossy(&plain).into_owned()
    }

    // ── 等待断言 ─────────────────────────────────────────────

    /// 等待状态栏包含指定文本。
    #[allow(dead_code)]
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

    /// 等待模型响应完成。
    ///
    /// The current footer no longer renders the old `ready | N msgs` contract.
    /// Treat a response as complete once the TUI has shown a busy state, emitted
    /// additional output after the submitted prompt, and returned to a non-busy
    /// screen.
    pub fn wait_response_done(&self, _min_msgs: usize, timeout: Duration) -> bool {
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(150));
        let baseline_len = self.current_text().len();
        let mut saw_busy = false;
        let mut saw_output_after_submit = false;

        loop {
            if start.elapsed() > timeout {
                return false;
            }

            let bar = self.status_bar();
            let screen = self.current_screen();
            let busy = response_is_busy(&bar, &screen);
            if busy {
                saw_busy = true;
            }

            if self.current_text().len() > baseline_len.saturating_add(8) {
                saw_output_after_submit = true;
            }

            if !busy
                && saw_output_after_submit
                && (saw_busy || start.elapsed() > Duration::from_secs(2))
            {
                return true;
            }

            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// 等待输出中出现指定文本（ANSI 去除后）。
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

    /// 等待屏幕中出现指定文本。
    #[allow(dead_code)]
    pub fn wait_for_screen_text(&self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return false;
            }
            if self.current_screen().contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// 等待任意一个文本出现，返回匹配的索引。
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

    // ── 截图与日志 ───────────────────────────────────────────

    /// Mid-session 截图：保存 .log / .html，返回纯文本。
    pub fn snapshot(&self, label: &str) -> String {
        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        let dir = logs_dir();
        let log_path = dir.join(format!("{label}.log"));
        std::fs::write(&log_path, &plain).expect("write log");

        let output = CapturedOutput {
            raw: raw.clone(),
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

    /// Mid-session 截图保存到指定目录。
    pub fn snapshot_to(&self, label: &str, dir: &std::path::Path) -> String {
        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        let log_path = dir.join(format!("{label}.log"));
        std::fs::write(&log_path, &plain).expect("write log");

        let output = CapturedOutput {
            raw: raw.clone(),
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

    // ── 结束会话 ─────────────────────────────────────────────

    /// 等待子进程退出，保存日志，返回捕获的输出。
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

        // 等待读取线程结束
        std::thread::sleep(Duration::from_millis(200));
        drop(self.slave.take());
        drop(self.writer);
        if let Some(h) = self.reader_thread.take() {
            let _ = h.join();
        }

        let raw = self.buffer.lock().unwrap().clone();
        let plain = strip_ansi_escapes::strip(&raw);

        // 保存日志文件（不保存 .raw，只保留 .log + .html）
        let dir = logs_dir();
        let log_path = dir.join(format!("{test_name}.log"));
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

        // 保存 HTML 终端截图
        let html_path = dir.join(format!("{test_name}.html"));
        let html = output.render_html();
        std::fs::write(&html_path, html.as_bytes()).expect("write html");

        output
    }

    /// 等待子进程退出并保存到指定目录（不保存 .raw）。
    pub fn finish_to(
        mut self,
        timeout: Duration,
        test_name: &str,
        dir: &std::path::Path,
    ) -> CapturedOutput {
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

        let log_path = dir.join(format!("{test_name}.log"));
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

        let html_path = dir.join(format!("{test_name}.html"));
        let html = output.render_html();
        std::fs::write(&html_path, html.as_bytes()).expect("write html");

        output
    }
}

// ─── CapturedOutput ──────────────────────────────────────────────────

/// 测试捕获的终端输出。
pub struct CapturedOutput {
    pub raw: Vec<u8>,
    pub plain: Vec<u8>,
    pub cols: u16,
    pub rows: u16,
}

impl CapturedOutput {
    /// 纯文本内容。
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.plain).into_owned()
    }

    /// 是否包含指定文本。
    pub fn contains(&self, needle: &str) -> bool {
        self.text().contains(needle)
    }

    /// 打印预览（最多 max_bytes 字节）。
    #[allow(dead_code)]
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

    /// 渲染为 HTML 终端截图。
    pub fn render_html(&self) -> String {
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&self.raw);

        // 如果屏幕被退出清理清空了，回退到最后一个有效帧
        if parser.screen().contents().trim().is_empty() {
            let markers: &[&[u8]] = &[b"\x1b[?1049l", b"\x1b[2J"];
            let mut best_end = self.raw.len();
            for m in markers {
                if let Some(pos) = rfind(&self.raw, m) {
                    best_end = best_end.min(pos);
                }
            }
            let home_seq = b"\x1b[H";
            let mut search_end = self.raw.len();
            while let Some(pos) = rfind(&self.raw[..search_end], home_seq) {
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
        html.push_str(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>PTY Test Screenshot</title>
<style>
body{background:#1e1e1e;margin:0;padding:16px;display:flex;justify-content:center}
.terminal{background:#0c0c0c;border:1px solid #444;border-radius:8px;padding:12px;box-shadow:0 4px 24px rgba(0,0,0,.5)}
pre{font-family:'Cascadia Code','Consolas','Courier New',monospace;font-size:14px;line-height:1.3;margin:0;color:#ccc}
.row{display:block;height:1.3em;white-space:pre}
</style></head><body><div class="terminal"><pre>
"#);

        for row in 0..self.rows {
            html.push_str("<span class=\"row\">");
            let mut col = 0u16;
            while col < self.cols {
                let cell = screen.cell(row, col).unwrap();
                let ch = cell.contents();
                let display = if ch.is_empty() { " " } else { &ch };
                html.push_str(&html_escape(display));
                let width = unicode_width::UnicodeWidthStr::width(display);
                col += if width > 1 { width as u16 } else { 1 };
            }
            html.push_str("</span>\n");
        }

        html.push_str("</pre></div></body></html>");
        html
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────

/// 从状态栏解析 "N msgs" 中的 N。
pub fn parse_msg_count(status: &str) -> Option<usize> {
    for word in status.split_whitespace() {
        if let Ok(n) = word.parse::<usize>() {
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

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_status_bar_candidate(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('>') || trimmed.contains("Message cc-rust") {
        return false;
    }

    let has_model_path = trimmed.contains(" | ");
    let has_status = trimmed.contains("ready")
        || trimmed.contains("BUSY")
        || trimmed.contains("INS")
        || trimmed.contains(" msgs")
        || trimmed.contains(" msg");
    has_model_path || has_status
}

fn response_is_busy(status_bar: &str, screen: &str) -> bool {
    status_bar.contains("BUSY")
        || status_bar.contains("tab to queue")
        || screen.contains("Thinking")
        || screen.contains("BUSY")
        || screen.contains("tab to queue")
}

fn extract_status_from_text(text: &str) -> Option<String> {
    let workspace = workspace();
    let idx = text.rfind(workspace)?;
    let prefix_start = text[..idx]
        .rfind(|ch: char| ch == '\n' || ch == '\r' || ch == '│' || ch == '┘')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    let suffix_end = text[idx..]
        .find(|ch: char| ch == '\n' || ch == '\r' || ch == '>' || ch == '│')
        .map(|offset| idx + offset)
        .unwrap_or_else(|| text.len());
    let candidate = text[prefix_start..suffix_end].trim();
    (!candidate.is_empty()).then(|| candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_candidate_accepts_model_path_line() {
        assert!(is_status_bar_candidate("gpt-5.4 | /tmp/cc-rust-e2e-test"));
    }

    #[test]
    fn status_candidate_rejects_prompt_line() {
        assert!(!is_status_bar_candidate(
            ">   Message cc-rust, / for commands  [INS]"
        ));
    }
}
