# CLAUDE.md — cc-rust

This file provides guidance to Claude Code when working with the Rust rewrite in `rust/`.

## Path Isolation (Critical)

The Rust version (`cc-rust`) 和原版 Claude Code (TypeScript) 共存于同一台机器上，**所有持久化路径必须隔离**：

| 用途 | 原版 Claude Code | cc-rust (本项目) |
|------|-----------------|-----------------|
| 全局数据目录 | `~/.claude/` | `~/.cc-rust/` |
| 项目配置 | `.claude/settings.json` | `.cc-rust/settings.json` |
| 项目记忆 | `.claude/memory/` | `.cc-rust/memory/` |
| 项目技能 | `.claude/skills/` | `.cc-rust/skills/` |
| Keychain 服务名 | `"claude-code"` | `"cc-rust"` |
| 项目指令文件 | `CLAUDE.md` | `CLAUDE.md` (共享，不隔离) |

**规则：**
- 新增任何文件/目录持久化时，必须使用 `~/.cc-rust/` 或 `.cc-rust/` 前缀
- 不得使用 `~/.claude/` 或 `.claude/` 作为数据目录（会与原版冲突）
- `CLAUDE.md` 是唯一共享的文件（两个版本都应读取同一份项目指令）
- Keychain entry 使用 `"cc-rust"` 服务名，不得用 `"claude-code"`
- 全局数据根目录通过 `config::settings::global_claude_dir()` 获取，返回 `~/.cc-rust/`

## Build

只有一种构建方式——全功能构建，无 feature flag：

```bash
cargo build --release
```

## Architecture

Rust 重写保持与 TypeScript 原版相同的架构（见根目录 `CLAUDE.md`），主要差异：

- 入口: `src/main.rs` (Phase A/B/I lifecycle)
- 状态机: `src/engine/lifecycle.rs` (QueryEngine)
- API 客户端: `src/api/client.rs` (多提供商: Anthropic, OpenAI, Google, etc.)
- 认证: `src/auth/` (API Key + Keychain + External Token, OAuth 接口保留)
- TUI: `src/ui/tui.rs` (ratatui + crossterm 全屏终端)
- 工具: `src/tools/` (21+ 工具)
- 命令: `src/commands/` (27+ 斜杠命令)

### Auth Flow

```
ApiClient::from_auth()
  ├─ from_env(): 多提供商环境变量检测 (ANTHROPIC_API_KEY, OPENAI_API_KEY, ...)
  └─ auth::resolve_auth(): ANTHROPIC_AUTH_TOKEN → 系统 Keychain ("cc-rust")
```

`/login sk-ant-...` 将 key 存入 Keychain；`/logout` 清除 Keychain + 磁盘 Token。
