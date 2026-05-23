//! PTY TUI E2E 测试套件 — 真实终端交互测试
//!
//! 完整测试 cc-rust TUI 的端到端行为：启动、权限、对话、状态。
//!
//! ## 架构
//!
//! 通过 `portable-pty` 创建真实伪终端，启动 `claude-code-rs` 二进制，
//! 模拟键盘输入并用 `vt100` 解析器读取屏幕状态。
//!
//! ```text
//! ┌──────────┐    PTY    ┌──────────────┐    screen    ┌───────────┐
//! │ 测试代码  │ ────────→ │ claude-code-rs│ ──────────→ │ vt100 解析│
//! │ (Rust)   │ ←──────── │ (TUI 二进制)  │ ←────────── │ (断言)    │
//! └──────────┘  stdin    └──────────────┘  stdout      └───────────┘
//! ```
//!
//! ## 模块说明
//!
//! | 模块           | 测试内容                              |
//! |---------------|--------------------------------------|
//! | `harness`     | PTY 会话封装（非测试，工具模块）        |
//! | `welcome`     | 欢迎屏幕：Logo、模型名、会话 ID        |
//! | `conversation`| 完整对话流：输入 → 响应 → 多轮 → 工具  |
//! | `permissions` | 权限对话框：触发、批准、拒绝、模式切换  |
//! | `commands`    | 斜杠命令：/help /version /clear       |
//! | `status`      | 状态栏：消息计数、模型名、运行状况      |
//! | `screenshot`  | 截图保存：HTML 渲染、mid-session 快照  |
//! | `script`      | 步骤式模板引擎：数据驱动的测试脚本      |
//!
//! ## 运行
//!
//! ```bash
//! # 离线测试（不需要 API key，测试 UI 渲染和交互）
//! cargo test --test pty_tui_e2e -- --nocapture
//!
//! # 在线测试（需要真实 API key，测试完整对话流程）
//! cargo test --test pty_tui_e2e -- --ignored --nocapture
//!
//! # 单个模块
//! cargo test --test pty_tui_e2e welcome -- --nocapture
//! cargo test --test pty_tui_e2e conversation -- --nocapture
//! cargo test --test pty_tui_e2e permissions -- --nocapture
//! ```

mod harness;

mod commands;
mod conversation;
mod model_flow;
mod permissions;
mod screenshot;
mod script;
mod status;
mod welcome;
