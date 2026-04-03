# Claude Code Rust — Lite

精简版 Claude Code CLI 的 Rust 实现，保留最小可用核心 + Skills 系统，适合作为新项目的起点。

基于完整版 (`master` 分支) 裁剪而来，从 ~49,000 行 / 204 文件精简至 **~28,000 行 / 109 文件**。

---

## 快速开始

### 前置要求

- Rust 工具链 1.70+ (推荐 stable 最新版)
- 系统依赖:
  - Linux: `pkg-config`, `libssl-dev`, `cmake`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio C++ Build Tools

### 构建 & 运行

```bash
cargo build --release
./target/release/claude-code-rs
```

### 认证

```bash
# 设置 API Key (推荐)
export ANTHROPIC_API_KEY="sk-ant-api03-..."

# 或在 REPL 中登录
> /login sk-ant-api03-...
```

### 基本用法

```bash
# 交互式 REPL
claude-code-rs

# 单次查询
claude-code-rs -p "解释这段代码"

# 带初始提示启动
claude-code-rs "帮我重构这个函数"

# 指定模型
claude-code-rs -m claude-opus-4-20250514
```

---

## 架构概览

```
src/
├── main.rs              入口 (Phase A/B/I 生命周期)
├── types/               核心类型 (消息、工具、状态、配置)
├── engine/              QueryEngine 生命周期 + 系统提示词
├── query/               异步流式查询循环 (async_stream)
├── tools/               8 个内置工具 + 注册/执行/编排
├── skills/              技能系统 (内置 + 用户自定义)
├── commands/            24 个斜杠命令
├── api/                 API 客户端 (Anthropic / OpenAI / Google)
├── auth/                认证 (API Key + Keychain)
├── permissions/         权限系统 (4 种模式 + 危险命令检测)
├── config/              配置管理 (全局 + 项目 + 环境变量)
├── session/             会话持久化与恢复
├── ui/                  终端 TUI (ratatui + crossterm + Vim 模式)
├── utils/               工具函数 (git, shell, token 估算等)
└── shutdown.rs          优雅关闭
```

---

## 内置工具 (8 个)

| 工具 | 说明 |
|------|------|
| **Bash** | 执行 shell 命令，支持超时和输出截断 |
| **Read** | 读取文件内容，支持 offset/limit 分页 |
| **Write** | 写入/覆盖文件 |
| **Edit** | 文件内精确字符串替换，支持 fuzzy 匹配 |
| **Glob** | 文件名模式匹配 (`**/*.rs`) |
| **Grep** | 正则内容搜索，支持上下文行和多种输出模式 |
| **AskUser** | 向用户提问并等待回复 |
| **Skill** | 调用技能系统 (内置 + 用户自定义) |

---

## 斜杠命令 (24 个)

| 命令 | 别名 | 说明 |
|------|------|------|
| `/help` | `/h`, `/?` | 列出所有命令 |
| `/clear` | — | 清空对话历史 |
| `/exit` | `/quit`, `/q` | 退出 |
| `/version` | `/v` | 显示版本号 |
| `/model` | — | 查看/切换模型 |
| `/cost` | `/usage` | 查看 token 用量和费用 |
| `/fast` | — | 切换快速模式 |
| `/effort` | — | 设置 thinking 深度 |
| `/config` | `/settings` | 查看/修改配置 |
| `/permissions` | `/perms` | 查看/修改工具权限 |
| `/login` | — | 认证 (API Key) |
| `/logout` | — | 清除凭据 |
| `/commit` | — | 创建 git commit |
| `/diff` | — | 显示 git 变更 |
| `/branch` | `/br` | 查看/切换分支 |
| `/session` | — | 查看会话信息 |
| `/resume` | — | 恢复之前的会话 |
| `/context` | `/ctx` | 上下文使用信息 |
| `/files` | — | 列出对话引用的文件 |
| `/memory` | `/mem` | 管理 CLAUDE.md 指令 |
| `/skills` | — | 列出可用技能 |
| `/init` | — | 初始化项目配置 |
| `/copy` | `/cp` | 复制最后回复到剪贴板 |
| `/status` | — | 查看会话状态 |

---

## Skills 系统

技能是可复用的提示词模板，通过 Skill 工具调用。

### 内置技能

随程序打包的技能，无需额外安装即可使用。

### 用户自定义技能

在以下目录放置 `.md` 文件:

```
~/.cc-rust/skills/       # 全局技能
.cc-rust/skills/         # 项目技能
```

格式:

```markdown
---
name: my-skill
description: 一句话说明
---

技能的提示词内容...

$ARGUMENTS 会被调用参数替换
```

---

## 配置

### 文件位置

| 层级 | 路径 |
|------|------|
| 全局 | `~/.cc-rust/settings.json` |
| 项目 | `.cc-rust/settings.json` |

优先级: 环境变量 > 项目配置 > 全局配置

### 环境变量

| 变量 | 说明 |
|------|------|
| `ANTHROPIC_API_KEY` | API 密钥 |
| `ANTHROPIC_AUTH_TOKEN` | 外部认证 Token |
| `CLAUDE_MODEL` | 覆盖默认模型 |
| `CLAUDE_PERMISSION_MODE` | 权限模式 (`default`/`auto`/`bypass`) |

---

## 与完整版的差异

本分支从完整版 (`master`) 裁剪了以下模块:

| 已移除 | 说明 |
|--------|------|
| MCP | Model Context Protocol 服务器集成 |
| LSP | 代码导航 / Language Server Protocol |
| Agent Teams | 多智能体协作系统 |
| Plugins | 插件系统 |
| Compact | 上下文压缩管线 |
| Analytics | 遥测分析 |
| Remote | 远程会话 |
| 22 个工具 | Agent, WebFetch/Search, Tasks, PlanMode, Worktree 等 |
| 50 个命令 | 高级 git, IDE 集成, 远程功能等 |
| 8 个依赖 | aws-sdk, gcp_auth, lsp-types, tree-sitter, image 等 |

如需这些功能，请切换到 `master` 分支。

---

## 项目结构

```
rust/
├── Cargo.toml           依赖配置
├── CLAUDE.md            Claude Code 项目指令
├── README.md            本文件
├── docs/                文档
│   └── USAGE_GUIDE.md   详细使用指南
└── src/                 源码 (109 个 .rs 文件, ~28k 行)
```

---

## License

本项目是 Claude Code CLI 的学习/研究用途重写。
