# Claude Code Rust — 使用指南

> 版本: 0.1.0 | 最后更新: 2026-04-01

---

## 目录

1. [构建与安装](#构建与安装)
2. [快速开始](#快速开始)
3. [命令行参数](#命令行参数)
4. [运行模式](#运行模式)
5. [斜杠命令](#斜杠命令)
6. [内置工具](#内置工具)
7. [权限系统](#权限系统)
8. [配置管理](#配置管理)
9. [会话管理](#会话管理)
10. [CLAUDE.md 项目指令](#claudemd-项目指令)
11. [上下文压缩](#上下文压缩)
12. [键盘快捷键](#键盘快捷键)
13. [构建说明](#构建说明)
14. [当前限制](#当前限制)
15. [故障排查](#故障排查)

---

## 构建与安装

### 前置要求

- **Rust 工具链**: 1.70+ (推荐 stable 最新版)
- **系统依赖**:
  - Linux: `pkg-config`, `libssl-dev`, `cmake`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio C++ Build Tools

### 构建

```bash
cd cc/rust
cargo build --release
```

生成的二进制文件位于 `target/release/claude-code-rs`。

默认构建即为全功能构建，包含网络、API、认证、LSP 等所有模块，无需额外指定 feature flag。

### 运行测试

```bash
cargo test
```

---

## 快速开始

### 1. 启动交互式 REPL

```bash
claude-code-rs
```

进入交互模式后，输入自然语言指令或斜杠命令即可。输入 `/exit` 或按 `Ctrl+D` 退出。

### 2. 单次查询 (Print 模式)

```bash
claude-code-rs -p "解释这段代码的作用"
```

输出模型回复后立即退出，适合管道和脚本集成。

### 3. 带初始提示启动

```bash
claude-code-rs "帮我重构这个函数"
```

启动 REPL 并自动发送第一条消息。

### 4. 查看版本

```bash
claude-code-rs --version
```

---

## 命令行参数

| 参数 | 短写 | 说明 | 默认值 |
|------|------|------|--------|
| `--version` | `-V` | 打印版本号并退出 | — |
| `--print` | `-p` | Print 模式: 输出回复后退出 | false |
| `--resume` | — | 恢复最近一次会话 | false |
| `--continue <ID>` | — | 恢复指定会话 ID | — |
| `--max-turns <N>` | — | 最大对话轮数限制 | 无限 |
| `--cwd <PATH>` | `-C` | 指定工作目录 | 当前目录 |
| `--model <MODEL>` | `-m` | 指定模型 | `claude-sonnet-4-20250514` |
| `--system-prompt <TEXT>` | — | 替换默认系统提示词 | 内置提示词 |
| `--append-system-prompt <TEXT>` | — | 追加到系统提示词末尾 | — |
| `--permission-mode <MODE>` | — | 权限模式: `default`, `auto`, `bypass` | `default` |
| `--verbose` | `-v` | 启用详细日志输出 | false |
| `--max-budget <USD>` | — | 最大消费预算 (美元) | 无限 |
| `--output-format <FMT>` | — | 输出格式: `text`, `json`, `stream-json` | text |

### 隐藏参数 (开发/调试用)

| 参数 | 说明 |
|------|------|
| `--dump-system-prompt` | 打印完整系统提示词并退出 |
| `--init-only` | 仅执行初始化然后退出 |

---

## 运行模式

### 交互式 REPL (默认)

```bash
claude-code-rs
```

- 启动后显示 `>` 提示符
- 输入文本发送给模型
- 输入斜杠命令执行本地操作
- 模型可调用工具执行文件操作、命令运行等
- 按 `Ctrl+C` 中断当前流式输出
- 按 `Ctrl+D` 或输入 `/exit` 退出

### Print 模式

```bash
echo "解释 main.rs" | claude-code-rs -p
claude-code-rs -p "列出所有 TODO"
```

- 非交互式，适合脚本和管道
- 输出纯文本到 stdout
- 错误输出到 stderr
- 返回 0 (成功) 或 1 (失败)

### 会话恢复模式

```bash
# 恢复最近的会话
claude-code-rs --resume

# 恢复指定会话
claude-code-rs --continue 550e8400-e29b-41d4-a716-446655440000
```

---

## 斜杠命令

在 REPL 中输入 `/` 前缀的命令执行本地操作。

| 命令 | 别名 | 说明 |
|------|------|------|
| `/help` | `/h`, `/?` | 列出所有可用命令 |
| `/clear` | — | 清空当前对话历史 |
| `/compact` | — | 压缩上下文以减少 token 用量 |
| `/config` | `/settings` | 查看或修改配置 |
| `/diff` | — | 显示当前 git 变更 |
| `/exit` | `/quit`, `/q` | 退出 REPL |
| `/version` | `/v` | 显示版本号 |
| `/model` | — | 查看或切换当前模型 |
| `/cost` | `/usage` | 显示当前会话的 token 用量和费用 |
| `/session` | — | 查看当前会话信息或列出已保存会话 |
| `/resume` | — | 恢复之前的会话 |
| `/files` | — | 列出对话中引用的文件 |
| `/context` | `/ctx` | 显示上下文使用信息 |
| `/permissions` | `/perms` | 查看或修改工具权限 |
| `/hooks` | — | 查看和管理工具执行钩子 |
| `/login` | — | 认证引导 (API Key / Auth Token) |
| `/logout` | — | 清除存储的认证凭据 |

### 用法示例

```
> /help
> /model claude-opus
> /cost
> /diff
> /permissions
> /login status
> /logout
```

---

## 内置工具

模型在对话中可以调用以下工具来完成任务:

### 文件操作

| 工具 | 说明 |
|------|------|
| **Read** | 读取文件内容，支持 `offset` 和 `limit` 参数按范围读取 |
| **Write** | 写入文件，会覆盖已有内容 |
| **Edit** | 文件内精确字符串替换，支持 `replace_all` 全局替换 |

### 搜索

| 工具 | 说明 |
|------|------|
| **Glob** | 文件名模式匹配 (如 `**/*.rs`)，按修改时间排序 |
| **Grep** | 正则表达式内容搜索，支持上下文行和多种输出模式 |

### 系统

| 工具 | 说明 |
|------|------|
| **Bash** | 执行 shell 命令，支持超时控制和输出捕获 |

### 交互

| 工具 | 说明 |
|------|------|
| **AskUserQuestion** | 向用户提问并等待回复 (脚手架) |
| **ToolSearch** | 按关键词搜索可用工具 (脚手架) |

### 笔记本

| 工具 | 说明 |
|------|------|
| **NotebookEdit** | 编辑 Jupyter Notebook 单元格 (脚手架) |

> **注意**: 标记为 "脚手架" 的工具已有类型定义但功能尚未完整实现。

---

## 权限系统

权限系统控制模型对工具的调用权限，保护用户免受意外操作。

### 权限模式

| 模式 | CLI 参数 | 行为 |
|------|---------|------|
| **Default** | `--permission-mode default` | 每次工具调用都需用户确认 |
| **Auto** | `--permission-mode auto` | 自动批准安全操作，危险操作仍需确认 |
| **Bypass** | `--permission-mode bypass` | 跳过所有权限检查 (危险) |
| **Plan** | `--permission-mode plan` | 只读模式，所有写操作需确认 |

### 权限决策流程

```
工具调用请求
    │
    ├─→ Phase 1a: 无条件规则 (仅匹配工具名)
    │       → always_allow / always_deny / always_ask
    │
    ├─→ Phase 1b: 模式匹配规则 (工具名 + 参数)
    │       → Bash(prefix:git)  匹配 git 开头的命令
    │       → Read(/tmp/*)      匹配 /tmp/ 下的文件路径
    │
    ├─→ Phase 2: Hook 拦截 (预留)
    │
    └─→ Phase 3: 根据当前模式做最终决策
            → Default: 询问用户
            → Auto: 自动批准 (3 次连续拒绝后回退到 Default)
            → Bypass: 直接允许
```

### 危险命令检测

Bash 工具内置 16 种危险模式检测，包括:

- `rm -rf` / `rm -r` — 递归删除
- `chmod 777` — 过于宽松的权限
- `> /dev/sda` — 磁盘覆写
- `mkfs` — 文件系统格式化
- `dd if=` — 低级磁盘操作
- `:(){ :|:& };:` — Fork 炸弹
- `curl | sh` / `wget | sh` — 远程代码执行
- `shutdown` / `reboot` — 系统关机
- `kill -9` — 强制终止进程
- 以及更多...

这些命令即使在 Auto 模式下也会触发确认提示。

### 配置权限规则

在 `settings.json` 中设置:

```json
{
  "allowed_tools": ["Read", "Glob", "Grep"],
  "hooks": {
    "before_tool_use": {}
  }
}
```

---

## 配置管理

### 配置文件位置

| 层级 | 路径 | 说明 |
|------|------|------|
| 全局 | `~/.claude/settings.json` | 用户级默认配置 |
| 项目 | `.claude/settings.json` | 项目级配置 (从 CWD 向上查找) |
| 环境变量 | — | 最高优先级覆盖 |

### 配置合并优先级

```
环境变量 > 项目配置 > 全局配置
```

### 配置文件格式

```json
{
  "model": "claude-sonnet-4-20250514",
  "theme": "dark",
  "verbose": false,
  "permission_mode": "default",
  "allowed_tools": ["Bash", "Read", "Grep"],
  "system_prompt": "你是一位 Rust 专家...",
  "hooks": {},
  "api_key": "sk-ant-..."
}
```

### 环境变量

| 变量名 | 说明 |
|--------|------|
| `ANTHROPIC_API_KEY` | API 密钥 (推荐方式) |
| `ANTHROPIC_AUTH_TOKEN` | 外部认证 Token |
| `CLAUDE_MODEL` | 覆盖默认模型 |
| `CLAUDE_VERBOSE` | 详细模式 (`true`/`1`) |
| `CLAUDE_PERMISSION_MODE` | 权限模式覆盖 |

### 认证方式

当前支持两种认证方式:

1. **API Key** (推荐): 设置 `ANTHROPIC_API_KEY` 环境变量，格式 `sk-ant-...`
2. **External Auth Token**: 设置 `ANTHROPIC_AUTH_TOKEN` 环境变量

认证优先级: API Key > External Auth Token > Keychain 存储

```bash
# 方式 1: API Key
export ANTHROPIC_API_KEY="sk-ant-api03-..."

# 方式 2: External Auth Token
export ANTHROPIC_AUTH_TOKEN="your-token"

# 查看认证状态
> /login status

# 清除存储的凭据
> /logout
```

> **注**: OAuth 登录流程已定义接口但尚未实现。

### 通过命令修改

```
> /config
> /model claude-opus
```

---

## 会话管理

### 存储位置

```
~/.claude/sessions/{session_id}.json
```

每个会话以 UUID 命名，存储为 JSON 文件。

### 会话文件结构

```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "created_at": 1704067200,
  "last_modified": 1704067200,
  "cwd": "/path/to/project",
  "messages": [
    {
      "type": "user",
      "uuid": "...",
      "timestamp": 1704067200,
      "data": { "content": "...", "is_meta": false }
    }
  ]
}
```

### 对话记录 (Transcript)

除了会话 JSON，系统还以 NDJSON 格式追加写入对话记录:

```
~/.claude/sessions/{session_id}.ndjson
```

每行一条消息，支持增量同步，不需要重写整个文件。

### 会话操作

```bash
# 恢复最近的会话
claude-code-rs --resume

# 恢复指定会话
claude-code-rs --continue <session-id>

# REPL 中查看会话
> /session

# REPL 中恢复会话
> /resume
```

会话按 `last_modified` 降序排列，恢复时自动匹配当前工作目录。

---

## CLAUDE.md 项目指令

在项目中放置 `CLAUDE.md` 文件，可以为模型注入自定义指令。

### 查找规则

系统从当前工作目录开始，向上遍历所有祖先目录，收集找到的所有 `CLAUDE.md` 文件。

```
/home/user/projects/myapp/src/   ← CWD
/home/user/projects/myapp/CLAUDE.md   ✓ 读取
/home/user/projects/CLAUDE.md         ✓ 读取
/home/user/CLAUDE.md                  ✓ (如果存在)
```

### 加载顺序

**根目录优先**: 最靠近文件系统根的文件最先加载，最靠近 CWD 的文件最后加载。后加载的指令可以覆盖先前的通用指令。

### 注入格式

每个文件的内容会被标注来源路径后注入系统提示词:

```
Contents of /path/to/CLAUDE.md (project instructions, checked into the codebase):

[文件内容]
```

### 用途建议

```markdown
# CLAUDE.md

## 项目概述
这是一个 Rust CLI 工具...

## 编码规范
- 使用 snake_case
- 错误处理统一使用 anyhow::Result

## 构建命令
cargo build --release
cargo test
```

---

## 上下文压缩

长对话会逐渐消耗 token 预算。上下文压缩系统通过多级策略自动管理。

### 压缩管线

```
snip (历史裁剪)  →  microcompact (微压缩)  →  autocompact (自动全量压缩)
```

| 阶段 | 触发条件 | 行为 |
|------|---------|------|
| **Snip** | 对话轮数过多 | 移除最早的对话轮，保留边界消息 |
| **Microcompact** | 工具输出超过阈值 | 裁剪大块工具输出，保护最近 N 条结果 |
| **Autocompact** | token 用量达到预算 80% | 调用 API 生成摘要替换历史 (需网络) |

### 手动压缩

```
> /compact
```

### 消息规范化

发送 API 请求前，消息会经过规范化处理:
- 确保 user/assistant 交替顺序
- 合并连续的同类型消息
- 移除空消息

---

## 键盘快捷键

### REPL 快捷键

| 快捷键 | 场景 | 说明 |
|--------|------|------|
| `Ctrl+C` | 流式输出中 | 中断当前回复 |
| `Ctrl+C` | 空闲时 | 退出程序 |
| `Ctrl+D` | 任何时候 | 退出程序 (EOF) |
| `Page Up` / `Shift+Up` | 任何时候 | 向上滚动 5 行 |
| `Page Down` / `Shift+Down` | 任何时候 | 向下滚动 5 行 |
| `Ctrl+U` | 非输入模式 | 向上滚动 10 行 |
| `Ctrl+D` | 流式输出中 | 向下滚动 10 行 |

### 输入编辑

| 快捷键 | 说明 |
|--------|------|
| `Ctrl+A` | 光标移到行首 |
| `Ctrl+U` | 删除光标前的所有文本 |
| `Ctrl+W` | 删除光标前的一个词 |

---

## 构建说明

默认构建即为全功能构建，包含以下所有模块:

| 模块 | 说明 | 依赖 |
|------|------|------|
| 网络 | HTTP 请求、SSE 流、WebSocket | reqwest, eventsource-stream, tokio-tungstenite |
| API | Anthropic / OpenAI / Google / Bedrock / Vertex | 多提供商支持 |
| 认证 | 系统 Keychain / OAuth | keyring, jsonwebtoken, oauth2 |
| MCP | Model Context Protocol | 网络模块 |
| LSP | 代码导航集成 | lsp-types |
| AST | Tree-sitter 解析 | tree-sitter |

```bash
cargo build --release
```

---

## 当前限制

本项目处于活跃开发阶段 (v0.1.0)，以下功能尚未完全实现:

### 功能缺口

| 功能 | 状态 | 说明 |
|------|------|------|
| Agent 工具 | 脚手架 | 子 QueryEngine 派生未实现 |
| Hooks 执行 | 脚手架 | 类型定义完整，子进程调用未实现 |
| AskUser 交互 | 脚手架 | 无真实终端集成 |
| 命令系统 | 部分 | 17/85+ 命令 |
| 插件/技能系统 | 缺失 | 未开始 |
| Vim 模式 | 缺失 | 未开始 |

### 已完整可用的功能

- 核心查询状态机 + 流式处理
- 文件读写编辑 (Read/Write/Edit)
- 文件搜索 (Glob/Grep)
- Shell 命令执行 (Bash)
- 认证: API Key + External Auth Token + `/login` + `/logout`
- 权限系统 (4 种模式 + 危险检测)
- 终端 UI (ratatui 渲染)
- 会话持久化与恢复
- 上下文压缩管线
- CLAUDE.md 指令注入
- 配置管理 (3 层合并)

---

## 故障排查

### 编译错误

```bash
# 清理并重新编译
cargo clean && cargo build

# 查看详细编译输出
cargo build -vv
```

### 运行时日志

```bash
# 启用详细日志
claude-code-rs -v

# 使用环境变量控制日志级别
RUST_LOG=debug claude-code-rs
RUST_LOG=claude_code_rs=trace claude-code-rs
```

### 会话数据

```bash
# 查看会话文件
ls ~/.claude/sessions/

# 查看配置
cat ~/.claude/settings.json
```

### 系统提示词调试

```bash
# 导出完整系统提示词
claude-code-rs --dump-system-prompt
```

### 常见问题

**Q: 启动后没有任何输出**
A: 默认模式需要 API 连接 (尚未完成)。当前版本主要可用于本地工具测试和 UI 开发。

**Q: 编译时间很长**
A: 首次编译需要下载和编译所有依赖。后续增量编译会快很多。使用 `cargo build` (不加 `--release`) 开发时编译更快。

**Q: Windows 上 git2 编译失败**
A: 确保安装了 Visual Studio C++ Build Tools 和 cmake。也可以尝试: `set LIBGIT2_NO_VENDOR=1`

**Q: 权限模式没有生效**
A: 检查优先级: CLI 参数 > 环境变量 > 项目配置 > 全局配置。使用 `--verbose` 查看加载了哪些配置。
