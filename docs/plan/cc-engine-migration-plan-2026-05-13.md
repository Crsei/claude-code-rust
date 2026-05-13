# cc-engine 迁移执行计划

> 日期：2026-05-13
> 范围：将 `crates/claude-code-rs/src/engine/**` 的主要模块迁移到 `crates/cc-engine/**`，并把 `claude-code-rs` 收敛为入口层和兼容适配层。

## 目标

把旧 `engine` 目录从 root crate 中拆出来，形成可独立演进的 `cc-engine` 核心包。

迁移完成后：

- `cc-engine` 拥有 `QueryEngine` 生命周期实现。
- `cc-engine` 拥有 `sdk_types`、`result`、`input_processing`、`codex_exec`、`system_prompt`、`prompt_sections`、`output_style`、`effort`、`agent` 相关实现。
- `claude-code-rs` 只保留入口、UI、IPC、daemon、tools、commands 等壳层逻辑。
- root crate 不再通过 `#[path = "..."]` 继续承载真实 `engine` 实现。
- 所有旧导入路径先通过 shim 过渡，最后再统一删除。

## 当前状态

### 已落到 `cc-engine` 的内容

- `types/{app_state, config, tool}`：已进入 `cc-engine::types`
- `status_line`：已进入 `cc-engine::status_line`
- `agent_runtime`：已进入 `cc-engine::agent_runtime`
- `lifecycle` 源码：物理位置已在 `cc-engine/src/lifecycle/**`，但 root crate 仍用 shim 方式挂载

### 仍由 root `engine/` 承载

- `agent/{mod, dispatch, fork, supervisor, tool_impl, worktree}`
- `codex_exec.rs`
- `effort.rs`
- `input_processing.rs`
- `output_style.rs`
- `prompt_sections.rs`
- `result.rs`
- `sdk_types.rs`
- `system_prompt.rs`

### 关键 shim

- `crates/claude-code-rs/src/engine/lifecycle/mod.rs` 仍通过 `#[path = "../../../../cc-engine/src/lifecycle/mod.rs"]` 转发
- `crates/claude-code-rs/src/types/mod.rs` 仍转发 `cc_engine::types`
- `crates/claude-code-rs/src/ui/mod.rs` 仍转发 `cc_engine::status_line`

## 迁移原则

1. 先搬纯叶子，再搬边界层，最后搬高耦合运行时。
2. 每一阶段都保持 `claude-code-rs` 可编译。
3. `cc-engine` 不能反向依赖 `claude-code-rs` 私有模块。
4. 只在公共 API 和测试稳定后再删除 shim。
5. 迁移顺序按依赖图走，不按文件大小走。

## 模块分层

| 层级 | 模块 | 依赖特征 | 建议顺序 |
| --- | --- | --- | --- |
| L0 | `codex_exec.rs`、`effort.rs` | 基本是纯函数和常量 | 最先 |
| L1 | `result.rs`、`input_processing.rs`、`sdk_types.rs` | 依赖 `cc-types` 和生命周期类型 | 第二波 |
| L2 | `prompt_sections.rs`、`output_style.rs`、`system_prompt.rs` | 依赖配置、路径、git、prompt 组装 | 第三波 |
| L3 | `agent/*` | 依赖 `QueryEngine`、权限、worktree、tasks、mcp、hooks | 最后 |
| L4 | `lifecycle` 公共化收口 | 只是把已迁入的源码真正暴露到 `cc-engine` | 和第二波并行收口 |

## 分阶段计划

### Phase 0：冻结基线

目标：记录迁移前行为，避免把既有缺陷误判成迁移回归。

工作项：

- 记录旧 `engine` 文件清单和当前 `cc-engine` 目录结构
- 记录 `QueryEngine`、`Agent`、`system prompt`、`output style`、`effort` 的现有测试覆盖
- 确认 `cc-engine` 当前的公开面只包含 `types`、`status_line`、`agent_runtime`

完成标准：

- 有可回溯的迁移前基线
- 没有行为改动

### Phase 1：先拆纯叶子

目标：把最容易独立的逻辑先挪进 `cc-engine`，缩小 root crate 的核心负担。

优先迁移：

- `codex_exec.rs`
- `effort.rs`
- `result.rs`

说明：

- `codex_exec.rs` 和 `effort.rs` 基本不依赖运行时状态，可以先落地
- `result.rs` 与消息收尾逻辑强相关，适合和生命周期边界一起收口

完成标准：

- `cc-engine` 内可直接引用这些模块
- root crate 不再把这些模块当作核心实现源

### Phase 2：迁移输入与 SDK 边界

目标：把查询入口和外部可见输出结构从 root crate 拆到 `cc-engine`。

优先迁移：

- `input_processing.rs`
- `sdk_types.rs`

说明：

- `input_processing.rs` 负责 slash command 识别和 user message 构造，依赖面相对稳定
- `sdk_types.rs` 是 `QueryEngine` 对外输出的结构层，应该和生命周期同一 crate 演进

需要同步处理的调用点：

- `claude-code-rs` 中所有 `crate::engine::sdk_types::*` 的引用
- `claude-code-rs` 中所有 `crate::engine::input_processing::*` 的引用

完成标准：

- SDK 输出类型不再由 root crate 主持
- 输入处理逻辑不再依赖 root crate 的模块布局

### Phase 3：迁移 prompt 组装链

目标：把系统提示词生成链从 root crate 中拆出。

优先迁移：

- `prompt_sections.rs`
- `output_style.rs`
- `system_prompt.rs`

依赖处理：

- `output_style.rs` 目前只需要一个稳定的路径/数据根访问点，应该先抽成轻量 adapter，再迁入 `cc-engine`
- `system_prompt.rs` 依赖 `claude_md`、git 状态、工作目录和工具上下文，需要把 root-only helper 变成显式输入参数或轻量 trait
- `prompt_sections.rs` 可以作为纯缓存和拼装层最先进入

完成标准：

- `cc-engine` 可以独立构建系统提示词
- root crate 只提供少量环境适配，不再承载 prompt 组装主体

### Phase 4：迁移 `agent` 子树

目标：把子代理、工作树、后台 supervisor、工具适配整体迁入 `cc-engine`。

优先迁移：

- `agent/mod.rs`
- `agent/dispatch.rs`
- `agent/fork.rs`
- `agent/supervisor.rs`
- `agent/tool_impl.rs`
- `agent/worktree.rs`

依赖处理：

- `QueryEngine` 需要先稳定为 `cc-engine` 的正式 public API
- 权限相关接口要以 `cc-types` 的类型为主，避免反向依赖 root crate
- worktree、task、mcp、hooks、bash 校验等 root 依赖要么抽成 adapter，要么改成 workspace crate 调用

完成标准：

- Agent tool 的所有执行路径都能在 `cc-engine` 内闭环
- root crate 不再直接持有 agent 的真实实现

### Phase 5：收口 `lifecycle`

目标：把已经落到 `cc-engine/src/lifecycle/**` 的源码真正公开出来，取消 root shim。

工作项：

- 在 `cc-engine/src/lib.rs` 暴露 `pub mod lifecycle`
- 把 root crate 的 `engine/lifecycle` 改成短期兼容转发或直接删除
- 把所有 `crate::engine::lifecycle::*` 的消费点切到 `cc_engine::lifecycle::*`

完成标准：

- `lifecycle` 不再依赖 root crate 的 `#[path]` 载入
- `cc-engine` 成为 `QueryEngine` 的唯一源码归属

### Phase 6：删除旧 `engine` 主实现

目标：清掉 root crate 中的真实实现，只保留必要的兼容层或彻底移除。

工作项：

- 删除 `crates/claude-code-rs/src/engine/` 下的真实实现文件
- 仅保留必要的过渡性 re-export，直到所有调用点切换完成
- 更新文档和测试路径，移除旧模块名假设

完成标准：

- root crate 不再包含 `engine` 业务实现
- `cc-engine` 成为唯一权威实现

## 主要阻断点

- `system_prompt.rs` 现在直接依赖 root 的 `config`、`utils::git`、`tools::worktree`
- `agent/*` 直接依赖 `permissions`、`worktree_hooks`、`tools::tasks`、`mcp`、`services::langfuse`
- `sdk_types.rs` 和 `result.rs` 共享 `lifecycle` 类型，必须同步迁移
- `output_style.rs` 和 `effort.rs` 现在被 `cc-ui` / `cc-config` 的逻辑引用，调用点切换要一起做

## 验收顺序

建议按下面的测试顺序推进：

1. `cargo test -p cc-engine`
2. `cargo test -p claude-code-rs engine::...`
3. `cargo test -p claude-code-rs ui::...`
4. `cargo build --workspace --release`

每一波迁移都要满足两个条件：

- 新 crate 侧测试通过
- 旧 root crate 侧仍可编译，直到最后一轮收口

## 备注

- 这份计划和 `cc-daemon` 迁移计划是并列的，不要混成同一个阶段图
- `lifecycle` 虽然已经搬到 `cc-engine` 目录里，但目前还只是物理迁移，**不是** 完成迁移
- `output_style`、`effort`、`system_prompt` 这几块要优先按依赖清理，而不是按“看起来像纯工具函数”来判断
