# PTY TUI E2E 测试套件

通过真实伪终端 (PTY) 端到端测试 cc-rust TUI 的完整行为。

## 架构

```text
┌──────────┐    PTY    ┌──────────────┐    screen    ┌───────────┐
│ 测试代码  │ ────────→ │ claude-code-rs│ ──────────→ │ vt100 解析│
│ (Rust)   │ ←──────── │ (TUI 二进制)  │ ←────────── │ (断言)    │
└──────────┘  stdin    └──────────────┘  stdout      └───────────┘
```

- `portable-pty` 创建真实伪终端，启动 `claude-code-rs` 二进制
- 模拟键盘输入（文本、快捷键、斜杠命令）
- `vt100` 解析器读取终端屏幕内容，用于断言和截图
- 自动回复 crossterm DSR 查询 (`\x1b[6n`)，防止进程阻塞

## 文件说明

| 文件 | 说明 |
|------|------|
| `main.rs` | 测试入口，声明所有模块 |
| `harness.rs` | PTY 会话封装：启动、输入、屏幕读取、等待、截图 |
| `script.rs` | **步骤式模板引擎**：用数据结构定义测试，自动执行和截图 |
| `welcome.rs` | 欢迎屏幕：Logo、模型名、会话 ID、终端尺寸 |
| `commands.rs` | 斜杠命令：/help /version /cost /status、命令面板 |
| `conversation.rs` | 完整对话流：单轮、多轮上下文、工具调用、Ctrl+C 中断 |
| `model_flow.rs` | 模型验证：settings.json 配置、/model 切换、authProfile 切换 |
| `permissions.rs` | 权限系统：bypass/default/auto 模式、权限对话框渲染 |
| `status.rs` | 状态栏：ready 状态、消息计数、操作后状态恢复 |
| `screenshot.rs` | 截图功能：HTML 渲染、mid-session 快照、多格式保存 |

## 运行

```bash
# 设置 Rust 工具链路径
export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
export PATH="$CARGO_HOME/bin:$PATH"

# 运行所有离线测试（不需要 API key）
cargo test -p claude-code-rs --test pty_tui_e2e -- --nocapture

# 运行所有在线测试（需要真实 API key，从 ~/.cc-rust/settings.json 读取）
cargo test -p claude-code-rs --test pty_tui_e2e -- --ignored --nocapture

# 运行单个模块
cargo test -p claude-code-rs --test pty_tui_e2e welcome -- --nocapture
cargo test -p claude-code-rs --test pty_tui_e2e commands -- --nocapture

# 运行模板引擎测试
cargo test -p claude-code-rs --test pty_tui_e2e -- script --nocapture

# 运行指定的模板测试
cargo test -p claude-code-rs --test pty_tui_e2e -- script_command_palette --nocapture
cargo test -p claude-code-rs --test pty_tui_e2e -- --ignored script_conversation_verify --nocapture
```

## 模板引擎 (`script.rs`)

### 核心概念

```text
TestCase（测试脚本）
  └─ Vec<TestStep>（步骤序列）
       ├─ SkipTrustGate          // 跳过首次信任确认
       ├─ Input("text")          // 输入文本 + Enter
       ├─ Command("model gpt-5.4") // 斜杠命令（自动加 "/"）
       ├─ Key(TestKey::CtrlC)    // 快捷键
       ├─ Snapshot("label")      // 截图保存
       ├─ WaitForText("x", 30s)  // 等待文本出现
       ├─ AssertStatusBar("gpt") // 断言状态栏
       ├─ SetPermission("full access") // 设置权限
       └─ LoginSwitch("profile") // 切换 authProfile
```

### TestStep 完整列表

| 步骤 | 说明 |
|------|------|
| `Wait(Duration)` | 等待固定时长 |
| `Input(String)` | 输入文本并按 Enter |
| `TypeText(String)` | 只输入文本，不按 Enter |
| `Command(String)` | 斜杠命令（自动加 "/" 前缀 + Enter） |
| `Key(TestKey)` | 发送快捷键 |
| `Snapshot(String)` | 手动截图（保存 .html + .log） |
| `OpenPalette` | 打开命令面板（发送 "/"，等待 "Commands" 出现） |
| `PaletteSelect(usize)` | 命令面板选择第 N 项（Down N-1 次 + Enter） |
| `ClosePalette` | 关闭命令面板（Esc） |
| `AssertScreenContains(String)` | 断言屏幕包含指定文本 |
| `AssertTextContains(String)` | 断言纯文本输出包含指定文本 |
| `AssertStatusBar(String)` | 断言状态栏包含指定文本 |
| `AssertScreenNotContains(String)` | 断言屏幕不包含指定文本 |
| `WaitForText(String, Duration)` | 等待文本出现在输出中 |
| `WaitForAny(Vec<String>, Duration)` | 等待任意一个文本出现 |
| `WaitForStatus(String, Duration)` | 等待状态栏包含指定文本 |
| `MultilineInput(Vec<String>)` | 多行输入（每行依次发送） |
| `SkipTrustGate` | 跳过 workspace trust gate |
| `SetPermission(String)` | 通过 /permissions 设置权限 |
| `LoginSwitch(String)` | 通过 /login 切换 authProfile |
| `AssertNoPanic` | 断言输出中无 "panicked" |

### TestKey 快捷键

| 快捷键 | 说明 |
|--------|------|
| `CtrlC` | 中断（ETX 0x03） |
| `CtrlD` | 退出（EOT 0x04） |
| `CtrlU` | 清除当前行 |
| `CtrlL` | 清屏 |
| `CtrlR` | 反向搜索 |
| `F12` | TUI debug snapshot |
| `Enter` | 回车 |
| `Escape` | Esc |
| `Tab` | Tab |
| `Up` / `Down` | 方向键 |

### 用法示例

#### 1. 基础对话验证

```rust
use crate::harness::*;
use crate::script::*;
use std::time::Duration;

#[test]
#[ignore = "requires real API key"]
fn verify_model_identity() {
    let settings = read_settings();
    let case = TestCase::new(format!("conv_{}", settings.active_auth_profile))
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::Snapshot("initial".into()))
        .step(TestStep::AssertStatusBar(settings.expected_model.clone()))
        .step(TestStep::Input("Answer with ONLY your model ID.".into()))
        .step(TestStep::WaitForAny(
            vec![settings.expected_model.clone()],
            API_TIMEOUT,
        ))
        .step(TestStep::Snapshot("model_reply".into()));

    TestRunner::new().run(&case).assert_no_errors();
}
```

#### 2. 切换 authProfile

```rust
#[test]
#[ignore = "requires real API key"]
fn switch_profile() {
    let case = TestCase::new("switch_to_claude_code")
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::LoginSwitch("claude_code".into()))
        .step(TestStep::Wait(Duration::from_secs(3)))
        .step(TestStep::AssertStatusBar("deepseek-v4-pro".into()))
        .step(TestStep::Snapshot("after_switch".into()));

    TestRunner::new().run(&case).assert_no_errors();
}
```

#### 3. 权限设置 + 工具执行

```rust
#[test]
#[ignore = "requires real API key"]
fn permissions_and_tool_use() {
    let case = TestCase::new("perm_tool")
        .step(TestStep::SkipTrustGate)
        .step(TestStep::SetPermission("full access".into()))
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::Input("Use Bash to run: echo OK".into()))
        .step(TestStep::WaitForText("OK".into(), API_TIMEOUT))
        .step(TestStep::Snapshot("tool_done".into()));

    TestRunner::new().run(&case).assert_no_errors();
}
```

#### 4. Ctrl+C 中断 + 恢复

```rust
#[test]
#[ignore = "requires real API key"]
fn abort_and_recover() {
    let case = TestCase::new("abort_recover")
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Input("Write a 2000-word essay.".into()))
        .step(TestStep::Wait(Duration::from_secs(3)))
        .step(TestStep::Key(TestKey::CtrlC))
        .step(TestStep::Snapshot("after_abort".into()))
        .step(TestStep::Input("Say exactly: RECOVERED".into()))
        .step(TestStep::WaitForText("RECOVERED".into(), API_TIMEOUT))
        .step(TestStep::Snapshot("recovered".into()));

    TestRunner::new().run(&case).assert_no_errors();
}
```

#### 5. 命令面板操作

```rust
#[test]
fn command_palette_flow() {
    let case = TestCase::new("palette_flow")
        .step(TestStep::SkipTrustGate)
        .step(TestStep::OpenPalette)
        .step(TestStep::AssertScreenContains("Commands".into()))
        .step(TestStep::Snapshot("palette_open".into()))
        .step(TestStep::ClosePalette);

    TestRunner::new().run(&case).assert_no_errors();
}
```

#### 6. 链式 Builder 语法

```rust
let case = TestCase::new("my_test")
    .cols(120)
    .rows(40)
    .env("CC_RUST_HOME", "/tmp/test-home")
    .exit(ExitMethod::CtrlC)
    .timeout(Duration::from_secs(180))
    .step(TestStep::SkipTrustGate)
    .step(TestStep::Input("hello".into()));
```

## Harness API (`harness.rs`)

### PtySession

```rust
// 启动
let session = PtySession::spawn(&args, cols, rows, strip_keys);
let session = PtySession::spawn_with_env(&args, cols, rows, strip_keys, &envs);

// 输入
session.send_line("text");          // 文本 + Enter
session.send_raw(b"\x03");          // 原始字节
session.send_ctrl_c();              // Ctrl+C
session.send_ctrl_d();              // Ctrl+D
session.send_ctrl_u();              // Ctrl+U (清除行)
session.send_ctrl_l();              // Ctrl+L (清屏)
session.send_ctrl_r();              // Ctrl+R (搜索)
session.send_f12();                 // F12 (debug snapshot)
session.send_up() / send_down();    // 方向键
session.send_escape();              // Esc
session.send_tab();                 // Tab

// 屏幕读取
let screen = session.current_screen();   // 当前可见屏幕文本
let text = session.current_text();       // 累积纯文本（ANSI 去除）
let bar = session.status_bar();          // 状态栏内容
let row = session.screen_row(0);         // 指定行

// 等待
session.wait_for_text("needle", timeout);      // 等待文本出现
session.wait_for_any(&["a", "b"], timeout);    // 等待任一文本
session.wait_for_screen_text("x", timeout);    // 等待屏幕文本
session.wait_status("ready", timeout);          // 等待状态栏
session.wait_response_done(min_msgs, timeout); // 等待响应完成

// 截图
session.snapshot("label");                          // 保存到 logs_dir()
session.snapshot_to("label", &dir);                 // 保存到指定目录
session.finish(timeout, "test_name");               // 退出 + 保存到 logs_dir()
session.finish_to(timeout, "test_name", &dir);      // 退出 + 保存到指定目录
```

### 辅助函数

```rust
workspace()          // 测试工作区路径（E2E_WORKSPACE 或 /tmp/cc-rust-e2e-test）
logs_dir()           // 日志根目录（logs/pty_tui_e2e_{timestamp}/）
test_subdir("name")  // 测试专属子目录（logs/pty_tui_e2e_{timestamp}/{name}/）
binary_path()        // claude-code-rs 二进制路径
default_args()       // 标准启动参数：-C {workspace} --permission-mode bypass
read_settings()      // 读取 ~/.cc-rust/settings.json 的 activeAuthProfile 和 model
skip_trust_gate()    // 跳过首次 workspace 信任确认
```

## 输出结构

```
crates/claude-code-rs/logs/pty_tui_e2e_{YYYYMMDDHHMM}/
├── {test_name}/                    ← 每个测试独立文件夹
│   ├── step_001_skip_trust.html    ← 步骤截图（HTML 终端渲染）
│   ├── step_001_skip_trust.log     ← 步骤纯文本
│   ├── step_003_input_question.html
│   ├── step_003_input_question.log
│   ├── ...
│   ├── session_full.html           ← 完整会话 HTML 截图
│   ├── session_full.log            ← 完整会话纯文本
│   └── errors.txt                  ← 错误汇总（仅在有错误时生成）
├── {test_name_2}/
│   └── ...
```

HTML 文件可在浏览器中打开查看终端截图，带暗色终端样式。

## 测试分类

### 离线测试（不需要 API key）

测试 UI 渲染和基本交互，使用 `--permission-mode bypass` 和空 API key：

- `welcome::*` — 欢迎屏幕、模型名、终端尺寸
- `commands::*` — 斜杠命令、命令面板
- `status::*` — 状态栏渲染
- `screenshot::*` — 截图功能
- `permissions::no_api_key_shows_error` — 无 API key 错误提示
- `permissions::all_permission_modes_start_cleanly` — 权限模式启动
- `permissions::permission_dialog_renders_in_screen_area` — 权限对话框渲染
- `script::tests::script_command_palette` — 模板引擎命令面板

### 在线测试（需要真实 API key）

从 `~/.cc-rust/settings.json` 读取 authProfile 配置，标记为 `#[ignore]`：

- `conversation::*` — 单轮/多轮对话、工具调用、中断恢复
- `model_flow::*` — 模型验证、/model 切换、authProfile 切换
- `permissions::bypass_mode_executes_without_dialog` — bypass 模式工具执行
- `permissions::default_mode_denies_tool` — default 模式拒绝工具
- `script::tests::script_conversation_verify` — 模板对话验证
- `script::tests::script_switch_auth_profile` — 模板 authProfile 切换
- `script::tests::script_set_permissions` — 模板权限设置
- `script::tests::script_abort_and_recover` — 模板中断恢复
- `script::tests::script_model_switch` — 模板模型切换

## 添加新测试

### 方式 1：使用模板引擎（推荐）

在 `script.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
#[ignore = "requires real API key"]
fn script_my_new_test() {
    let case = TestCase::new("my_new_test")
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::Input("your prompt here".into()))
        .step(TestStep::WaitForText("expected".into(), API_TIMEOUT))
        .step(TestStep::Snapshot("result".into()));

    TestRunner::new().run(&case).assert_no_errors();
}
```

### 方式 2：直接使用 Harness

创建新文件 `my_tests.rs`，在 `main.rs` 中添加 `mod my_tests;`：

```rust
use crate::harness::*;
use std::time::Duration;

#[test]
fn my_test() {
    let session = PtySession::spawn(&default_args(), 120, 40, true);
    std::thread::sleep(RENDER_WAIT);
    skip_trust_gate(&session);

    session.send_line("/help");
    std::thread::sleep(Duration::from_secs(2));

    let screen = session.current_screen();
    session.send_ctrl_d();
    let output = session.finish(QUICK_TIMEOUT, "my_test");

    assert!(!output.contains("panicked"));
    assert!(screen.contains("help"));
}
```

## 依赖

```
portable-pty    # 伪终端创建
vt100           # 终端屏幕解析
strip-ansi-escapes  # ANSI 转义序列去除
chrono          # 时间戳（日志目录命名）
tempfile        # 临时目录（测试隔离）
assert_cmd      # cargo_bin() 二进制路径解析
which           # PATH 中查找二进制
dirs            # home 目录解析
serde_json      # settings.json 解析
unicode-width   # Unicode 字符宽度（HTML 渲染）
```
