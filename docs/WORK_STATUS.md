# cc-rust 工作状态总览

> 更新日期: 2026-04-11 | 分支: `rust-lite`
>
> 本文档合并了原 `UNIMPLEMENTED_CHECKLIST.md`、`sdk-work-tracker.md`、`unfinished-features.md`
> 三份文档，作为唯一的状态跟踪入口。

---

## 1. 未完成功能 (需实现)

### 1.1 API 提供商

| 提供商 | 文件 | 状态 | 说明 |
|--------|------|------|------|
| AWS Bedrock | `src/api/client.rs:151` | **未实现** | `unimplemented!()`，调用会 panic |
| GCP Vertex AI | `src/api/client.rs:154` | **未实现** | `unimplemented!()`，调用会 panic |

已完成: Anthropic 直连, OpenAI 兼容, Google Gemini, Azure

### 1.2 认证 ✅ 全部完成

~~OAuth 登录/刷新/登出~~ — **2026-04-11 已实现**

已完成: API Key 环境变量, 系统 Keychain, OAuth PKCE (Claude.ai + Console), Token 持久化 + 自动刷新

### 1.3 Agent Teams / Coordinator Mode

整个模块 feature-gated (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`)，所有文件 `#![allow(unused)]`。

| 子模块 | 文件 | 状态 |
|--------|------|------|
| Runner 协议处理 | `src/teams/runner.rs` | **Stub** — plan approval / permission 仅 log，无真实阻塞 |
| 终端后端 | `src/teams/backend.rs` | **仅 Trait** — Tmux / iTerm2 未实现，仅 in-process 可用 |
| 前端 Dashboard | — | **不存在** — 无 Team 管理 UI 组件 |
| 斜杠命令 | — | **不存在** — 无 `/team` 类命令 |

已完成 (8/11): types, protocol, mailbox, context, identity, in_process, helpers, constants

### 1.4 工具

| 工具 | 文件 | 状态 |
|------|------|------|
| Agent 后台模式 | `src/tools/agent.rs:652-658` | **Stub** — `run_in_background` 仅 log 后同步运行 |

其余 27 个工具均已完整实现。

### 1.5 权限系统

| 阶段 | 文件 | 状态 |
|------|------|------|
| Phase 2 (Hook 拦截) | `src/permissions/decision.rs:269` | **Stub** — 跳过 hook 层直接 fall through |

已完成: Phase 1a/1b 规则匹配, Phase 3 模式检查

### 1.6 IPC

| 功能 | 文件 | 状态 |
|------|------|------|
| clear_messages | `src/ipc/headless.rs:234` | **TODO** — engine 无此方法，仅通知前端 |

### 1.7 前端 (ink-terminal)

| 功能 | 文件 | 状态 |
|------|------|------|
| Vim 状态机 | `ui/src/vim/state-machine.ts` | **部分** — 模式切换可用，按键处理器不完整 |
| 终端 resize 回流 | — | **Open** — KNOWN_ISSUES #1 |
| 窄终端布局降级 | — | **Open** — KNOWN_ISSUES #4, #5 |

14 个核心组件均已完成。

---

## 2. SDK 对标路线图

> 对标: OpenAI Codex SDK (`docs/reference/Codex_SDK_Features.md`)

### P0 — 安全加固 ✅ 全部完成

| # | 功能 | 完成日期 |
|---|------|----------|
| P0-1 | 危险命令拦截 (Stage 3c.2) | 2026-04-10 |
| P0-2 | 路径边界检查 (Stage 3c.3) | 2026-04-10 |
| P0-3 | Plan 模式写入拦截 (Stage 3c.1) | 2026-04-10 |

### P1 — 实用性

| # | 功能 | 状态 |
|---|------|------|
| P1-1 | Git 上下文注入 system prompt | ✅ |
| P1-2 | `--ephemeral` 临时会话 | ❌ |
| P1-3 | Web 搜索缓存层 | ❌ |
| P1-4 | LSP 9/9 方法实现 | ✅ |

### P2 — 生态扩展

| # | 功能 | 状态 |
|---|------|------|
| P2-1 | MCP 服务器模式 (暴露工具给外部客户端) | ❌ |
| P2-2 | JSON-RPC v2 App-Server (IDE 集成) | ❌ |
| P2-3 | OS 级沙盒 (Windows Restricted Token) | ❌ |
| P2-4 | 网络访问控制 (`--no-network` / 白名单) | ❌ |
| P2-5 | 沙盒模式 (read-only / workspace / full) | ❌ |

### P3 — 功能完善

| # | 功能 | 状态 |
|---|------|------|
| P3-1 | 会话回滚 / 快照 | ❌ |
| P3-2 | Tree-sitter AST 感知编辑 | ❌ |
| P3-3 | API 级 JSON Schema 约束输出 | ❌ |
| P3-4 | 配置 Schema 自动生成 | ❌ |
| P3-5 | Web 搜索 live/cached 切换 | ❌ |

---

## 3. 显式延期 (Deferred)

以下功能在设计上明确不在 `rust-lite` 范围内。

### 延期命令

`/remote-control`, `/web-setup`, `/chrome`, `/desktop`, `/mobile`,
`/remote-env`, `/release-notes`, `/stickers`, `/terminal-setup`, `/usage`

### 延期工具

`RemoteTrigger`, `CronCreate/Delete/List`, `WebBrowser`, `McpAuthTool`,
`Monitor`, `ListPeers`, `Workflow`, `TerminalCapture`, `SubscribePR`,
`PushNotification`, `SendUserFile`, `SuggestBackgroundPR`

### 延期模块

| 模块 | 说明 |
|------|------|
| `bridge/` | 远程控制桥接 |
| `cli/transports/` | SSE, WebSocket, Worker 传输 |
| `server/` | 服务器模式 |
| `remote/` | 云容器 (CCR) |
| `services/remoteManagedSettings/` | MDM + 远程设置同步 |
| `services/analytics/` | 遥测管道 |
| Desktop / Mobile 集成 | — |

### 内部 / Ant-Only 命令 (不实现)

`/agents-platform`, `/ant-trace`, `/autofix-pr`, `/backfill-sessions`,
`/break-cache`, `/bridge-kick`, `/bughunter`, `/ctx-viz`,
`/debug-tool-call`, `/env`, `/good-claude`, `/init-verifiers`,
`/issue`, `/mock-limits`, `/oauth-refresh`, `/onboarding`,
`/perf-issue`, `/reset-limits`, `/share`, `/summary`,
`/teleport`, `/heapdump`

### 内部工具 (不实现)

`CtxInspect`, `OverflowTest`, `VerifyPlanExecution`, `Tungsten`

---

## 4. 已完成基线

> 详细清单见 `COMPLETED_FULL.md` 和 `COMPLETED_SIMPLIFIED.md`。

- **斜杠命令**: 75/75 (含 `/login-code`, `/extra-usage`, `/rate-limit-options`)
- **工具**: 28 个 (Bash, Read, Write, Edit, Grep, Glob, Agent, Skill, LSP, Tasks, Web, PowerShell...)
- **API 提供商**: 4/6 (Anthropic, OpenAI, Google, Azure)
- **认证**: API Key + Keychain + OAuth PKCE (Claude.ai Bearer / Console API Key)
- **核心模块**: engine, query, compact, session, permissions, config, ipc, skills, plugins, mcp, lsp_service, ui
- **前端组件**: 14/14 (App, Header, MessageList, InputPrompt, MessageBubble, ToolUse/Result, PermissionDialog, Suggestions, WelcomeScreen, StatusBar, Spinner, ThinkingBlock, DiffView)

---

## 5. 完成度总览

```
  API 提供商    ████████████░░░░  4/6 (67%)
  认证          ████████████████  4/4 (100%) ✅
  Teams 系统    ████████████░░░░  8/11 模块 (73%) — feature-gated
  工具          ████████████████  27/28 (96%)
  权限          ████████████░░░░  2/3 phases (67%)
  斜杠命令      ████████████████  75/75 (100%)
  IPC           ████████████████  ~98%
  前端组件      ████████████████  14/14 (100%)
  Vim 模式      ████████░░░░░░░░  ~50%
```
