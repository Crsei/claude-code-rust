# TUI 命令面板、/init 与状态栏修复记录

日期：2026-05-22

## 背景

本记录汇总一次 Rust TUI 交互问题排查与修复范围，涉及模型显示、命令面板、`/init`、`/add-dir`、`/diff` 和命令面板样式。

用户反馈的运行错误：

```text
API error provider=anthropic status=403 type=new_api_error: 用户额度不足
```

本机配置检查结果显示，`~/.cc-rust/settings.json` 已通过 `env` 将 Anthropic-compatible 调用指向 `deepseek-v4-pro`：

- `ANTHROPIC_BASE_URL = https://inferaichat.com`
- `ANTHROPIC_MODEL = deepseek-v4-pro`
- `ANTHROPIC_DEFAULT_HAIKU_MODEL = deepseek-v4-pro`
- `ANTHROPIC_DEFAULT_OPUS_MODEL = deepseek-v4-pro`
- `ANTHROPIC_DEFAULT_SONNET_MODEL = deepseek-v4-pro`

后续复查发现启动进程中可继承外部 shell 环境变量，例如：

- `ANTHROPIC_MODEL = claude-opus-4-20250514`
- `ANTHROPIC_BASE_URL = https://ssvip.dmxapi.com`

旧启动逻辑中 `settings.env` 只填充缺失变量，不覆盖已存在的进程环境变量，因此 `~/.cc-rust/settings.json` 中的 `deepseek-v4-pro` 会被外部环境遮蔽，导致状态栏仍显示 opus4，并可能把请求发到旧账号或旧 base URL。Anthropic-compatible 请求端点仍是 `ANTHROPIC_BASE_URL + /v1/messages`。

## 修复范围

### `/init`

- 继续在缺失时创建 `.cc-rust/settings.json`。
- 缺失 `CLAUDE.md` 时生成完整模板。
- 已存在 `CLAUDE.md` 时不覆盖、不刷新、不追加用户文件内容，直接返回：
  `CLAUDE.md already exists here. Skipping /init to avoid overwriting it.`
- 新建模板中的项目说明改为 cc-rust，不再写 `Project instructions for Claude Code.`。

### cc-rust 文案统一

- 系统提示词前缀改为 `You are cc-rust, a coding CLI.`。
- `/help` 系统提示、内置 general-purpose agent、statusline agent、hook source 描述、bundled `update-config` skill、Read/Bash 工具提示、CLI about 文案等用户或模型可见文本统一改为 cc-rust。
- 保留真实协议、兼容性或路径语义中的名字，例如 `CLAUDE.md` 文件名、模型 ID、`Claude.ai` OAuth、上游规范说明和参考文档。

### 命令面板

- 命令别名紧跟命令名显示，不保留额外空隔，例如 `/assistant(kairos)`。
- `Command details` 固定列对齐，不能被别名长度挤歪。
- 隐藏 `/advisor` 的可见命令入口，但保留直接输入 `/advisor ...` 的兼容执行能力。
- 高亮命令按 Enter：
  - 无参数命令直接执行。
  - 需要参数的命令填入 `/command ` 并等待用户补参。
- 面板中的行为说明要匹配实际行为，不再固定写成 `Enter inserts`。

### 底部状态栏

- 输入框下方默认只展示完整模型名和当前工作区路径。
- 不再截断 `deepseek-v4-pro` 这类模型 ID。
- 默认隐藏消息数、ready/streaming、权限、sandbox、effort、快捷键、cost、remote、vim 等附加状态。

### 启动环境变量

- TUI 主启动和 dump-system-prompt 快路径使用启动专用的 `settings.env` 注入策略。
- 普通变量仍只在缺失时填充。
- cc-rust settings 中显式声明的 provider auth、endpoint、model 变量会覆盖继承来的 shell 环境变量，避免被外部 Claude/Codex 环境污染。
- 覆盖范围包括 `ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`、`ANTHROPIC_MODEL`、Anthropic tier fallback 模型变量，以及 OpenAI Codex auth/base/model 变量。

### Anthropic-compatible 520

- 非官方 Anthropic-compatible base URL 继续使用 `/v1/messages`。
- 兼容端点不再发送 `anthropic-beta` 扩展 header，避免 `interleaved-thinking`、`prompt-caching` 等官方 beta 头触发网关 520。
- 兼容端点请求体会剥离 `cache_control` / `cache_reference` 等 prompt-cache 字段，以及顶层 `thinking` 字段。
- 官方 `https://api.anthropic.com` 仍保留 beta header、prompt-cache 与 thinking 行为。

### `/add-dir`

- `/add-dir` 的执行结果继续进入对话记录。
- 信息文本使用中性样式，不再以蓝色信息字体默认显示。

### `/diff`

- `/diff` 打开的 diff 面板在正常列表和详情模式下也显示 `Esc close`。
- 错误模式下继续显示关闭提示。

### 新命令面板样式

- 新出现的命令/子面板使用更深的背景色。
- 面板主体默认使用纯白字体。
- 已有 warning/error/accent 等显式语义样式可继续保留。

### Session 内容滚动条

- prompt session 和 transcript body 在内容高度超过终端可视区域时，右侧显示 1 列滚动条。
- 滚动条包含顶部/底部箭头、轨道和当前位置滑块，用于提示还有隐藏内容。
- 鼠标点击顶部/底部箭头按行滚动。
- 鼠标点击或拖动中间轨道会按位置跳转 session 内容。
- 鼠标捕获默认开启，滚轮无需额外设置即可发送到 TUI 并滚动聊天记录；如需恢复终端原生拖选，设置 `CLAUDE_CODE_DISABLE_MOUSE=1`。
- TUI 启动时会把 `QueryEngine` 中已恢复的 session history 同步到 App，并默认定位到最底部。
- 修复 virtual scroll overscan 下的 skip 计算：每条消息按自己的 visual offset 决定跳过行数，避免默认 bottom 或滚动到底部时仍显示旧内容。
- 保留既有键盘滚动和 transcript 滚动行为。

### 用户消息背景

- 普通用户文本消息使用整行背景色，不再只给文本 span 上色。
- 背景块额外包含用户消息上方一行和下方一行，使用户输入在 TUI 中形成完整高亮区域。
- 工具结果、attachment、meta user message 等非普通用户输入不套用该背景。

### 运行中输入与队列

- 参考 `/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/codex/codex-rs/tui/src/bottom_pane/chat_composer.rs` 的语义：`Enter` 是提交，`Tab` 是显式 queue；Codex footer 对应提示为 `tab to queue message`。
- cc-rust 运行中仍允许在输入框继续编辑草稿。
- 运行中按 `Enter` 不再隐式把草稿入队，也不会清空输入框；TUI 会提示使用 `Tab` 排队，避免用户误以为当前 turn 被立即打断或在下一次工具调用时注入。
- 运行中按 `Tab` 才把当前草稿转成内存 FIFO 队列项；队列保存在 TUI runner 的 `queued_prompts` 中。
- 队列项不会在“下一次工具调用”时发送到对话；只有当前模型 turn 结束并收到 `EngineEvent::Done` 后，TUI 才从队列头取出一条，按正常用户输入路径写入对话并启动下一轮请求。
- 输入框下方状态栏在运行中有草稿时显示 `tab to queue message`；已有队列时同时显示 `N queued`。

### 兼容网关 stream 尾部错误

- 非官方 Anthropic-compatible 网关可能在已经输出文本后关闭 chunked/SSE 响应，Rust 侧原先会把这类尾部 `error reading response chunk` 展示为 API error。
- 当前只在“已经累积到纯文本/思考文本且没有 tool_use”的情况下接受部分 assistant response，并补 `end_turn` stop reason；带 tool_use 的中断仍按错误/回退处理，避免执行不完整工具调用。

### 对话路径跳转、复制选择与任务耗时

- 消息选择模式现在会从 assistant 文本、tool result 文本和 connector 文本中提取代码路径引用，支持 `path=...`、普通相对路径和可选 `:line` 行号。
- 选中包含代码路径的消息后按 `o` 会通过 `$VISUAL` 或 `$EDITOR` 打开对应文件；`code`/`code-insiders`/`codium` 使用 `-g path:line`，常见终端编辑器使用 `+line path`。
- 选中消息的 header 显示 `o open`，和已有复制动作一起作为消息动作入口。
- TUI 默认启用 mouse capture 以支持滚轮和滚动条；滚轮统一滚动聊天记录，输入框历史只通过键盘上/下键切换。需要终端原生拖选时设置 `CLAUDE_CODE_DISABLE_MOUSE=1` 或 `CLAUDE_CODE_ENABLE_MOUSE_CAPTURE=0`，部分终端仍可用 Shift+拖选临时绕过应用鼠标捕获。
- 工具/任务展示统一以缩进后的 `●` 开始，避免和正文混在同一视觉层级。
- 工具 compact line、后台任务行、任务 header 和回合结束消息都会显示 `worked for ...`，便于确认本轮或单个任务的运行耗时。

### 鼠标区域焦点与 subagent Task 兼容

- App 在每次 render 时记录当前聊天历史区域和 prompt 输入框区域。
- 鼠标点击输入框后，或者滚轮事件直接落在输入框区域时，滚轮上/下会调用 prompt history previous/next，展示历史输入内容。
- 鼠标点击聊天历史区域后，或者滚轮事件直接落在聊天历史区域时，滚轮上/下会滚动 session 历史消息。
- 保留 transcript/focus 模式原有滚动路径；prompt 历史滚轮只在 prompt view 且输入框 active 时生效。
- Agent 工具新增上游兼容别名 `Task`，注册到默认工具池和 coordinator 工具池。模型在聊天框中按 Claude Code 上游习惯调用 `Task` 时，会复用 cc-rust 现有 `AgentTool` subagent runtime，不再因 `tool not found: Task` 显示调用失败。
- `Task` 别名复用 `Agent` 的 schema、校验、权限检查和执行逻辑；worker/teammate policy 仍不暴露 `Agent`/`Task` 生成子 agent，避免递归 spawn 面扩大。

## 主要代码落点

- `/init` 行为：`crates/cc-commands/src/init.rs`
- cc-rust 提示词/可见文案：`crates/cc-config/src/constants.rs`、`crates/cc-engine/src/system_prompt.rs`、`crates/cc-engine/src/agent/builtin_agents.rs`、`crates/cc-services/src/agent_definitions/builtin.rs`、`crates/cc-tools/src/`、`crates/cc-skills/src/bundled.rs`、`crates/claude-code-rs/src/cli.rs`
- 命令注册与隐藏策略：`crates/cc-commands/src/lib.rs`、`crates/claude-code-rs/src/ui/command_palette/filter.rs`
- 命令面板渲染与 Enter 行为：`crates/claude-code-rs/src/ui/command_palette/`
- TUI 输入事件分发：`crates/claude-code-rs/src/ui/app/input.rs`
- 底部状态栏、运行中 queue 提示与 session 滚动条：`crates/claude-code-rs/src/ui/app/render.rs`
- 运行中输入队列调度：`crates/claude-code-rs/src/ui/tui.rs`
- 用户消息背景与 virtual scroll：`crates/claude-code-rs/src/ui/messages/render.rs`
- stream chunk 尾部错误降级：`crates/cc-engine/src/query/loop_impl.rs`
- 启动环境变量注入：`crates/cc-config/src/settings.rs`、`crates/claude-code-rs/src/main.rs`、`crates/start-up/src/fast_paths.rs`
- Anthropic-compatible 请求降级：`crates/cc-api/src/api/client/mod.rs`、`crates/cc-api/src/api/stream_provider.rs`、`crates/cc-api/src/api/providers.rs`
- diff surface：`crates/claude-code-rs/src/ui/command_surface/surfaces/diff.rs`
- 通用 overlay 背景与默认文本样式：`crates/claude-code-rs/src/ui/overlays/mod.rs`、`crates/claude-code-rs/src/ui/app/render.rs`
- 消息路径提取与 `o open` 提示：`crates/claude-code-rs/src/ui/messages/render.rs`
- 消息动作与默认快捷键：`crates/claude-code-rs/src/ui/app/input.rs`、`crates/cc-keybindings/src/defaults.rs`
- 路径打开实现：`crates/claude-code-rs/src/ui/tui/export.rs`、`crates/claude-code-rs/src/ui/tui.rs`
- 终端选择/复制默认策略：`crates/claude-code-rs/src/ui/platform/terminal_env.rs`
- 鼠标区域焦点与滚轮路由：`crates/claude-code-rs/src/ui/app.rs`、`crates/claude-code-rs/src/ui/app/input.rs`、`crates/claude-code-rs/src/ui/app/render.rs`
- 任务 bullet 与耗时：`crates/claude-code-rs/src/ui/messages/assistant_tool_use_message.rs`、`crates/claude-code-rs/src/ui/messages/grouped_tool_use_content.rs`、`crates/claude-code-rs/src/ui/rendering/tool_activity.rs`、`crates/claude-code-rs/src/ui/tasks/background_task.rs`、`crates/claude-code-rs/src/ui/tasks/task_status_utils.rs`、`crates/claude-code-rs/src/ui/tui/engine_events.rs`
- subagent `Task` 兼容别名：`crates/cc-engine/src/agent/mod.rs`、`crates/cc-engine/src/agent/tool_impl.rs`、`crates/start-up/src/tool_registry.rs`、`crates/cc-tools/src/registry.rs`

## 验证计划

使用仓库本地 Rust 工具链：

```bash
export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
export PATH="$CARGO_HOME/bin:$PATH"
```

建议验证：

```bash
cargo test -p cc-commands init
cargo test -p cc-config apply_startup_runtime_env
cargo test -p cc-api startup_settings_env_overrides_inherited_anthropic_provider_env
cargo test -p cc-api compatible_anthropic
cargo test -p claude-code-rs command_palette
cargo test -p claude-code-rs command_surface
cargo test -p cc-engine builtin_agent
cargo test -p claude-code-rs ui::messages::render::tests
cargo test -p claude-code-rs ui::app::tests
cargo test -p claude-code-rs ui::app::tests::prompt_stays_editable_while_streaming_and_tab_queues
cargo build --workspace --release
```

本轮 TUI 文案、滚动条和用户消息背景修复已执行并通过：

```bash
cargo test -p cc-commands init -- --nocapture
cargo test -p cc-engine builtin_agent -- --nocapture
cargo test -p claude-code-rs ui::messages::render::tests -- --nocapture
cargo test -p claude-code-rs ui::app::tests -- --nocapture
cargo build --workspace --release
git diff --check
```

本轮路径跳转、终端复制选择、任务 bullet 和耗时显示修复已执行并通过：

```bash
cargo fmt --all --check
cargo test -p cc-keybindings -- --nocapture
cargo test -p claude-code-rs ui::messages -- --nocapture
cargo test -p claude-code-rs ui::tasks -- --nocapture
cargo test -p claude-code-rs snapshot_grouped_activity_states -- --nocapture
cargo test -p claude-code-rs messages_action_opens_code_path_from_assistant_text -- --nocapture
cargo test -p claude-code-rs enable_mouse_capture_opts_into_mouse_events -- --nocapture
cargo build --workspace --release
git diff --check
```

本轮鼠标区域焦点和 subagent `Task` 兼容修复已执行并通过：

```bash
cargo fmt --all --check
cargo test -p claude-code-rs mouse_wheel -- --nocapture
cargo test -p claude-code-rs enable_mouse_capture -- --nocapture
cargo test -p cc-engine test_task_tool_alias_name_and_schema -- --nocapture
cargo test -p cc-startup test_find_tool_by_name -- --nocapture
cargo test -p cc-startup coordinator_policy_exposes_only_lead_orchestration_tools -- --nocapture
cargo test -p cc-tools coordinator_policy_is_lead_only -- --nocapture
```

修复完成后需要检查新增 warning，并处理所有由本次改动引入的未使用项或 dead code。

## 注意事项

- `/advisor` 只是从可见命令发现入口隐藏，不应删除 handler 或破坏直接执行。
- 已有 `CLAUDE.md` 内容必须保留。
- 状态栏里的“模型、工作区”按完整模型 ID 和完整工作区路径处理。
- `CLAUDE.md` 是兼容既有项目指令约定的文件名，不属于需要替换成 cc-rust 的品牌文案。
