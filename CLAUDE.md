# CLAUDE.md — cc-rust (Lite)

This file provides guidance to Claude Code when working with the Rust Lite version in `rust/`.

## 分支说明

本分支 (`rust-lite`) 是完整版 (`master`) 的精简版，保留最小可用核心 + Skills 系统。

## Path Isolation (Critical)

cc-rust 和原版 Claude Code (TypeScript) 共存于同一台机器上，**所有持久化路径必须隔离**：

| 用途 | 原版 Claude Code | cc-rust (本项目) |
|------|-----------------|-----------------|
| 全局数据目录 | `~/.claude/` | `~/.cc-rust/` |
| 项目配置 | `.claude/settings.json` | `.cc-rust/settings.json` |
| 项目技能 | `.claude/skills/` | `.cc-rust/skills/` |
| Keychain 服务名 | `"claude-code"` | `"cc-rust"` |
| 项目指令文件 | `CLAUDE.md` | `CLAUDE.md` (共享) |

## Build

```bash
# Rust 后端
cargo build --release

# ink-terminal 前端 (含 submodule)
git submodule update --init --recursive
cd ui && bun install && bun run dev
```

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
│   ├── main.rs              入口 (Phase A/B/I lifecycle, --headless flag)
│   ├── types/               核心类型
│   ├── engine/              QueryEngine + 系统提示词
│   │   └── lifecycle/       QueryEngine 生命周期 (mod, types, submit_message, deps, helpers)
│   ├── query/               异步流式查询循环 (loop_impl + loop_helpers)
│   ├── tools/               28 个工具
│   ├── skills/              技能系统 (内置 + 用户自定义)
│   ├── compact/             上下文压缩管道
│   ├── commands/            28 个斜杠命令
│   ├── api/                 API 客户端 (Anthropic, OpenAI, Google)
│   ├── auth/                认证 (API Key + Keychain)
│   ├── permissions/         权限系统
│   ├── config/              配置管理
│   ├── session/             会话持久化
│   ├── ipc/                 IPC 协议 + headless 模式 (JSONL over stdio)
│   ├── ui/                  TUI legacy (ratatui + crossterm)
│   ├── utils/               工具函数
│   └── shutdown.rs          优雅关闭
│
├── ui/                      ink-terminal 前端 (bun workspace root)
│   ├── ink-terminal/        渲染库源码 (git submodule → Crsei/ink-terminal)
│   ├── src/
│   │   ├── components/      14 个 React 组件
│   │   ├── ipc/             IPC 客户端 + 协议类型 (与 Rust 端一致)
│   │   ├── store/           状态管理 (useReducer)
│   │   ├── vim/             Vim 模式状态机
│   │   └── main.tsx         前端入口
│   ├── package.json         workspaces: ["ink-terminal"]
│   └── run.sh               启动脚本
│
├── docs/
│   ├── KNOWN_ISSUES.md      已知 UI/UX 问题跟踪
│   └── ...
└── architecture/
    └── ink-terminal-frontend.md  IPC 架构设计文档
```

### Monorepo (ui/)

`ui/` 是 bun workspace root，包含两个包：
- `cc-rust-ui` (ui/) — 应用层，`import { Box } from 'ink-terminal'`
- `ink-terminal` (ui/ink-terminal/) — 渲染库，作为 **git submodule** 管理

ink-terminal 修改流程：
```bash
cd ui/ink-terminal
# 修改源码 → 提交推送
git add . && git commit -m "fix: ..." && git push
# 回主项目更新 submodule 引用
cd ../..
git add ui/ink-terminal && git commit -m "chore: bump ink-terminal"
```

### IPC 架构

ink-terminal 前端通过 `--headless` 模式与 Rust 后端通信:
- Rust 端: `src/ipc/protocol.rs` (协议类型) + `src/ipc/headless.rs` (事件循环)
- TS 端: `ui/src/ipc/client.ts` (spawn + JSONL) + `ui/src/ipc/protocol.ts`
- 详见: `architecture/ink-terminal-frontend.md`

### 已移除的模块 (完整版有)

analytics, remote

### 新增服务模块

services/ — tool_use_summary, session_memory, prompt_suggestion, lsp_lifecycle

### Auth Flow

```
ApiClient::from_auth()
  ├─ from_env(): 环境变量检测 (ANTHROPIC_API_KEY, OPENAI_API_KEY, ...)
  └─ auth::resolve_auth(): ANTHROPIC_AUTH_TOKEN → 系统 Keychain ("cc-rust")
```

### 注意事项

- 每次写完代码，编译过后查看有没有 warning，解决 warning（必须保证未使用的都在代码中起作用），然后构建相应的 e2e test
- UI 已知问题记录在 `docs/KNOWN_ISSUES.md`，用户反馈的问题追加到该文件