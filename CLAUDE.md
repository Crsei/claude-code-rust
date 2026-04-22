# CLAUDE.md — cc-rust (全量构建 / Full Build)

This file provides guidance to Claude Code when working with the Rust port in `rust/`.

## 当前阶段：全量构建（Full Build）

> **重要**：本分支历史上曾标记为 `rust-lite`（完整版的精简版）。现已进入**全量构建阶段**，目标是与上游完整版 (`master` / TypeScript `cc/src/`) 行为对齐。
>
> 书写与审阅规则：
> - **不再按 Lite 缩减**。任何新代码都应覆盖上游对应模块的完整行为，不再以"精简版"为由省略分支、截断、错误恢复、沙箱、UI 细节等。
> - **已有的缩减实现视为 TODO**，不是既定边界。具体清单见 [`docs/IMPLEMENTATION_GAPS.md`](docs/IMPLEMENTATION_GAPS.md) §2 与 [`docs/archive/COMPLETED_SIMPLIFIED.md`](docs/archive/COMPLETED_SIMPLIFIED.md)；补齐后请把条目迁移到 [`docs/archive/COMPLETED_FULL.md`](docs/archive/COMPLETED_FULL.md)。
> - **历史 `Deferred` 清单需重评**。[`docs/WORK_STATUS.md`](docs/WORK_STATUS.md) §3 "显式延期"不再默认等于"不做"；新工作如触及这些条目，按上游完整实现对齐，除非另有书面确认。
> - **上游参考**：需要对照行为时，读 `F:\AIclassmanager\cc\src\**`（TypeScript 原版）或 `F:\AIclassmanager\cc\claude-code-bun\**`（Bun 版），不要只参照本仓库历史的 Rust 简化版。
> - **如确需保留某项缩减**（平台差异、许可、明确不做），必须在 PR 描述中显式说明，并在对应文档标注为"故意保留"（Intentional），而不是沉默地继续按简化版写。

历史名称 `rust-lite` 仍保留在分支名与部分文档链接中，仅作为版本标识，不再承担"按精简版维护"的语义。

## Path Isolation (Critical)

cc-rust 和原版 Claude Code (TypeScript) 共存于同一台机器上，**所有持久化路径必须隔离**：

| 用途 | 原版 Claude Code | cc-rust (本项目) |
|------|-----------------|-----------------|
| 全局数据目录 | `~/.claude/` | `~/.cc-rust/` |
| 项目配置 | `.claude/settings.json` | `.cc-rust/settings.json` |
| 项目技能 | `.claude/skills/` | `.cc-rust/skills/` |
| Keychain 服务名 | `"claude-code"` | `"cc-rust"` |
| 项目指令文件 | `CLAUDE.md` | `CLAUDE.md` (共享) |

### `CC_RUST_HOME` override

To place all runtime data somewhere other than `~/.cc-rust/`, set the
`CC_RUST_HOME` environment variable. See [docs/STORAGE.md](docs/STORAGE.md)
for the canonical path reference and fallback behavior.

## Build

```bash
# Rust 后端
cargo build --release

# 当前终端 UI (OpenTUI)
cd ui && bun install && bun run dev
```

### 构建前资源检查（多 worktree 并行开发）

- 在执行任何 `cargo build` / `cargo check` / `cargo test` / `cargo clippy` 之前，先检查整机内存占用与当前 `rust-analyzer` 进程数。
- 多个 worktree 并行开发时，全机**最多只能同时运行 2 个 `rust-analyzer`**。如果准备打开第 3 个 Rust worktree，必须先关闭一个已有 worktree 的编辑器 / LSP，或显式停掉对应 `rust-analyzer`。
- 如果当前内存压力已经偏高，不要继续叠加新的 `cargo` 编译任务；先释放内存，再开始构建、测试或 `clippy`。
- Windows / PowerShell 下优先用下面的命令做快速检查：

```powershell
Get-CimInstance Win32_OperatingSystem |
  Select-Object @{Name='TotalGB';Expression={[math]::Round($_.TotalVisibleMemorySize / 1MB, 1)}},
                @{Name='FreeGB';Expression={[math]::Round($_.FreePhysicalMemory / 1MB, 1)}}

Get-Process rust-analyzer -ErrorAction SilentlyContinue |
  Select-Object Id, ProcessName,
                @{Name='WorkingSetGB';Expression={[math]::Round($_.WorkingSet64 / 1GB, 2)}},
                Path
```

如果发现 `rust-analyzer` 已经达到 2 个，默认先不要再开新的 Rust worktree IDE 会话，也不要在新的 worktree 里直接跑 `cargo`，先回收已有工作树的 LSP / 内存占用。

### 全局快捷启动

在 PowerShell `$PROFILE` 中添加：

```powershell
function cc-rust { & "F:\AIclassmanager\cc\rust\ui\run.ps1" @args }
```

之后在任意目录输入 `cc-rust` 即可启动。

## Project Structure

```
rust/
├── src/                     Rust 后端
│   ├── main.rs              入口 (Phase A/B/I lifecycle, --headless/--daemon flag)
│   ├── types/               核心类型
│   ├── engine/              QueryEngine + 系统提示词
│   │   └── lifecycle/       QueryEngine 生命周期 (mod, types, submit_message, deps, helpers)
│   ├── query/               异步流式查询循环 (loop_impl + loop_helpers)
│   ├── tools/               30 个工具 + background_agents (后台代理类型)
│   ├── skills/              技能系统 (内置 + 用户自定义)
│   ├── compact/             上下文压缩管道
│   ├── commands/            36 个斜杠命令
│   ├── api/                 API 客户端 (Anthropic, OpenAI, Google)
│   ├── auth/                认证 (API Key + Keychain + OAuth PKCE)
│   │   └── oauth/           OAuth 子模块 (pkce, config, client)
│   ├── permissions/         权限系统
│   ├── config/              配置管理 + Feature Gate (features.rs)
│   ├── session/             会话持久化
│   ├── daemon/              KAIROS daemon (HTTP server + tick loop + channels + team_memory_proxy)
│   ├── lsp_service/         LSP 服务 (JSON-RPC 传输 + 客户端 + 类型转换, 9 操作全实现)
│   ├── ipc/                 IPC 协议 + headless 模式 (JSONL over stdio)
│   ├── ui/                  TUI legacy (ratatui + crossterm)
│   ├── utils/               工具函数
│   └── shutdown.rs          优雅关闭
│
├── ui/                      OpenTUI 前端
│   ├── src/
│   │   ├── components/      14 个 React 组件
│   │   ├── ipc/             IPC 客户端 + 协议类型 (与 Rust 端一致)
│   │   ├── store/           状态管理 (useReducer)
│   │   ├── vim/             Vim 模式状态机
│   │   └── main.tsx         前端入口
│   ├── team-memory-server/  Team Memory TS 服务 (Bun + SQLite)
│   │   ├── index.ts         HTTP server 入口 (Bun.serve, 密钥认证)
│   │   ├── db.ts            SQLite 数据层 (bun:sqlite, WAL, 事务)
│   │   └── routes.ts        GET/PUT 端点 (ETag, 304, 412, 413)
│   └── run.sh               启动脚本
│
├── docs/
│   ├── KNOWN_ISSUES.md      已知 UI/UX 问题跟踪
│   └── ...
└── architecture/
    └── ink-terminal-frontend.md  终端前端演进历史说明
```

### Terminal Frontend

`ui/` 是当前默认终端前端，基于 `@opentui/core` + `@opentui/react`。

`ink-ui/` / `ui/ink-terminal/` 路线已退役，不再作为当前仓库主线依赖保留。

### IPC 架构

两种前后端通信模式:

**Headless 模式** (`--headless`): JSONL over stdio
- Rust 端: `src/ipc/protocol.rs` (协议类型) + `src/ipc/headless.rs` (事件循环, `tokio::select!` 多路复用)
- TS 端: `ui/src/ipc/client.ts` (spawn + JSONL) + `ui/src/ipc/protocol.ts`
- 详见: `architecture/ink-terminal-frontend.md`（历史演进说明）

**Daemon 模式** (`--daemon`, KAIROS): HTTP/SSE over localhost
- Rust 端: `src/daemon/server.rs` (axum HTTP) + `src/daemon/sse.rs` (SSE 事件流) + `src/daemon/routes.rs` (12 个端点)
- TS 端: `ui/src/ipc/daemon-client.ts` (fetch + EventSource)
- 前端可随时 attach/detach，daemon 持续运行

### KAIROS — 常驻助手模式

通过 `FEATURE_*` 环境变量启用，`--daemon` 启动 daemon 进程:

```
src/daemon/
├── mod.rs                  入口
├── state.rs                DaemonState (共享状态, SSE 客户端管理, 事件缓冲, team_memory_port/secret)
├── server.rs               axum HTTP server (127.0.0.1:19836)
├── routes.rs               REST 端点 (submit, abort, attach, detach, webhook, team_memory...)
├── team_memory_proxy.rs    Team Memory 代理 (spawn Bun 子进程 + reqwest 转发)
├── sse.rs                  SSE 事件流 (断线重连, Last-Event-ID)
├── tick.rs                 Proactive tick 循环 (30s 间隔, 自主执行)
├── channels.rs             ChannelManager (MCP + Webhook 消息路由, allowlist)
├── webhook.rs              Webhook 签名验证 (GitHub HMAC-SHA256, Slack)
├── notification.rs         推送通知 (Windows Toast + Webhook 回调)
└── memory_log.rs           每日日志 (~/.cc-rust/logs/YYYY/MM/YYYY-MM-DD.md)
```

Feature Gate 系统: `src/config/features.rs`
- `FEATURE_KAIROS` — 主开关
- `FEATURE_KAIROS_BRIEF` — BriefTool 结构化输出
- `FEATURE_KAIROS_CHANNELS` — 外部 MCP Channel 消息
- `FEATURE_KAIROS_PUSH_NOTIFICATION` — 推送通知
- `FEATURE_KAIROS_GITHUB_WEBHOOKS` — GitHub Webhook
- `FEATURE_PROACTIVE` — 自主 tick 循环 (可独立启用，KAIROS 隐含启用)
- `FEATURE_TEAMMEM` — Team Memory 团队共享记忆 (独立于 KAIROS)

新增工具: `Sleep` (tick 休眠控制), `Brief` (结构化输出)
新增命令: `/brief`, `/sleep`, `/assistant`, `/daemon`, `/notify`, `/channels`, `/dream`
MCP Channel: `src/mcp/channel.rs` (capabilities 检测 + 通知解析)

设计文档: `docs/superpowers/specs/2026-04-11-kairos-design.md`

### Team Memory — 团队共享记忆

通过 `FEATURE_TEAMMEM=1` 启用，需配合 `--daemon` 模式。

架构: Rust daemon 代理转发 → 独立 Bun TS 服务 → SQLite 存储

```
前端 → Rust Daemon (:19836) → TS Server (:19837) → ~/.cc-rust/team-memory.db
         /api/claude_code/team_memory (GET/PUT)
```

Rust 端:
- `src/daemon/team_memory_proxy.rs` — spawn Bun 子进程 + reqwest HTTP 转发
- 共享密钥认证 (`X-Team-Memory-Secret` header, UUID v4)
- 启动时健康检查 (5s 超时, 100ms 轮询)

TS 端:
```
ui/team-memory-server/
├── index.ts     Bun.serve 入口 (CLI 参数, 密钥中间件, 优雅关闭)
├── db.ts        bun:sqlite (team_memory + repo_meta 表, WAL 模式, 事务写入)
└── routes.ts    GET (全量/hashes/304) + PUT (upsert, ETag 乐观锁, 412/413)
```

API 端点:
- `GET  /api/claude_code/team_memory?repo={owner/repo}` — 全量/条件请求
- `GET  ...&view=hashes` — 仅 checksums
- `PUT  /api/claude_code/team_memory?repo={owner/repo}` — upsert (If-Match ETag)

设计文档: `docs/superpowers/specs/2026-04-11-team-memory-design.md`

### 已移除的模块 (完整版有)

analytics, remote

### 新增服务模块

services/ — tool_use_summary, session_memory, prompt_suggestion, lsp_lifecycle

### Auth Flow

```
ApiClient::from_auth()
  ├─ from_env(): 环境变量检测 (ANTHROPIC_API_KEY, OPENAI_API_KEY, ...)
  └─ auth::resolve_auth():
       1. ANTHROPIC_API_KEY env → ApiKey
       2. ANTHROPIC_AUTH_TOKEN env → ExternalToken
       3. ~/.cc-rust/credentials.json → OAuthToken (自动刷新过期 token)
       4. 系统 Keychain ("cc-rust") → ApiKey
       5. None
```

OAuth 登录流程 (`/login 2|3` + `/login-code`):
- PKCE: `src/auth/oauth/pkce.rs` (code_verifier + code_challenge + state)
- 端点: `src/auth/oauth/config.rs` (platform.claude.com OAuth endpoints)
- HTTP: `src/auth/oauth/client.rs` (token exchange + refresh + create_api_key)
- 状态: `src/commands/login_code.rs` (PENDING_OAUTH static, 两步流程)

### 注意事项

- 每次写完代码，编译过后查看有没有 warning，解决 warning（必须保证未使用的都在代码中起作用），然后构建相应的 e2e test
- 运行任何 `cargo` 相关命令前，先检查内存占用和 `rust-analyzer` 数量；多 worktree 并行时全机最多 2 个 `rust-analyzer`
- UI 已知问题记录在 `docs/KNOWN_ISSUES.md`，用户反馈的问题追加到该文件
