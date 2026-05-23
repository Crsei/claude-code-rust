# PTY TUI E2E 测试问题记录

> 更新日期: 2026-05-23

## 已知问题

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| TP-001 | 高 | Open | LoginSwitch | `/login claude_code` 命令会打开登录方法选择器（[1] API Key, [2] Claude.ai OAuth, [3] Console OAuth），而不是直接切换 authProfile。测试中发送的 `/login claude_code` 字符串被 TUI 的命令面板拦截，`claude_code` 被解释为搜索过滤文本而非命令参数。 | 改用 `CC_RUST_HOME` + 临时 settings.json（`create_temp_profile_home("claude_code")`）可绕过此问题，已在 `model_flow.rs` 中验证。 |
| TP-002 | 高 | Open | SetPermission | `/permissions full access` 不是有效命令。TUI 返回 `Unknown permissions subcommand: 'full'`。正确语法：`/permissions mode <m>` 其中 m 可选 `default\|auto\|bypass\|plan\|acceptEdits\|dontAsk`。 | 修复方法：将 `SetPermission("full access")` 改为 `Command("permissions mode bypass")` 或使用正确的 `/permissions allow Bash` 语法。 |
| TP-003 | 中 | Open | API 超时 | 使用 `codex` profile（openai-codex 提供者，后端 URL: `https://chatgpt.com/backend-api/codex/responses`）时，API 请求频繁超时：`error=TimedOut`。这导致模型无法响应，TUI 卡在 "Thinking..." 状态。 | 改用 `claude_code` profile（anthropic 提供者，`https://inferaichat.com`）并使用 `CC_RUST_HOME` 指向临时 settings。 |

## 修复方案

### TP-001: LoginSwitch 替代方案

使用 `model_flow.rs` 中的 `create_temp_profile_home("claude_code")` 创建临时 settings.json，并通过 `CC_RUST_HOME` 环境变量传递给 TUI 进程：

```rust
let tmp = create_temp_profile_home("claude_code");
let tmp_home = tmp.path().join(".cc-rust");
let case = TestCase::new("test")
    .env("CC_RUST_HOME", tmp_home.to_str().unwrap());
```

### TP-002: SetPermission 替代方案

```rust
// 错误（无效命令）
TestStep::SetPermission("full access".into())

// 正确（设置 bypass 模式）
TestStep::Command("permissions mode bypass".into())

// 或使用 allow 规则
TestStep::Command("permissions allow Bash".into())
```

### TP-003: API 超时缓解

- 优先使用 `claude_code` profile（拥有可用的 anthropic 后端）
- 如果需要测试 `codex` profile，考虑增加 API 超时时间
- 使用 `TestRunner` 时可通过 `TestCase::timeout()` 增加全局超时

## 更新规则

1. 问题修复后移入 `docs/archive/`。
2. 使用 `logs/pty_tui_e2e_{timestamp}/{test_name}/` 目录中的截图和日志进行问题复现/验证。
