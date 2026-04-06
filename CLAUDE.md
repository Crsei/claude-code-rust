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
cargo build --release
```

## Architecture

```
src/
├── main.rs              入口 (Phase A/B/I lifecycle)
├── types/               核心类型
├── engine/              QueryEngine + 系统提示词
├── query/               异步流式查询循环
├── tools/               24 个工具 (Bash, Read, Write, Edit, Glob, Grep, AskUser, Agent, Skill, WebFetch, WebSearch, EnterPlanMode, ExitPlanMode, TaskCreate, TaskGet, TaskUpdate, TaskList, TaskStop, TaskOutput, PowerShell, Config, REPL, StructuredOutput, SendUserMessage)
├── skills/              技能系统 (内置 + 用户自定义)
├── compact/             上下文压缩管道 (microcompact, snip, pipeline, tool_result_budget)
├── commands/            27 个斜杠命令 (含 /compact)
├── api/                 API 客户端 (Anthropic, OpenAI, Google)
├── auth/                认证 (API Key + Keychain)
├── permissions/         权限系统
├── config/              配置管理
├── session/             会话持久化
├── ui/                  TUI (ratatui + crossterm)
├── utils/               工具函数
└── shutdown.rs          优雅关闭
```

### 已移除的模块 (完整版有)

mcp, analytics, remote, plugins, lsp_service, teams

### 新增服务模块

services/ — tool_use_summary, session_memory, prompt_suggestion, lsp_lifecycle

### Auth Flow

```
ApiClient::from_auth()
  ├─ from_env(): 环境变量检测 (ANTHROPIC_API_KEY, OPENAI_API_KEY, ...)
  └─ auth::resolve_auth(): ANTHROPIC_AUTH_TOKEN → 系统 Keychain ("cc-rust")
```

### 注意事项

每次写完代码，编译过后查看有没有warning,解决warning(必须保证未使用的都在代码中起作用),然后构建相应的e2e test