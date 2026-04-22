# AGENTS.md — cc-rust (全量构建 / Full Build)

This file provides guidance to Codex when working with the Rust port in `rust/`.

## 当前阶段：全量构建（Full Build）

> **重要**：本分支历史上曾标记为 `rust-lite`（完整版的精简版）。现已进入**全量构建阶段**，目标是与上游完整版 (`master` / TypeScript `cc/src/`) 行为对齐。
>
> 书写与审阅规则：
> - **不再按 Lite 缩减**。新代码应覆盖上游对应模块的完整行为，不要以"精简版"为由省略分支、截断、错误恢复、沙箱、UI 细节等。
> - **已有的缩减实现视为 TODO**，不是既定边界。清单见 [`docs/IMPLEMENTATION_GAPS.md`](docs/IMPLEMENTATION_GAPS.md) §2 与 [`docs/archive/COMPLETED_SIMPLIFIED.md`](docs/archive/COMPLETED_SIMPLIFIED.md)；补齐后迁移到 [`docs/archive/COMPLETED_FULL.md`](docs/archive/COMPLETED_FULL.md)。
> - **历史 `Deferred` 清单需重评**。[`docs/WORK_STATUS.md`](docs/WORK_STATUS.md) §3 不再默认等于"不做"；触及这些条目时按上游完整实现对齐，除非另有书面确认。
> - **上游参考**：对照行为时读 `F:\AIclassmanager\cc\src\**`（TypeScript 原版）或 `F:\AIclassmanager\cc\claude-code-bun\**`（Bun 版）。
> - **如确需保留某项缩减**，在 PR 描述中显式说明，并在文档标注为"故意保留"（Intentional），而不是沉默继续按简化版写。

历史名称 `rust-lite` 仍保留在分支名与部分文档链接中，仅作为版本标识，不再承担"按精简版维护"的语义。

## Path Isolation (Critical)

cc-rust 和原版 Codex (TypeScript) 共存于同一台机器上，**所有持久化路径必须隔离**：

| 用途 | 原版 Codex | cc-rust (本项目) |
|------|-----------------|-----------------|
| 全局数据目录 | `~/.Codex/` | `~/.cc-rust/` |
| 项目配置 | `.Codex/settings.json` | `.cc-rust/settings.json` |
| 项目技能 | `.Codex/skills/` | `.cc-rust/skills/` |
| Keychain 服务名 | `"Codex"` | `"cc-rust"` |
| 项目指令文件 | `AGENTS.md` | `AGENTS.md` (共享) |

## Build

```bash
# Rust 后端
cargo build --release

# 当前终端 UI (OpenTUI)
cd ui && bun install && bun run dev

# Web UI 资源（用于 --web）
cd web-ui && npm install && npm run build

```

## Project Structure

```
rust/
├── src/                     Rust 后端
│   ├── main.rs              入口 (Phase A/B/I lifecycle, --headless flag)
│   ├── types/               核心类型
│   ├── engine/              QueryEngine + 系统提示词
│   │   └── lifecycle/       QueryEngine 生命周期 (mod, types, submit_message, deps, helpers)
│   ├── query/               异步流式查询循环 (loop_impl + loop_helpers)
│   ├── tools/               工具系统（含 Agent / LSP / Web / Brief / Sleep）
│   ├── skills/              技能系统 (内置 + 用户自定义)
│   ├── compact/             上下文压缩管道
│   ├── commands/            斜杠命令系统
│   ├── api/                 API 客户端 (Anthropic / OpenAI / Google / Azure / OpenAI Codex)
│   ├── auth/                认证 (API Key + Keychain + OAuth + Codex CLI fallback)
│   ├── permissions/         权限系统
│   ├── config/              配置管理
│   ├── session/             会话持久化
│   ├── ipc/                 IPC 协议 + headless 模式 (JSONL over stdio)
│   ├── daemon/              daemon + Team Memory 代理
│   ├── web/                 Web 模式静态资源与路由支持
│   ├── services/            tool_use_summary / session_memory / prompt_suggestion / lsp_lifecycle
│   ├── ui/                  Rust TUI (ratatui + crossterm)
│   ├── utils/               工具函数
│   └── shutdown.rs          优雅关闭
│
├── ui/                      OpenTUI 前端
│   ├── src/
│   │   ├── components/      React 组件
│   │   ├── ipc/             IPC 客户端 + 协议类型
│   │   ├── store/           状态管理
│   │   └── main.tsx         前端入口
│   ├── team-memory-server/  Team Memory TS/SQLite 服务
│   ├── subagent-dashboard/  Subagent 调试面板
│   └── run.sh               启动脚本
│
├── web-ui/                  Web Chat UI 前端资源
│
├── docs/
│   ├── WORK_STATUS.md       当前完成度 / 未完成项总览
│   ├── IMPLEMENTATION_GAPS.md  注意点 / 缩减实现 / 设计限制总入口
│   ├── KNOWN_ISSUES.md      用户可感知问题跟踪
│   └── archive/             已完成功能的历史设计 / 计划 / 变更记录
└── architecture/
    └── ink-terminal-frontend.md  终端前端演进历史说明
```

### Frontends

- `ui/` 是当前默认终端 UI，基于 `@opentui/core` + `@opentui/react`
- `ink-ui/` 已退役，不再维护
- `ui/ink-terminal/` 已不再作为当前前端依赖保留

### IPC 架构

终端前端通过 `--headless` 模式与 Rust 后端通信:
- Rust 端: `src/ipc/protocol.rs` (协议类型) + `src/ipc/headless.rs` (事件循环)
- TS 端: `ui/src/ipc/client.ts` (spawn + JSONL) + `ui/src/ipc/protocol.ts`
- 详见: `architecture/ink-terminal-frontend.md`（历史演进说明）

### 已移除的模块 (完整版有)

analytics, remote

### 文档入口

- `docs/WORK_STATUS.md`：当前完成度、未完成项、延期范围
- `docs/IMPLEMENTATION_GAPS.md`：注意事项、缩减实现、设计限制统一入口
- `docs/KNOWN_ISSUES.md`：用户可感知问题，持续追加
- `docs/archive/`：已经落地功能的历史方案、设计、日报、变更记录

### Auth Flow

```
ApiClient::from_backend()
  ├─ native  → auth::resolve_auth()
  │            ├─ ANTHROPIC_API_KEY / ANTHROPIC_AUTH_TOKEN
  │            └─ ~/.cc-rust/credentials.json / 系统 Keychain ("cc-rust")
  └─ codex   → auth::resolve_codex_auth_token()
               ├─ OPENAI_CODEX_AUTH_TOKEN
               ├─ ~/.cc-rust/credentials.json
               └─ ~/.codex/auth.json
```

### 注意事项

- 每次写完代码，编译过后查看有没有 warning，解决 warning（必须保证未使用的都在代码中起作用），然后构建相应的 e2e test
- UI 已知问题记录在 `docs/KNOWN_ISSUES.md`，用户反馈的问题追加到该文件
- Codex backend 当前行为看 `docs/codex-backend.md`；历史调研笔记已归档到 `docs/archive/implemented/codex-agent.md`
