# 建议系统与输入处理移植缺口

对应: `claude-code-bun/src/utils/suggestions/` + `processUserInput/`

Rust 对应: **部分落地，分散在 crates 中；尚无独立 `cc-suggestions` / `cc-completions` crate**
最近核查: 2026-05-18

## 结论

旧结论“完全缺失”已过时。Rust crates 已经应用了部分相关能力：

- 命令解析与执行已落在 `cc-commands`、`cc-engine::input_processing`、Rust TUI、headless ingress。
- Rust TUI 已有 `/` 命令面板、简单模糊匹配、参数提示、Ctrl+R 历史搜索、下一步 prompt suggestion 渲染。
- headless 协议已经有 `suggestions` 和 `search_files` 消息，`cc-services` 有 prompt suggestion 与文件内容搜索。
- 技能系统已迁入 `cc-skills`，支持 frontmatter、user/model invocable、allowed tools、Skill tool 和 `/skills` 管理。

但 Bun 的 `suggestions/**` 与 `processUserInput/**` 不是完整移植。主要缺口仍是：输入框级自动补全统一管线、路径/目录补全、`!` shell 历史幽灵补全、Slack `#channel` 补全、技能使用频率排序，以及 Bun 中把图片/IDE 选择/附件/Hook/Telemetry/命令路由合并在一起的中央输入处理器。

## Bun 端规模

### suggestions/

| 文件 | 行数 | 说明 |
|------|------|------|
| `commandSuggestions.ts` | 576 | Fuse.js 命令补全、alias、mid-input slash、hidden exact、技能使用排序 |
| `directoryCompletion.ts` | 263 | LRU 缓存的目录/文件路径补全 |
| `shellHistoryCompletion.ts` | 119 | `!` shell 历史前缀补全与后缀建议 |
| `slackChannelSuggestions.ts` | 209 | Slack MCP `#channel` 补全、缓存、known-channel 高亮 |
| `skillUsageTracking.ts` | 55 | 技能使用频率和 7 天半衰期排序 |
| **合计** | **1,222** | 不含测试 |

### processUserInput/

| 文件 | 行数 | 说明 |
|------|------|------|
| `processUserInput.ts` | 620 | 中央输入路由、图片/附件/IDE/Hook 预处理 |
| `processSlashCommand.tsx` | 1,209 | slash command、prompt command、local/local-jsx、forked skill、插件遥测 |
| `processBashCommand.tsx` | 182 | input-box `!` bash / PowerShell 路由、进度 UI、结果 XML 包装 |
| `processTextPrompt.ts` | 100 | 文本/图片 user message、prompt id、OTEL、否定/继续关键词 |
| **合计** | **2,111** | 不含测试 |

## Rust crates 应用状态

### 建议系统

| Bun 能力 | Rust 当前位置 | 状态 | 主要差距 |
|----------|---------------|------|----------|
| 命令补全 | `crates/claude-code-rs/src/ui/command_palette/`, `ui/components/fuzzy_match.rs`, `cc-commands` | 部分应用 | 仅 Rust TUI 命令面板；无 `findMidInputSlashCommand`、inline ghost suffix、Fuse 权重、hidden exact 特例、技能使用排序、按 builtin/user/project/policy 分组 |
| 目录/路径补全 | `crates/claude-code-rs/src/ui/input/file_search.rs`, `crates/cc-services/src/file_search.rs` | 相邻实现 | 当前是文件/内容搜索，不是输入 token 的 `getDirectoryCompletions` / `getPathCompletions`；无 LRU path cache、目录优先排序、`includeHidden/includeFiles` 选项接线 |
| Shell 历史补全 | `ui/runtime/persistent_history.rs`, `ui/components/history_search_dialog.rs`, `ui/app/input.rs` | 相邻实现 | 已有 Ctrl+R prompt history；没有专门读取 `!` 历史并返回 suffix 的 `getShellHistoryCompletion` |
| Slack 频道补全 | `cc-daemon`/`gateway` 有 Slack webhook 与 channel 概念 | 未应用到输入建议 | 无 MCP `slack_search_channels` 查询、`#channel` token 定位、known channel cache、footer suggestions |
| 技能使用追踪 | `cc-skills`, `cc-commands/src/skills_cmd.rs`, `cc-engine/src/skill_tool.rs` | 技能系统已落地，排序未落地 | 无 `skillUsage` 持久化、60s debounce、7 天半衰期分数，也未用于 `/` 命令建议排序 |
| 下一步 prompt suggestion | `cc-services/src/prompt_suggestion.rs`, `ui/tui/engine_events.rs`, `app_runtime_adapters/sdk_mapper.rs`, `BackendMessage::Suggestions` | 已应用 | 这是 assistant turn 后的下一步建议，不等价于 Bun `utils/suggestions/**` 的输入补全 |

### 输入处理

| Bun 能力 | Rust 当前位置 | 状态 | 主要差距 |
|----------|---------------|------|----------|
| 中央输入路由 | `cc-engine/src/input_processing.rs`, `cc-engine/src/lifecycle/submit_message.rs` | 部分应用 | 只统一处理普通文本和已注册 slash command；TUI/headless 各自还有独立路由，未形成 Bun 式单入口 |
| `UserPromptSubmit` Hook | `cc-engine/src/lifecycle/submit_message.rs` | 部分应用 | Rust 已能阻断 prompt；尚未等价处理 Bun 的 additional contexts、progress hook message、blocking attachment 形态 |
| Slash command 执行 | `cc-commands/src/lib.rs`, `ui/tui/commands.rs`, `app_runtime_adapters/ingress.rs`, `cc-engine/src/command_runtime.rs` | 部分应用 | 内建命令和 alias 已接通；Bun 的 prompt command 动态技能 slash、local-jsx UI、forked command、plugin telemetry、unknown skill args 保留等仍不完整 |
| Bash input mode | 消息渲染有 `user_bash_input_message.rs` / `user_bash_output_message.rs` | 基本未接输入路由 | 没有 Bun `mode === "bash"` / input-box `!` 路由，也无 bash vs PowerShell 选择和 shell progress UI |
| Text prompt | `cc-engine/src/input_processing.rs` | 部分应用 | 可创建普通 user message；缺少图片 content blocks、image paste ids、permissionMode 写入、prompt id、OTEL、否定/继续关键词统计 |
| 附件/IDE 选择 | 多处 attachment 渲染与工具产生的 `Attachment` | 相邻实现 | 没有 Bun `getAttachmentMessages(input, ideSelection, ...)` 风格的输入前解析管线 |
| 图片粘贴 | `ui/input/clipboard_paste.rs`, `ui/prompt_input.rs` | 相邻实现 | 有路径/大粘贴 UI 辅助；没有 Bun 的 pasted image store、resize/downsample、metadata meta-message 管线 |

## 差距细节

### 1. 命令补全

Rust TUI 命令面板现在从 `cc_commands::get_all_commands()` 取命令，使用自研 `fuzzy_match` 进行 exact/prefix/contains/subsequence 排序，并能显示 usage/examples/edit targets。

仍缺 Bun 行为：

- 输入中间的 `/cmd` token 识别与 ghost suffix。
- Fuse.js 的多字段加权搜索和 alias key。
- 空 query 时按 recently used / builtin / user / project / policy 分组。
- 隐藏命令 exact-name 仍能优先出现的特例。
- prompt skill 使用频率排序。
- `applyCommandSuggestion(... shouldExecute)` 的“无参命令直接提交”行为。

### 2. 路径和目录补全

Rust 有两类相邻能力：

- `ui/input/file_search.rs`: ignore-aware 文件路径搜索，返回匹配文件。
- `cc-services/src/file_search.rs`: headless `SearchFiles`，用 `rg` 或 pure-Rust fallback 做内容搜索。

它们都不是 Bun 的输入框路径补全。缺口是：

- `parsePartialPath()` 的 cwd / `~` / separator 解析语义。
- `scanDirectory()` / `scanDirectoryForPaths()` 的 LRU 缓存和 TTL。
- `getDirectoryCompletions()` / `getPathCompletions()` 的 prefix 过滤、目录优先、`includeFiles`、`includeHidden`。
- 与 prompt footer / autocomplete keybindings 的接线。

### 3. Shell 历史补全

Rust Ctrl+R 现在能从当前会话和持久 session 中搜索 prompt history。Bun 的 `shellHistoryCompletion.ts` 只针对 `!` shell 命令历史：读取历史里以 `!` 开头的命令，缓存 60 秒，输入长度至少 2，返回 `fullCommand` 和 suffix。

Rust 缺口：

- 区分 prompt history 与 shell command history。
- `!` 模式输入时的 suffix ghost text。
- 执行 shell 命令后增量更新 cache。

### 4. Slack channel suggestions

Rust 目前有 Slack webhook / channel 相关 daemon 基础，但输入建议没有使用它。Bun 行为依赖 connected Slack MCP server，调用 `slack_search_channels`，解析 `Name: #channel`，缓存最多 50 个查询，并维护 known-channel 集合用于定位和高亮已有 `#channel`。

### 5. 技能使用排序

Rust `cc-skills` 已有更完整的技能定义、frontmatter、user/model invocable、allowed tools 和 Skill tool；但 Bun 的 `skillUsageTracking.ts` 是另一个排序层，写入全局配置中的 `skillUsage`，用 usage count 和 7 天半衰期计算排序分。

Rust 缺口：

- 全局技能使用记录 schema。
- invocation 时记录 user-invocable skill usage。
- 命令面板空 query 和 fuzzy tie-break 使用该分数。

## 建议迁移顺序

1. 建 `cc-completions` 或 TUI 内部 `input/completions` 模块，先承接命令、路径、shell-history 的统一 `CompletionItem` 数据模型。
2. 把现有 `command_palette` 的 fuzzy/filter 逻辑迁入共享层，补齐 mid-input slash、ghost suffix、empty-query 分组、hidden exact 和 apply/submit 语义。
3. 用 workspace 已有 `lru` + `std::fs::read_dir` 实现 `getPathCompletions`，先只接 Rust TUI，再考虑 headless IPC。
4. 增加 `!` shell input mode：路由、历史 suffix、Bash/PowerShell 选择、进度消息、`<bash-input>` / `<bash-stdout>` / `<bash-stderr>` 上下文。
5. 最后接 Slack channel suggestions 和 skill usage ranking；它们依赖 MCP/client 状态与全局配置写入，耦合度更高。

## Rust 替代方案

| Bun 模块 | Rust 替代/现状 |
|----------|----------------|
| Fuse.js | 现有 `ui/components/fuzzy_match.rs`，也可评估 `nucleo` / `fuzzy-matcher` |
| LRU Cache | workspace 已有 `lru` crate |
| 文件枚举 | `std::fs::read_dir`，需要 ignore-aware 时用 workspace 已有 `ignore` crate |
| shell-quote | `shell-words` crate 已使用 |
| chokidar | 如需文件监控再引入 `notify` |
| tree-sitter | 如需结构化 token 再接 `tree-sitter` crate |
