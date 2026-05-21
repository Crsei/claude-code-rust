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
- 已存在 `CLAUDE.md` 时不覆盖用户内容，而是通过稳定 marker 刷新或追加 cc-rust 托管段落。
- `/init` 执行结果需要在对话中展示清晰状态，包括创建、更新、跳过的路径，以及 `CLAUDE.md` 是新建还是刷新。

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

## 主要代码落点

- `/init` 行为：`crates/cc-commands/src/init.rs`
- 命令注册与隐藏策略：`crates/cc-commands/src/lib.rs`、`crates/claude-code-rs/src/ui/command_palette/filter.rs`
- 命令面板渲染与 Enter 行为：`crates/claude-code-rs/src/ui/command_palette/`
- TUI 输入事件分发：`crates/claude-code-rs/src/ui/app/input.rs`
- 底部状态栏：`crates/claude-code-rs/src/ui/app/render.rs`
- 启动环境变量注入：`crates/cc-config/src/settings.rs`、`crates/claude-code-rs/src/main.rs`、`crates/start-up/src/fast_paths.rs`
- Anthropic-compatible 请求降级：`crates/cc-api/src/api/client/mod.rs`、`crates/cc-api/src/api/stream_provider.rs`、`crates/cc-api/src/api/providers.rs`
- diff surface：`crates/claude-code-rs/src/ui/command_surface/surfaces/diff.rs`
- 通用 overlay 背景与默认文本样式：`crates/claude-code-rs/src/ui/overlays/mod.rs`、`crates/claude-code-rs/src/ui/app/render.rs`

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
cargo build --workspace --release
```

修复完成后需要检查新增 warning，并处理所有由本次改动引入的未使用项或 dead code。

## 注意事项

- `/advisor` 只是从可见命令发现入口隐藏，不应删除 handler 或破坏直接执行。
- 已有 `CLAUDE.md` 内容必须保留。
- 状态栏里的“模型、工作区”按完整模型 ID 和完整工作区路径处理。
