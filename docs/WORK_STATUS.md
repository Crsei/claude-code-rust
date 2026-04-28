# cc-rust 工作状态总览

> 更新日期: 2026-04-22 | 分支: `rust-lite`（历史名称，当前阶段：**全量构建 / Full Build**）
>
> **阶段说明**：本仓库已从 "rust-lite 精简版" 切换到**全量构建**。§3 原"显式延期 (Deferred)"清单不再默认等于"不做"，触及这些条目的新工作默认按上游完整实现对齐，除非重新评估后登记到 [`IMPLEMENTATION_GAPS.md`](IMPLEMENTATION_GAPS.md) §7 "Intentional 裁剪"。详细规则见 [`../CLAUDE.md`](../CLAUDE.md) 顶部"当前阶段"说明。
>
> 本文档合并了原 `UNIMPLEMENTED_CHECKLIST.md`、`sdk-work-tracker.md`、`unfinished-features.md`
> 三份文档，作为唯一的状态跟踪入口。
>
> 缩减实现、设计限制与注意事项统一汇总见 [`IMPLEMENTATION_GAPS.md`](IMPLEMENTATION_GAPS.md)。
>
> 与 `claude-code-bun` 的差异、Web UI 进行度、REPL 结构对比与后续路线图见 [`claude-code-bun-gap-plan.md`](claude-code-bun-gap-plan.md)。

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

**收口状态**：in-process 闭环 + 用户面全量已落地。env var 不再是唯一开关，`/team create` 或 `TeamSpawn` 工具会在会话内即时解锁 team 功能。MVP-005 已明确 backend 策略：cc-rust 只支持 in-process；tmux/iTerm2 pane backend 记录为 intentional crop。

| 子模块 | 文件 | 状态 |
|--------|------|------|
| in-process runner | `src/teams/runner.rs` | ✅ — 驱动子 QueryEngine，处理 mailbox 协议消息；首轮完成后保持 idle loop，后续 mailbox 普通消息会继续进入同一 teammate 会话 |
| `SendMessage` 工具 | `src/tools/send_message.rs` | ✅ — 对话内消息路由；`is_enabled` 总返回 true，call 时检查 team_context |
| `TeamSpawn` 工具 | `src/tools/team_spawn.rs` | ✅ — 对话内拉起 teammate，缺 team 时自动建 session 团队 |
| `/team` 斜杠命令 | `src/commands/team_cmd.rs` | ✅ — `create / list / status / spawn / send / kill / leave / delete` |
| Team Dashboard | `ui/src/components/TeamPanel.tsx` | ✅ — 订阅 `BackendMessage::TeamEvent`，展示成员/未读/最近消息 |
| IPC QueryTeamStatus | `src/ipc/agent_handlers.rs` | ✅ — `build_team_status_events` 读盘后发 `StatusSnapshot` |
| 终端后端 trait | `src/teams/backend.rs` | **Intentional crop** — `SUPPORTED_BACKENDS` 仅包含 `in-process`；`PaneBackend` trait 只保留为未来 parity 审查边界 |

- 激活入口：`crate::teams::is_agent_teams_active(&app_state)` — env var 或 `team_context` 任一满足即启用
- ingress 同步：`src/ipc/ingress.rs` 斜杠命令执行后把 `app_state.team_context` 同步回 engine
- 已完成 (10/11 含 runner/backend + SendMessage + TeamSpawn + /team 命令 + TeamPanel)

### 1.4 工具

| 工具 | 文件 | 状态 |
|------|------|------|
| Agent 后台模式 | `src/tools/agent.rs` | ✅ — `run_in_background` 通过 tokio::spawn + mpsc 异步执行 |

其余 29 个工具均已完整实现 (含 BriefTool, SleepTool)。

### 1.5 权限系统

| 阶段 | 文件 | 状态 |
|------|------|------|
| Phase 2 (Hook 拦截) | `src/permissions/decision.rs:259-362` + `src/tools/execution/pipeline.rs:124-211` | ✅ — 预执行 hook 结果经 `HookPermissionDecision` 折入中心决策，deny/ask/allow 按 spec 顺序生效 |

已完成: Phase 1a/1b 规则匹配, Phase 2 hook 拦截, Phase 3 模式检查。

### 1.6 IPC

| 功能 | 文件 | 状态 |
|------|------|------|
| clear_messages | `src/engine/lifecycle/mod.rs:245` + `src/ipc/ingress.rs:332-339` | ✅ — `/clear` 命令调用 `engine.clear_messages()` 真清空后端历史，再广播 `conversation_replaced` 给前端 |

### 1.7 前端 (终端 UI)

| 功能 | 文件 | 状态 |
|------|------|------|
| Vim 状态机 | `ui/src/vim/state-machine.ts` | ✅ — normal/insert/visual 三模式；导航 (h/l/0/$/^/w/b/e)、operator (d/y/c)、单键 (x/X/p/u/D/C) 与 visual 选区；不计划扩展到完整 Vim 语义 |
| 终端 resize 回流 | `src/ui/tui.rs` + `src/ui/virtual_scroll.rs` | ✅ (Rust TUI 端 2026-04-19) / **Open** (TS/OpenTUI 端) — 见 KNOWN_ISSUES #1 |
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
| P1-3 | Web 搜索缓存层 | ✅ |
| P1-4 | LSP 9/9 方法实现 | ✅ |
| P1-5 | Team Memory 团队共享记忆 | ✅ (服务端最小实现, 客户端同步待做) |

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

## 3. 历史 Deferred 清单（进入全量构建后需逐项重评）

> **状态反转**：以下条目历史上登记为"`rust-lite` 范围外"。进入全量构建阶段后，它们**不再自动等于"不实现"**。规则：
>
> - **默认行为**：触及任一条目的新工作按上游完整版对齐。
> - **如要继续延期**：在 [`IMPLEMENTATION_GAPS.md`](IMPLEMENTATION_GAPS.md) §7 "Intentional 裁剪"里登记理由与复审条件，再从本节删除。
> - **Ant-only 内部工具**：继续不实现的可信度高；仍需迁移到 §7 以正式化。
> - **远程控制 / 服务端扩展 / 多端集成**：按路线图重评，默认进入 TODO 队列。

保留作为历史对照 →

### 历史延期命令（需重评）

`/remote-control`, `/web-setup`, `/chrome`, `/desktop`, `/mobile`,
`/remote-env`, `/release-notes`, `/stickers`, `/terminal-setup`, `/usage`

### 历史延期工具（需重评）

`RemoteTrigger`, `CronCreate/Delete/List`, `WebBrowser`, `McpAuthTool`,
`Monitor`, `ListPeers`, `Workflow`, `TerminalCapture`, `SubscribePR`,
`PushNotification`, `SendUserFile`, `SuggestBackgroundPR`

### 历史延期模块（需重评）

| 模块 | 说明 |
|------|------|
| `bridge/` | 远程控制桥接 |
| `cli/transports/` | SSE, WebSocket, Worker 传输 |
| `server/` | 服务器模式 |
| `remote/` | 云容器 (CCR) |
| `services/remoteManagedSettings/` | MDM + 远程设置同步 |
| `services/analytics/` | 遥测管道 |
| Desktop / Mobile 集成 | — |

### 内部 / Ant-Only 命令（建议保留延期，但仍需正式登记到 Intentional 裁剪）

`/agents-platform`, `/ant-trace`, `/autofix-pr`, `/backfill-sessions`,
`/break-cache`, `/bridge-kick`, `/bughunter`, `/ctx-viz`,
`/debug-tool-call`, `/env`, `/good-claude`, `/init-verifiers`,
`/issue`, `/mock-limits`, `/oauth-refresh`, `/onboarding`,
`/perf-issue`, `/reset-limits`, `/share`, `/summary`,
`/teleport`, `/heapdump`

### 内部工具（建议保留延期，但仍需正式登记到 Intentional 裁剪）

`CtxInspect`, `OverflowTest`, `VerifyPlanExecution`, `Tungsten`

---

## 4. 已完成基线

> 详细清单见 [`archive/COMPLETED_FULL.md`](archive/COMPLETED_FULL.md) 和 [`archive/COMPLETED_SIMPLIFIED.md`](archive/COMPLETED_SIMPLIFIED.md)。

- **斜杠命令**: 75/75 (含 `/login-code`, `/extra-usage`, `/rate-limit-options`)
- **工具**: 30 个 (Bash, Read, Write, Edit, Grep, Glob, Agent, Skill, LSP, Tasks, Web, PowerShell, Brief, Sleep...)
- **API 提供商**: 4/6 (Anthropic, OpenAI, Google, Azure)
- **认证**: API Key + Keychain + OAuth PKCE (Claude.ai Bearer / Console API Key)
- **核心模块**: engine, query, compact, session, permissions, config, ipc, skills, plugins, mcp, lsp_service, daemon, ui
- **新增功能**: Git 上下文注入 system prompt, Web 搜索 TTL 缓存, Agent 后台执行, Feature Gate 系统, Team Memory (Rust 代理 + TS/SQLite 服务)
- **前端组件**: 14/14 (App, Header, MessageList, InputPrompt, MessageBubble, ToolUse/Result, PermissionDialog, Suggestions, WelcomeScreen, StatusBar, Spinner, ThinkingBlock, DiffView)

---

## 5. 完成度总览

```
  API 提供商    ████████████░░░░  4/6 (67%) — Bedrock/Vertex 单独立项
  认证          ████████████████  4/4 (100%) ✅
  Teams 系统    ████████████████  in-process + /team + TeamSpawn + Dashboard ✅
  工具          ████████████████  30/30 (100%) ✅
  权限          ████████████████  3/3 phases ✅ (Phase 2 hook 已接入中心决策)
  斜杠命令      ████████████████  75/75 (100%)
  IPC           ████████████████  clear_messages 已落地
  前端组件      ████████████████  14/14 (100%)
  Vim 模式      ████████████████  ~90% (normal/insert/visual + motions/ops)
```
