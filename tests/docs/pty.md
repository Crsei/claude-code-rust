# PTY 终端测试

使用 `portable-pty` (Windows ConPTY) 在伪终端中启动 `claude-code-rs`，捕获完整的终端渲染输出（含 ANSI 转义序列），验证 TUI 的渲染、输入和交互行为。

## 与 E2E 测试的区别

| | E2E (assert_cmd) | PTY (portable-pty) |
|---|---|---|
| 子进程 stdio | piped (管道) | 伪终端 (ConPTY) |
| `isatty()` | `false` | `true` |
| 捕获内容 | stdout/stderr 纯文本 | 完整终端渲染 (ANSI + 光标) |
| 能测 TUI 渲染 | 不能 | 能 |
| 能模拟键盘 | 仅 stdin 写入 | Ctrl+C/D/G、方向键、vim |
| 输出日志 | 无 | `.raw` + `.log` 文件 |

## 运行方式

```bash
# 所有 offline PTY 测试
cargo test --test pty_ui

# 独立版 PTY 测试
cargo test --test e2e_pty

# 单个模块
cargo test --test pty_ui welcome
cargo test --test pty_ui input
cargo test --test pty_ui resize

# 附带终端输出
cargo test --test pty_ui -- --nocapture

# live 测试 (需要 API key)
cargo test --test pty_ui -- --ignored --nocapture

# 运行演示
cargo run --example pty_demo
```

## 日志输出

每次运行在 `logs/YYYYMMDDHHMM/` 目录下生成：

- `{test_name}.raw` — 原始字节，含 ANSI 转义序列，可用 `xxd` 查看
- `{test_name}.log` — strip ANSI 后的纯文本

同一次测试运行共享同一个时间戳目录（`OnceLock` 保证只初始化一次）。

```
logs/
├── .gitkeep
├── 202604082057/
│   ├── fp_version.raw
│   ├── fp_version.log
│   ├── welcome_logo.raw
│   ├── welcome_logo.log
│   ├── input_vim_toggle.raw
│   └── ...
└── 202604082115/
    └── ...
```

## 依赖

```toml
[dev-dependencies]
portable-pty = "0.9"    # ConPTY 伪终端
# strip-ansi-escapes 已在 [dependencies] 中
```

---

## 架构：PtySession

```
┌─────────────────────────────────────────────────────────┐
│  cargo test                                             │
│                                                         │
│  ┌──────────────┐     ConPTY (Windows 伪终端)           │
│  │  PtySession   │                                      │
│  │               │    ┌─────────┐    ┌────────────────┐ │
│  │  writer ──────────►│  slave  │───►│ claude-code-rs │ │
│  │  (Arc<Mutex>) │    │  (输入) │    │   子进程        │ │
│  │               │    └─────────┘    │                │ │
│  │               │    ┌─────────┐    │  stdout/stderr │ │
│  │  reader ◄──────────│ master  │◄───│  (终端渲染)    │ │
│  │  (后台线程)   │    │  (输出) │    └────────────────┘ │
│  │     │         │    └─────────┘                       │
│  │     ▼         │                                      │
│  │  buffer ──────────► logs/*.raw + logs/*.log          │
│  └──────────────┘                                       │
└─────────────────────────────────────────────────────────┘
```

### 关键机制

**DSR 自动响应**: crossterm 启动时发送 `\x1b[6n` 查询光标位置并阻塞等待。reader 线程检测后自动回复 `\x1b[1;1R`，否则子进程会永远卡住。

**slave handle 保留**: spawn 后不立即 drop slave。在 Windows ConPTY 上，过早 drop 会导致快速退出的进程丢失缓冲区数据。slave 在 `finish()` 中子进程退出后才 drop。

**光标渲染空格**: TUI 光标渲染在 ANSI strip 后会在字符间产生空格（如 `"hel lo w orl d"`），文本匹配需使用短片段。

### harness API

```rust
// 创建会话
PtySession::spawn(args, cols, rows, strip_keys) -> PtySession

// 模拟输入
session.send_raw(bytes)      // 原始字节
session.send_line("text")    // 文字 + Enter
session.send_ctrl_c()        // Ctrl+C (0x03)
session.send_ctrl_d()        // Ctrl+D (0x04)
session.send_ctrl_g()        // Ctrl+G (0x07, vim toggle)
session.send_up()            // ↑ (ESC [ A)
session.send_down()          // ↓ (ESC [ B)
session.send_escape()        // ESC (0x1b)

// 输出检测
session.current_text() -> String          // 当前纯文本快照
session.wait_for_text(needle, timeout)    // 等待文本出现
session.wait_for_any(needles, timeout)    // 等待任一文本

// 结束
session.finish(timeout, test_name) -> CapturedOutput
output.text()        // 纯文本
output.contains(s)   // 文本匹配
output.preview(n)    // 打印前 n 字节预览
```

---

## e2e_pty.rs — 独立版 PTY 测试 (6 tests)

最初的 PTY 测试文件，包含内联的 `PtySession`。`pty_ui/` 是其模块化重构版。

| 测试 | 说明 |
|------|------|
| `pty_version_flag` | `-V` 版本输出捕获 |
| `pty_init_only` | `--init-only` 初始化退出 |
| `pty_dump_system_prompt` | 系统提示词完整捕获 (22KB+) |
| `pty_tui_starts_and_captures_output` | TUI 启动渲染捕获 |
| `live_pty_simple_chat` | (live) TUI 中发送 prompt 并验证响应 |
| `live_pty_print_mode` | (live) `-p` 模式输出捕获 |

---

## pty_ui/ — 模块化 PTY UI 测试 (29 tests)

按 UI 能力分模块组织，共享 `harness.rs` 基础设施。

### 目录结构

```
tests/pty_ui/
├── main.rs         模块声明 + 运行说明
├── harness.rs      PtySession 共享基础设施 (无测试)
├── fast_path.rs    快路径: 立即退出的命令
├── welcome.rs      欢迎页: logo、header、tips
├── input.rs        输入框: 键盘、光标、vim
├── streaming.rs    流式响应: 生命周期、中断
└── resize.rs       终端尺寸: 边界条件
```

---

### fast_path.rs — 快路径 (6 tests)

命令执行后立即退出，不进入 TUI。验证快路径输出在真实终端中正确渲染。

| 测试 | Offline | 说明 |
|------|---------|------|
| `version_flag` | 是 | `-V` 输出 "claude-code-rs" |
| `init_only` | 是 | `--init-only` 退出，验证日志文件生成 |
| `dump_system_prompt` | 是 | `--dump-system-prompt` 输出包含 "tool" (22KB+) |
| `dump_custom_system_prompt` | 是 | `--system-prompt "..."` 自定义内容出现在输出中 |
| `print_mode_no_api_key` | 是 | `-p` 无 key 报 API 错误 |
| `print_mode_live` | 否 | `-p` 有 key 输出正确响应 |

---

### welcome.rs — 欢迎页 (5 tests)

验证 TUI 启动后的初始渲染内容。

| 测试 | 说明 |
|------|------|
| `shows_header_with_version` | header 包含 "Claude Code" 或 "cc-rust" |
| `shows_ascii_logo` | ASCII art logo 渲染（>500 bytes raw 输出） |
| `shows_tips` | 显示 "Tips" / "Enter" / "Ctrl" 等使用提示 |
| `renders_at_small_terminal` | 60x20 小终端正常渲染 |
| `renders_at_wide_terminal` | 200x50 宽终端正常渲染 |

---

### input.rs — 输入框 (8 tests)

验证键盘输入处理和编辑操作。全部 offline。

| 测试 | 说明 |
|------|------|
| `typed_text_appears` | 输入的文字片段出现在终端输出中 |
| `ctrl_d_exits` | Ctrl+D 正常退出，无 panic |
| `ctrl_u_clears_line` | Ctrl+U 清除输入行，无 panic |
| `arrow_keys_on_empty_input` | 空输入时 ↑↓←→ 不崩溃 |
| `backspace_deletes` | 退格键删除字符，无 panic |
| `vim_toggle` | Ctrl+G 切换 vim 模式，执行 vim 命令，无 panic |
| `submit_empty_prompt` | 空 prompt 按 Enter 不崩溃 |
| `rapid_typing` | 单次写入长文本不丢字符、不崩溃 |

> **注意**: TUI 光标渲染在 ANSI strip 后会在字符间插入空格（如 `"hel lo w orl d"`），因此文本匹配使用短片段（如 `"hel"`）而非完整字符串。

---

### streaming.rs — 流式响应 (6 tests)

验证流式输出的显示和交互。5 个需要 API key（`#[ignore]`），1 个 offline。

| 测试 | Offline | 说明 |
|------|---------|------|
| `submit_prompt_no_api_key_shows_error` | 是 | 无 key 提交 prompt 显示错误，不 panic |
| `simple_chat_renders_response` | 否 | 响应文本出现在 TUI 渲染中 |
| `abort_during_streaming` | 否 | Ctrl+C 中断后可继续发送新 prompt |
| `multi_turn_conversation` | 否 | 两轮连续对话，各自响应正确 |
| `tool_use_displayed` | 否 | 工具名称 / 输出在 TUI 中可见 |

---

### resize.rs — 终端尺寸 (5 tests)

验证不同终端尺寸下 TUI 不崩溃、能产生输出。全部 offline。

| 测试 | 终端大小 | 说明 |
|------|----------|------|
| `minimum_size` | 40x10 | 最小尺寸不 panic |
| `very_wide` | 300x40 | 超宽不 panic |
| `very_tall` | 80x100 | 超高不 panic |
| `typing_at_narrow_width` | 40x20 | 输入超过终端宽度的长文本不 panic |
| `standard_80x24` | 80x24 | 标准尺寸渲染正常（>100 bytes 输出） |

---

## examples/pty_demo.rs — PTY 演示

不依赖 `claude-code-rs`，用系统命令演示 PTY 的 3 个核心能力：

```bash
cargo run --example pty_demo
```

| Demo | 展示能力 | 使用的 API |
|------|----------|-----------|
| 1. 捕获输出 | `cmd /c echo` 的 raw vs plain 差异 | `spawn()` → `finish()` |
| 2. 交互输入 | 启动 `cmd`，发送命令，再 `exit` | `send_line()` |
| 3. 实时等待 | 异步延迟输出，实时检测文本出现 | `wait_for_text()` |

---

## 踩过的坑

### 1. `take_writer()` 只能调用一次

`MasterPty::take_writer()` 消费 writer handle，第二次调用 panic。

**解法**: spawn 时取出 writer，包装为 `Arc<Mutex<Box<dyn Write + Send>>>`，reader 线程和主线程共享。

### 2. 子进程快速退出时丢失输出

`--version` 这样的快路径命令瞬间退出，如果 spawn 后立刻 `drop(slave)`，ConPTY 管道在 reader 读到数据之前就关闭了。

**解法**: 保留 slave handle，在 `finish()` 中等子进程退出 + sleep 200ms 后再 drop。

### 3. Unicode 边界 panic

`output.text()[..500]` 如果 500 恰好落在多字节字符中间（如 `█` 占 3 bytes），会 panic。

**解法**: 用 `char_indices()` 找安全的截断位置。

### 4. crossterm DSR 阻塞

crossterm 检测到真实终端后发送 `\x1b[6n` 并阻塞。普通 piped 测试不触发（`isatty()=false`），但 PTY 中必须自动响应。

**解法**: reader 线程检测 `\x1b[6n` 后写回 `\x1b[1;1R`。

### 5. TUI 光标渲染产生空格

TUI 的光标移动指令在 strip ANSI 后变成空格字符，导致 `"hello world"` 变成 `"hel lo w orl d"`。

**解法**: 文本断言使用 3 字符短片段（如 `"hel"`），或只断言"不 panic"。
