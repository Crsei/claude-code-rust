# Claude Code Rust Lite — 使用指南

> 版本: 0.1.0-lite | 最后更新: 2026-04-03

---

## 目录

1. [构建与安装](#构建与安装)
2. [快速开始](#快速开始)
3. [命令行参数](#命令行参数)
4. [运行模式](#运行模式)
5. [斜杠命令](#斜杠命令)
6. [内置工具](#内置工具)
7. [Skills 技能系统](#skills-技能系统)
8. [权限系统](#权限系统)
9. [配置管理](#配置管理)
10. [会话管理](#会话管理)
11. [CLAUDE.md 项目指令](#claudemd-项目指令)
12. [键盘快捷键](#键盘快捷键)
13. [故障排查](#故障排查)

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

### 运行测试

```bash
cargo test
```

---

## 快速开始

### 1. 认证

```bash
# 设置 API Key (推荐)
export ANTHROPIC_API_KEY="sk-ant-api03-..."

# 或在 REPL 中登录
> /login sk-ant-api03-...

# 查看认证状态
> /login status
```

### 2. 启动交互式 REPL

```bash
claude-code-rs
```

进入交互模式后，输入自然语言指令或斜杠命令即可。输入 `/exit` 或按 `Ctrl+D` 退出。

### 3. 单次查询 (Print 模式)

```bash
claude-code-rs -p "解释这段代码的作用"
```

输出模型回复后立即退出，适合管道和脚本集成。

### 4. 带初始提示启动

```bash
claude-code-rs "帮我重构这个函数"
```

启动 REPL 并自动发送第一条消息。

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

### 交互式 TUI (默认)

```bash
claude-code-rs
```

启动后进入全屏终端界面 (基于 ratatui + crossterm)：

- 顶部显示欢迎信息 (版本、模型、会话 ID)
- 消息区域可滚动浏览对话历史
- 底部输入框输入文本发送给模型
- 状态栏显示模型名称、消息数、费用、当前状态
- 输入斜杠命令执行本地操作
- 模型可调用工具执行文件操作、命令运行等
- 流式输出期间显示 Braille 点阵旋转动画
- 按 `Ctrl+C` 中断当前流式输出
- 按 `Ctrl+D` 退出

### Print 模式

```bash
echo "解释 main.rs" | claude-code-rs -p
claude-code-rs -p "列出所有 TODO"
```

- 非交互式，适合脚本和管道
- 输出纯文本到 stdout
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

### 基础

| 命令 | 别名 | 说明 |
|------|------|------|
| `/help` | `/h`, `/?` | 列出所有可用命令 |
| `/clear` | — | 清空当前对话历史 |
| `/exit` | `/quit`, `/q` | 退出 REPL |
| `/version` | `/v` | 显示版本号 |

### 模型控制

| 命令 | 别名 | 说明 |
|------|------|------|
| `/model` | — | 查看或切换当前模型 |
| `/cost` | `/usage` | 显示 token 用量和费用 |
| `/fast` | — | 切换快速模式 |
| `/effort` | — | 设置 thinking 深度 (low/medium/high) |

### 配置 & 权限

| 命令 | 别名 | 说明 |
|------|------|------|
| `/config` | `/settings` | 查看或修改配置 |
| `/permissions` | `/perms` | 查看或修改工具权限 |
| `/init` | — | 初始化项目配置 (`.cc-rust/settings.json`) |

### 认证

| 命令 | 说明 |
|------|------|
| `/login` | 认证引导 (API Key / Auth Token) |
| `/logout` | 清除存储的认证凭据 |

### Git

| 命令 | 别名 | 说明 |
|------|------|------|
| `/commit` | — | 创建 git commit |
| `/diff` | — | 显示当前 git 变更 |
| `/branch` | `/br` | 查看或切换分支 |

### 会话 & 上下文

| 命令 | 别名 | 说明 |
|------|------|------|
| `/session` | — | 查看会话信息或列出已保存会话 |
| `/resume` | — | 恢复之前的会话 |
| `/context` | `/ctx` | 显示上下文使用信息 |
| `/files` | — | 列出对话中引用的文件 |
| `/status` | — | 查看会话状态 |
| `/copy` | `/cp` | 复制最后回复到剪贴板 |

### 技能 & 记忆

| 命令 | 别名 | 说明 |
|------|------|------|
| `/skills` | — | 列出可用技能 |
| `/memory` | `/mem` | 查看和管理 CLAUDE.md 项目指令 |

---

## 内置工具

模型在对话中可以调用以下 8 个工具来完成任务:

### 文件操作

| 工具 | 说明 |
|------|------|
| **Read** | 读取文件内容，支持 `offset` 和 `limit` 参数按范围读取 |
| **Write** | 写入文件，会覆盖已有内容 |
| **Edit** | 文件内精确字符串替换，支持 fuzzy 匹配和 `replace_all` 全局替换 |

### 搜索

| 工具 | 说明 |
|------|------|
| **Glob** | 文件名模式匹配 (如 `**/*.rs`)，按修改时间排序 |
| **Grep** | 正则表达式内容搜索，支持上下文行和多种输出模式 |

### 系统

| 工具 | 说明 |
|------|------|
| **Bash** | 执行 shell 命令，支持超时控制和输出截断 (head 200 + tail 100) |

### 交互

| 工具 | 说明 |
|------|------|
| **AskUser** | 向用户提问并等待回复 |

### 技能

| 工具 | 说明 |
|------|------|
| **Skill** | 调用内置或用户自定义技能 |

---

## Skills 技能系统

Skills 是可复用的提示词模板，通过 Skill 工具调用。技能系统是 Lite 版保留的核心扩展机制。

### 技能来源

| 来源 | 路径 | 说明 |
|------|------|------|
| 内置 | 程序打包 | 随程序发布的预定义技能 |
| 全局 | `~/.cc-rust/skills/*.md` | 用户级自定义技能 |
| 项目 | `.cc-rust/skills/*.md` | 项目级自定义技能 |

### 技能文件格式

```markdown
---
name: my-skill
description: 一句话说明这个技能的用途
---

技能的提示词内容写在这里。

支持的变量替换:
- $ARGUMENTS — 调用时传入的参数
- ${ARG_NAME} — 命名参数
- ${CLAUDE_SKILL_DIR} — 技能文件所在目录
- ${CLAUDE_SESSION_ID} — 当前会话 ID
```

### 使用技能

技能由模型通过 Skill 工具自动调用，也可以通过 `/skills` 命令查看所有可用技能。

### 创建自定义技能

1. 创建技能目录: `mkdir -p .cc-rust/skills/`
2. 创建技能文件: `.cc-rust/skills/my-skill.md`
3. 编写 frontmatter (name, description) 和提示词内容
4. 重启后即可使用

示例 — 代码审查技能:

```markdown
---
name: review
description: 对指定文件进行代码审查
---

请对以下文件进行代码审查，关注:
1. 潜在 bug
2. 性能问题
3. 代码风格

文件: $ARGUMENTS
```

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

### 危险命令检测

Bash 工具内置 16 种危险模式检测，包括:

- `rm -rf` / `rm -r` — 递归删除
- `chmod 777` — 过于宽松的权限
- `> /dev/sda` — 磁盘覆写
- `mkfs` — 文件系统格式化
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
| 全局 | `~/.cc-rust/settings.json` | 用户级默认配置 |
| 项目 | `.cc-rust/settings.json` | 项目级配置 (从 CWD 向上查找) |
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

---

## 会话管理

### 存储位置

```
~/.cc-rust/sessions/{session_id}.json
```

每个会话以 UUID 命名，存储为 JSON 文件。

### 对话记录 (Transcript)

除了会话 JSON，系统还以 NDJSON 格式追加写入对话记录:

```
~/.cc-rust/sessions/{session_id}.ndjson
```

### 会话操作

```bash
# 恢复最近的会话
claude-code-rs --resume

# 恢复指定会话
claude-code-rs --continue <session-id>

# REPL 中查看/恢复会话
> /session
> /resume
```

---

## CLAUDE.md 项目指令

在项目中放置 `CLAUDE.md` 文件，可以为模型注入自定义指令。

### 查找规则

系统从当前工作目录开始，向上遍历所有祖先目录，收集找到的所有 `CLAUDE.md` 文件。根目录优先加载，CWD 最近的文件最后加载 (可覆盖通用指令)。

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

## 键盘快捷键

### 全局

| 快捷键 | 场景 | 说明 |
|--------|------|------|
| `Ctrl+C` | 流式输出中 | 中断当前回复 |
| `Ctrl+C` | 空闲时 | 退出程序 |
| `Ctrl+D` | 任何时候 | 退出程序 (EOF) |
| `Page Up` / `Shift+Up` | 任何时候 | 向上滚动 |
| `Page Down` / `Shift+Down` | 任何时候 | 向下滚动 |

### 输入编辑

| 快捷键 | 说明 |
|--------|------|
| `Enter` | 提交输入 |
| `Up` / `Down` | 浏览输入历史 |
| `Left` / `Right` | 移动光标 |
| `Home` / `End` | 光标移到行首/行尾 |
| `Ctrl+A` / `Ctrl+E` | 行首/行尾 |
| `Ctrl+U` | 清除整行 |
| `Ctrl+W` | 删除光标前一个词 |
| `Ctrl+K` | 删除光标到行尾 |

### 权限对话框

| 快捷键 | 说明 |
|--------|------|
| `Y` / `N` | 允许 / 拒绝 |
| `A` | 始终允许 |
| `Enter` | 确认当前选项 |
| `Esc` | 拒绝 |

---

## 故障排查

### 编译问题

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

# 环境变量控制日志级别
RUST_LOG=debug claude-code-rs
```

### 数据文件

```bash
# 会话文件
ls ~/.cc-rust/sessions/

# 配置文件
cat ~/.cc-rust/settings.json

# 技能文件
ls ~/.cc-rust/skills/
```

### 系统提示词调试

```bash
claude-code-rs --dump-system-prompt
```

### 常见问题

**Q: Windows 上 git2 编译失败**
A: 确保安装了 Visual Studio C++ Build Tools 和 cmake。也可以尝试: `set LIBGIT2_NO_VENDOR=1`

**Q: 权限模式没有生效**
A: 检查优先级: CLI 参数 > 环境变量 > 项目配置 > 全局配置。使用 `--verbose` 查看配置来源。

**Q: 编译时间很长**
A: 首次编译需下载所有依赖。后续增量编译会快很多。开发时使用 `cargo build` (不加 `--release`)。
