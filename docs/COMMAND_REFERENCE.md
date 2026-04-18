# cc-rust Slash Command Reference

> 本文按当前源码实现整理：
> - 命令注册表：`src/commands/mod.rs`
> - 每个命令的参数解析：`src/commands/*.rs`
>
> 与旧文档不同，这里优先描述“当前代码实际支持什么”，而不是历史设计目标。

## 约定

- 命令格式：`/command [args...]`
- 别名和主命令等价，例如 `/help`、`/h`、`/?`
- 若某命令没有写明子参数，表示当前实现只支持“无参数”或“把整段参数当自由文本”
- KAIROS / proactive / notification 相关命令受 feature gate 控制；未启用时会直接返回提示文本

## Core Commands

### `/help`

- Aliases: `/h`, `/?`
- Syntax:
  - `/help`
  - `/help <command-or-alias>`
- Behavior:
  - 无参数时列出所有已注册命令
  - 传命令名或别名时，显示该命令的一行说明和别名
- Examples:
  - `/help`
  - `/help model`
  - `/help q`

### `/clear`

- Aliases: none
- Syntax: `/clear`
- Behavior:
  - 返回 `CommandResult::Clear`，由外层 REPL 清空当前会话消息
- Examples:
  - `/clear`

### `/exit`

- Aliases: `/quit`, `/q`
- Syntax: `/exit`
- Behavior:
  - 退出 REPL
- Examples:
  - `/exit`
  - `/q`

### `/version`

- Aliases: `/v`
- Syntax: `/version`
- Behavior:
  - 输出当前 `claude-code-rs` 版本
- Examples:
  - `/version`
  - `/v`

### `/status`

- Aliases: none
- Syntax: `/status`
- Behavior:
  - 输出当前会话状态摘要：消息数、模型、fast mode、effort、permission mode
- Examples:
  - `/status`

## Model / Config / Permissions

### `/config`

- Aliases: `/settings`
- Syntax:
  - `/config`
  - `/config show`
  - `/config set <key> <value>`
  - `/config reset`
- Supported keys for `set`:
  - `model`
  - `backend`
  - `theme`
  - `verbose`
- Notes:
  - `/config` 默认等价于 `/config show`
  - `backend` 会走规范化逻辑：`codex` 保留为 `codex`，其他值会回退成 `native`
- Examples:
  - `/config`
  - `/config set model claude-sonnet-4-20250514`
  - `/config set backend codex`
  - `/config set verbose true`
  - `/config reset`

### `/model`

- Aliases: none
- Syntax:
  - `/model`
  - `/model <name-or-alias>`
- Supported aliases:
  - `opus` → `claude-opus-4-20250514`
  - `sonnet` → `claude-sonnet-4-20250514`
  - `haiku` → `claude-haiku-3-5-20241022`
- Notes:
  - 任意未知名字都会被当成原始模型字符串直接写入，不做校验
- Examples:
  - `/model`
  - `/model opus`
  - `/model gpt-5.4`

### `/model-add`

- Aliases: `/ma`
- Syntax:
  - `/model-add <name>`
  - `/model-add <name> <input_price> <output_price>`
- Behavior:
  - 把 `CLAUDE_MODEL`、`MODEL_INPUT_PRICE`、`MODEL_OUTPUT_PRICE` 写入当前工作目录的 `.env`
  - 同时更新当前进程环境变量与当前 `app_state.main_loop_model`
- Notes:
  - 价格单位是 USD / 1M tokens
  - 只给模型名时，会尝试查内置 pricing 表；查不到会报错并要求显式传价格
- Examples:
  - `/model-add gpt-4o`
  - `/model-add my-model 1.5 7.0`

### `/cost`

- Aliases: `/usage`
- Syntax: `/cost`
- Behavior:
  - 聚合当前会话 assistant 消息里的 usage / cost 字段
- Examples:
  - `/cost`
  - `/usage`

### `/extra-usage`

- Aliases: `/eu`
- Syntax: `/extra-usage`
- Behavior:
  - 输出扩展用量分析：单次调用拆分、最贵调用 Top 5、缓存命中率、估算成本拆分
- Notes:
  - 当前实现不解析任何子参数
- Examples:
  - `/extra-usage`
  - `/eu`

### `/rate-limit-options`

- Aliases: `/rlo`, `/rate-limit`
- Syntax: `/rate-limit-options`
- Behavior:
  - 显示当前模型命中的速率限制参考表、当前会话里出现的 rate-limit 错误次数和建议
- Notes:
  - 当前实现不解析任何子参数
- Examples:
  - `/rate-limit-options`
  - `/rate-limit`

### `/effort`

- Aliases: none
- Syntax:
  - `/effort`
  - `/effort <low|medium|high>`
- Behavior:
  - 无参数时显示当前 effort
  - 有参数时只接受 `low` / `medium` / `high`
- Notes:
  - 顶部注释提到“numeric budget token count”，但当前代码并不支持数字参数
- Examples:
  - `/effort`
  - `/effort low`
  - `/effort high`

### `/fast`

- Aliases: none
- Syntax:
  - `/fast`
  - `/fast on`
  - `/fast off`
  - `/fast status`
- Supported synonyms:
  - `on` / `enable`
  - `off` / `disable`
- Behavior:
  - 无参数时切换 fast mode
  - 启用时如果当前模型不兼容，会自动切到 `claude-opus-4-6-20250414`
- Examples:
  - `/fast`
  - `/fast on`
  - `/fast off`
  - `/fast status`

### `/permissions`

- Aliases: `/perms`
- Syntax:
  - `/permissions`
  - `/permissions mode <mode>`
  - `/permissions allow <tool>`
  - `/permissions deny <tool>`
  - `/permissions reset`
- Supported modes:
  - `default`
  - `auto`
  - `bypass`
  - `plan`
- Mode aliases:
  - `ask` → `default`
  - `readonly` → `plan`
- Notes:
  - `bypass` 还会检查 `is_bypass_permissions_mode_available`
  - `allow` / `deny` 当前只接收一个工具名 token
- Examples:
  - `/permissions`
  - `/permissions mode auto`
  - `/permissions mode readonly`
  - `/permissions allow Bash`
  - `/permissions deny Write`
  - `/permissions reset`

## Session / Context / Workspace

### `/session`

- Aliases: none
- Syntax:
  - `/session`
  - `/session list`
  - `/session list all`
- Supported aliases:
  - `list` / `ls`
- Behavior:
  - 无参数时显示当前 session 信息和最近的工作区 session
  - `list` 只列当前 workspace
  - `list all` 列所有 workspace
- Examples:
  - `/session`
  - `/session list`
  - `/session ls all`

### `/resume`

- Aliases: none
- Syntax:
  - `/resume`
  - `/resume <session-id-or-prefix>`
- Behavior:
  - 无参数时恢复当前工作区最近的一次历史会话
  - 传 ID 时先尝试当前 workspace，再尝试全局；支持前缀匹配
- Notes:
  - 如果前缀命中多个 session，会返回候选列表让你继续缩小范围
- Examples:
  - `/resume`
  - `/resume 550e8400`

### `/context`

- Aliases: `/ctx`
- Syntax: `/context`
- Behavior:
  - 输出本地估算的上下文 token 使用情况和消息分布
- Notes:
  - 这里是本地启发式估算，不是精确 token 数
- Examples:
  - `/context`
  - `/ctx`

### `/compact`

- Aliases: none
- Syntax:
  - `/compact`
  - `/compact <free-text-instructions>`
- Behavior:
  - 尝试对当前对话做本地压缩
  - 若给了参数，会把整段参数当“压缩指令”文本
- Notes:
  - 这不是子命令接口，参数是自由文本
  - 当前主要走本地 compaction pipeline，不是远程总结
- Examples:
  - `/compact`
  - `/compact focus on code changes only`

### `/files`

- Aliases: none
- Syntax: `/files`
- Behavior:
  - 从消息和工具调用里提取当前上下文中引用过的文件路径
- Examples:
  - `/files`

### `/copy`

- Aliases: `/cp`
- Syntax: `/copy`
- Behavior:
  - 提取最后一条 assistant 纯文本消息并返回“Copied to clipboard …”提示
- Notes:
  - 当前实现并没有真正写系统剪贴板，而是把要复制的文本回显出来
- Examples:
  - `/copy`
  - `/cp`

### `/add-dir`

- Aliases: none
- Syntax:
  - `/add-dir`
  - `/add-dir <path>`
- Behavior:
  - 无参数时列出当前 session 已添加的额外工作目录
  - 有参数时解析路径、做 canonicalize，然后加入 `additional_working_directories`
- Notes:
  - 支持相对路径和 `~`
  - 如果目标目录已经在当前工作目录之内，或已经添加过，会直接提示
  - 当前新增目录默认不是只读
- Examples:
  - `/add-dir`
  - `/add-dir ..\\shared-lib`
  - `/add-dir ~/projects/other-repo`

### `/init`

- Aliases: none
- Syntax: `/init`
- Behavior:
  - 在当前工作目录下创建 `.cc-rust/settings.json`
- Notes:
  - 如果已存在，只会提示现有路径，不会覆盖
- Examples:
  - `/init`

### `/memory`

- Aliases: `/mem`
- Syntax:
  - `/memory`
  - `/memory show`
  - `/memory path`
  - `/memory edit`
  - `/memory list`
  - `/memory get <key>`
  - `/memory set <key> <value> [--global] [--category=<cat>]`
  - `/memory rm <key> [--global]`
  - `/memory search <query>`
- Supported aliases:
  - `list` / `ls`
  - `rm` / `delete` / `del`
  - `search` / `find`
- Behavior:
  - `show`：显示当前 `CLAUDE.md` 汇总内容，且是默认行为
  - `path`：列出找到的 `CLAUDE.md`
  - `edit`：在当前目录创建或定位 `CLAUDE.md`
  - `list/get/set/rm/search`：操作 memdir 记忆项
- Notes:
  - `set` 支持 `--global` 和 `--category=<cat>`
  - `rm` 支持 `--global`
  - 当前 `search` 查询在实现上更适合单 token；多词查询不会完整保留
- Examples:
  - `/memory`
  - `/memory path`
  - `/memory edit`
  - `/memory list`
  - `/memory get style`
  - `/memory set style use_rustfmt --category=code`
  - `/memory set api_base https://example.com --global`
  - `/memory rm api_base --global`
  - `/memory search rustfmt`

### `/skills`

- Aliases: none
- Syntax:
  - `/skills`
  - `/skills list`
  - `/skills <skill-name>`
- Behavior:
  - 无参数或 `list` 时列出所有已加载技能
  - 传技能名时显示该技能的详细信息
- Examples:
  - `/skills`
  - `/skills list`
  - `/skills review`

## Auth Commands

### `/login`

- Aliases: none
- Syntax:
  - `/login`
  - `/login status`
  - `/login sk-ant-...`
  - `/login 1`
  - `/login 2`
  - `/login 3`
  - `/login 4`
  - `/login 5`
  - `/login codex`
  - `/login codex-cli`
- Meaning of numbered entries:
  - `1`：手动粘贴 Anthropic API Key
  - `2`：Claude.ai OAuth
  - `3`：Console OAuth
  - `4` / `codex`：OpenAI Codex OAuth
  - `5` / `codex-cli`：检查并尝试导入 / 刷新 `~/.codex/auth.json`
- Examples:
  - `/login`
  - `/login status`
  - `/login sk-ant-api03-...`
  - `/login 4`
  - `/login codex-cli`

### `/login-code`

- Aliases: none
- Syntax:
  - `/login-code <authorization-code>`
  - `/login-code <redirect-url-containing-code>`
- Behavior:
  - 完成由 `/login 2`、`/login 3`、`/login 4` 发起的 OAuth 流程
- Notes:
  - 如果没有 pending OAuth state，会直接提示先跑 `/login 2/3/4`
  - 如果传的是整条回调 URL，会自动尝试提取 `code=` 查询参数
- Examples:
  - `/login-code eyJhbGciOi...`
  - `/login-code https://example/callback?code=abc123&state=xyz`

### `/logout`

- Aliases: none
- Syntax: `/logout`
- Behavior:
  - 清理 keychain 与 `~/.cc-rust/credentials.json`
- Notes:
  - 环境变量里的 token / API key 不会自动 unset
- Examples:
  - `/logout`

## Git / Export Commands

### `/diff`

- Aliases: none
- Syntax:
  - `/diff`
  - `/diff --staged`
  - `/diff --cached`
- Behavior:
  - 默认同时显示 staged 和 unstaged diff
  - `--staged` / `--cached` 只显示 staged diff
- Examples:
  - `/diff`
  - `/diff --staged`

### `/branch`

- Aliases: `/br`
- Syntax:
  - `/branch`
  - `/branch <branch-name>`
- Behavior:
  - 无参数时列出本地分支并标记当前分支
  - 有参数时先尝试 `git checkout <name>`，失败后再尝试 `git checkout -b <name>`
- Examples:
  - `/branch`
  - `/br feature/docs`

### `/commit`

- Aliases: none
- Syntax:
  - `/commit`
  - `/commit <message>`
- Behavior:
  - 无参数时不会直接提交，而是生成一条 `Query` 消息，让模型帮你审查变更并创建提交
  - 有参数时执行 `git commit -m "<message>"`
- Notes:
  - 当前实现不会自动 `git add`
  - 注释写了 “Stages all changes”，但代码实际没有做 stage
- Examples:
  - `/commit`
  - `/commit docs: add command reference`

### `/export`

- Aliases: none
- Syntax:
  - `/export`
  - `/export list`
  - `/export <path>`
  - `/export <session-id-or-prefix>`
- Behavior:
  - 无参数：导出当前 session 到默认导出目录
  - `list`：列出已有 Markdown 导出
  - 带 `/`、`\\` 或 `.md` 结尾的参数：当成路径
  - 其他参数：当成 session id / prefix
- Examples:
  - `/export`
  - `/export list`
  - `/export notes/session.md`
  - `/export 550e8400`

### `/audit-export`

- Aliases: `/audit`
- Syntax:
  - `/audit-export`
  - `/audit-export list`
  - `/audit-export verify <path>`
  - `/audit-export <path>`
  - `/audit-export <session-id-or-prefix>`
- Behavior:
  - 导出可校验 audit record，或校验已有 audit 文件
  - 路径识别规则：包含 `/`、`\\` 或 `.json` 后缀
- Examples:
  - `/audit-export`
  - `/audit-export list`
  - `/audit-export verify ~/.cc-rust/audits/run.audit.json`
  - `/audit-export artifacts/run.audit.json`
  - `/audit 550e8400`

### `/session-export`

- Aliases: `/sexport`
- Syntax:
  - `/session-export`
  - `/session-export list`
  - `/session-export <path>`
  - `/session-export <session-id-or-prefix>`
- Behavior:
  - 导出结构化 session JSON 包
  - 路径识别规则：包含 `/`、`\\` 或 `.json` 后缀
- Examples:
  - `/session-export`
  - `/session-export list`
  - `/session-export artifacts/session.json`
  - `/sexport 550e8400`

## MCP / Plugin Commands

### `/mcp`

- Aliases: none
- Syntax:
  - `/mcp`
  - `/mcp list`
  - `/mcp ls`
  - `/mcp status`
- Behavior:
  - 无参数时显示帮助和 `mcpServers` 配置示例
  - `list`：列出当前发现到的 MCP servers
  - `status`：输出 discovery 视图，运行态提示转到 SystemStatus / headless IPC
- Examples:
  - `/mcp`
  - `/mcp list`
  - `/mcp status`

### `/plugin`

- Aliases: none
- Syntax:
  - `/plugin`
  - `/plugin list`
  - `/plugin ls`
  - `/plugin status`
  - `/plugin enable <plugin-id>`
  - `/plugin disable <plugin-id>`
- Behavior:
  - 无参数时显示帮助
  - `enable/disable` 会改 `~/.cc-rust/plugins/installed_plugins.json`
- Notes:
  - `enable` / `disable` 缺少 plugin id 时会返回真正的错误，而不是普通文本输出
- Examples:
  - `/plugin`
  - `/plugin list`
  - `/plugin status`
  - `/plugin enable github`
  - `/plugin disable github`

## KAIROS / Assistant Commands

### `/brief`

- Aliases: none
- Syntax:
  - `/brief`
  - `/brief on`
  - `/brief off`
  - `/brief status`
- Supported synonyms:
  - `on` / `enable`
  - `off` / `disable`
- Behavior:
  - 无参数时切换 brief mode
- Feature gate:
  - 需要 `FEATURE_KAIROS_BRIEF=1`
- Examples:
  - `/brief`
  - `/brief on`
  - `/brief status`

### `/sleep`

- Aliases: none
- Syntax:
  - `/sleep`
  - `/sleep <seconds>`
- Behavior:
  - 无参数时显示用法和当前 tick interval
  - 参数必须是 `1..3600` 范围内的整数秒
- Feature gate:
  - 需要 `FEATURE_PROACTIVE=1`
- Examples:
  - `/sleep`
  - `/sleep 60`
  - `/sleep 300`

### `/assistant`

- Aliases: `/kairos`
- Syntax: `/assistant`
- Behavior:
  - 显示 KAIROS / assistant mode 状态
- Notes:
  - 当前实现不解析子参数；任何额外参数都会被忽略
- Feature gate:
  - 需要 `FEATURE_KAIROS=1`
- Examples:
  - `/assistant`
  - `/kairos`

### `/daemon`

- Aliases: none
- Syntax:
  - `/daemon`
  - `/daemon status`
  - `/daemon stop`
- Behavior:
  - 无参数默认等价于 `status`
  - `stop` 目前只是返回“stop requested”提示
- Feature gate:
  - 需要 `FEATURE_KAIROS=1`
- Examples:
  - `/daemon`
  - `/daemon status`
  - `/daemon stop`

### `/notify`

- Aliases: none
- Syntax:
  - `/notify`
  - `/notify status`
  - `/notify test`
  - `/notify on`
  - `/notify off`
- Behavior:
  - 无参数默认等价于 `status`
  - 当前实现主要返回状态 / 提示文本
- Feature gate:
  - 需要 `FEATURE_KAIROS_PUSH_NOTIFICATION=1`
- Examples:
  - `/notify`
  - `/notify test`
  - `/notify on`

### `/channels`

- Aliases: none
- Syntax:
  - `/channels`
  - `/channels list`
  - `/channels status`
- Behavior:
  - 无参数默认等价于 `list`
  - 当前实现只返回占位状态文本
- Feature gate:
  - 需要 `FEATURE_KAIROS_CHANNELS=1`
- Examples:
  - `/channels`
  - `/channels list`
  - `/channels status`

### `/dream`

- Aliases: none
- Syntax:
  - `/dream`
  - `/dream --days <N>`
  - `/dream help`
  - `/dream --help`
- Behavior:
  - 无参数默认蒸馏最近 7 天日志
  - `--days N` 要求 `N > 0`
- Feature gate:
  - 需要 `FEATURE_KAIROS=1`
- Examples:
  - `/dream`
  - `/dream --days 14`
  - `/dream help`

## Source Notes

一些命令的“文案目标”和“代码行为”目前有差异，写文档时已按代码落地行为处理：

- `/commit`：当前不会自动 stage，只在有 message 时跑 `git commit -m`
- `/effort`：当前只支持 `low|medium|high`
- `/copy`：当前不真正写系统剪贴板
- `/notify`、`/channels`、`/daemon stop`：目前偏状态/占位接口
- `/mcp status`：当前偏 discovery 视图，不是实时连接面板

如果后续继续补文档，下一步适合增加：

- 每个命令的失败示例
- 每个命令对应源码路径
- 哪些命令会返回 `Clear` / `Exit` / `Query` 而不是普通文本
