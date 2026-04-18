# AGENTS.md — cc-rust (Lite)

This file provides guidance to Codex when working with the Rust Lite version in `rust/`.

## 分支说明

本分支 (`rust-lite`) 是完整版 (`master`) 的精简版，保留最小可用核心 + Skills 系统。

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

# ink-terminal 实验 UI
cd ink-ui && bun install && bun run dev

# Web UI 资源（用于 --web）
cd web-ui && npm install && npm run build

# 修改 ink-terminal submodule 时再同步
git submodule update --init --recursive
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
├── ink-ui/                  ink-terminal 实验前端
│   ├── ink-terminal/        渲染库源码 (git submodule → Crsei/ink-terminal)
│   └── src/                 实验 UI 源码
│
├── web-ui/                  Web Chat UI 前端资源
│
├── docs/
│   ├── WORK_STATUS.md       当前完成度 / 未完成项总览
│   ├── IMPLEMENTATION_GAPS.md  注意点 / 缩减实现 / 设计限制总入口
│   ├── KNOWN_ISSUES.md      用户可感知问题跟踪
│   └── archive/             已完成功能的历史设计 / 计划 / 变更记录
└── architecture/
    └── ink-terminal-frontend.md  IPC 架构设计文档
```

### Frontends

- `ui/` 是当前默认终端 UI，基于 `@opentui/core` + `@opentui/react`
- `ink-ui/` 是独立的 ink-terminal 实验前端
- `ui/ink-terminal/` 仍作为 submodule 保留，主要服务于 ink-terminal 相关实验与复用代码

ink-terminal 修改流程：
```bash
cd ink-ui/ink-terminal
# 修改源码 → 提交推送
git add . && git commit -m "fix: ..." && git push
# 回主项目更新 submodule 引用
cd ../..
git add ink-ui/ink-terminal && git commit -m "chore: bump ink-terminal"
```

### IPC 架构

终端前端通过 `--headless` 模式与 Rust 后端通信:
- Rust 端: `src/ipc/protocol.rs` (协议类型) + `src/ipc/headless.rs` (事件循环)
- TS 端: `ui/src/ipc/client.ts` (spawn + JSONL) + `ui/src/ipc/protocol.ts`
- 详见: `architecture/ink-terminal-frontend.md`

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
