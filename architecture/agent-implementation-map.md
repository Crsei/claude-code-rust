# Agent 实现映射表

本文只覆盖 `docs/bun-docs-documentation-plan.md` 的 `agent` 章节，对照 Bun 上游的三份文档，梳理 `cc-rust` 当前实现与差异。

状态分类只使用以下五种：

- `已实现`
- `部分实现`
- `未实现`
- `待确认`
- `故意裁剪`

## 范围

本页仅处理下面三份上游文档，以及它们在 `cc-rust` 中的对应实现：

1. `F:\AIclassmanager\cc\claude-code-bun\docs\agent\coordinator-and-swarm.mdx`
2. `F:\AIclassmanager\cc\claude-code-bun\docs\agent\sub-agents.mdx`
3. `F:\AIclassmanager\cc\claude-code-bun\docs\agent\worktree-isolation.mdx`

只做只读核查，不修改源码，不运行长测试。

## 上游文档清单

| 上游文档 | 主题 | cc-rust 对应实现入口 |
| --- | --- | --- |
| `coordinator-and-swarm.mdx` | Coordinator / swarm / mailbox / task 分发 | `crates/claude-code-rs/src/teams/mod.rs:11`，`crates/claude-code-rs/src/commands/team_cmd.rs:1`，`crates/claude-code-rs/src/tools/send_message.rs:1`，`crates/claude-code-rs/src/tools/team_spawn.rs:1`，`crates/claude-code-rs/src/tools/tasks.rs:1522` |
| `sub-agents.mdx` | 子 Agent、内置 agent、hooks、后台生命周期、AgentTree | `crates/claude-code-rs/src/engine/agent/mod.rs:30`，`crates/claude-code-rs/src/engine/agent/tool_impl.rs:1`，`crates/claude-code-rs/src/engine/agent/dispatch.rs:168`，`crates/claude-code-rs/src/engine/agent/supervisor.rs:74`，`crates/claude-code-rs/src/ipc/builtin_agents.rs:1` |
| `worktree-isolation.mdx` | 子 Agent worktree 隔离、生命周期、清理策略 | `crates/claude-code-rs/src/engine/agent/worktree.rs:1`，`crates/claude-code-rs/src/engine/agent/supervisor.rs:597`，`crates/claude-code-rs/src/engine/agent/tool_impl.rs:44` |

## 实现映射表

| 上游文档 | 结论 | cc-rust 侧核心依据 | 备注 |
| --- | --- | --- | --- |
| `coordinator-and-swarm.mdx` | `部分实现` | `teams/mod.rs:11-44`、`team_cmd.rs:1-224`、`send_message.rs:1-208`、`team_spawn.rs:1-338`、`tasks.rs:1522-1600` | Agent Teams、in-process teammate、Mailbox、TaskList / TaskStop 都有；但没有独立的 coordinator 专用模式入口，也没有 `subscribe_pr_activity`。 |
| `sub-agents.mdx` | `已实现` | `engine/agent/mod.rs:30-62`、`tool_impl.rs:1-229`、`dispatch.rs:168-238`、`supervisor.rs:74-189`、`ipc/builtin_agents.rs:1-149`、`ipc/agent_settings.rs:548-582` | 子 Agent 入口、内置 agent、background / sync 生命周期、hooks、AgentTree、worktree 隔离和 fork 侧路都已落地。 |
| `worktree-isolation.mdx` | `部分实现` | `engine/agent/worktree.rs:1-411`、`engine/agent/supervisor.rs:597-762`、`engine/agent/tool_impl.rs:147-229` | 有 worktree 隔离和清理，但路径、hook 入口和恢复流程与 Bun 上游不同。 |

## 逐文档分析

### coordinator-and-swarm.mdx

- 上游核心主题是 Coordinator Mode、Agent Swarm、Mailbox、TaskList / TaskStop、以及 PR activity 订阅。
- `cc-rust` 已经实现了一个更偏“in-process swarm”的替代面：`teams/mod.rs:11-44` 说明它把 Agent Teams 作为单独模块组织起来；`teams/mod.rs:74-84` 提供 `is_agent_teams_enabled()` 和 `is_agent_teams_active()`，把 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 和会话级 `team_context` 作为激活条件。
- `/team` 入口已经接上：`commands/team_cmd.rs:1-224` 覆盖 `create`、`spawn`、`send`、`kill`、`leave`、`delete`，并把 active team 镜像回 `AppState::team_context`。
- `TeamSpawn` 已接到 Agent 侧：`tools/team_spawn.rs:1-338` 负责创建 in-process teammate、写入 team file、更新 `AppState::team_context`，并且默认只支持 `in-process` backend；`tools/registry.rs:51-70` 也把 `TeamSpawnTool`、`SendMessageTool`、`TaskListTool`、`TaskStopTool` 注册进工具池。
- `SendMessage` 已实现 mailbox 定向与广播能力：`tools/send_message.rs:1-208` 负责路由消息到队友 mailbox；`teams/mailbox.rs:71-177` 负责读写、锁和清理；`teams/runner.rs:281-448` 负责轮询 mailbox、处理协议消息和空闲通知。
- `TaskList` / `TaskStop` 已实现：`tools/tasks.rs:1522-1600` 提供任务列表与取消，配合 `supervisor.rs:74-189` 把后台 Agent 绑定到持久任务。
- `plan` 相关的审批链路也有对应：`tools/plan_mode.rs` 和 `teams/runner.rs:330-386` 支持计划审批消息往返。
- 缺口在于 Bun 的独立 coordinator 机制没有在 Rust 源码里出现：当前没有单独的 `coordinatorMode` 入口，也没有 `subscribe_pr_activity` 一类的 PR 事件订阅工具。
- 另一个显著差异是后端策略：`teams/mod.rs:39-44` 明确把当前实现收敛到 `in_process::InProcessBackend`，因此 Bun 文档里的 tmux / 多终端 swarm 路径属于 `故意裁剪`。

### sub-agents.mdx

- 上游主题是 `AgentTool.call()` 的完整链路、fork 子进程、内置 agent、hook 注入、后台生命周期、worktree 隔离和结果回传。
- `AgentTool` 本体已经存在：`engine/agent/mod.rs:30-62` 定义了 `AgentTool`、`MAX_AGENT_DEPTH`、`subagent_type`、`run_in_background`、`name`、`mode` 和 `isolation` 等输入字段。
- 命名 teammate 的分支已经接入：`engine/agent/tool_impl.rs:112-138` 会先判断 `teammate_spawn_request()`，命中后转到 `TeamSpawnTool`，这条路径直接对应上游文档里的“命名 agent / teammate”能力。
- 內置 agent 已经注册：`ipc/builtin_agents.rs:1-149` 列出 `general-purpose`、`Explore`、`Plan`、`code-reviewer`、`statusline-setup`；`ipc/agent_settings.rs:548-582` 还把 `ExitPlanMode`、`SendMessage`、`TeamSpawn` 等名字预置进可见工具集合。
- 工具池和权限边界是独立组装的：`tools/registry.rs:51-70` 把 `Agent`、`EnterPlanMode`、`ExitPlanMode`、`TaskList`、`TaskStop`、`SendMessage`、`TeamSpawn` 放进默认注册表；`tools/send_message.rs:71-101` 和 `tools/team_spawn.rs:99-136` 也分别对 team 上下文做了校验。
- 生命周期钩子已接入：`engine/agent/dispatch.rs:168-238` 显式触发 `SubagentStart` / `SubagentStop`；`engine/agent/supervisor.rs:100-154`、`428-463` 在后台路径里也会再次触发这些 hook。
- 后台 Agent 已实现：`engine/agent/tool_impl.rs:190-229` 进入 `spawn_background_agent()`；`engine/agent/supervisor.rs:74-189` 负责注册 task、创建 cancellation token、绑定树节点；`supervisor.rs:374-463` 负责结束、结果、树快照和 hook 收尾。
- `worktree` 作为子 Agent 隔离模式也已经纳入：`engine/agent/worktree.rs:1-411` 负责 worktree 内执行、结果保留/清理和失败回退。
- 轻量 fork 侧路也存在：`engine/agent/fork.rs:1-130` 提供无持久化的子引擎执行，支持 `parent_messages` 以复用上下文。
- 因此，这份 Bun 文档里的“子 Agent 机制”在 Rust 侧整体可视为 `已实现`；但如果要追求与 Bun 的路径分层、门控和 prompt-cache 细节逐项一致，仍需要后续复核。

### worktree-isolation.mdx

- 上游文档强调 `.claude/worktrees/<slug>` 的目录结构、`WorktreeCreate` / `WorktreeRemove` hook、`EnterWorktreeTool` / `ExitWorktreeTool` 以及 fail-closed 的删除逻辑。
- `cc-rust` 已经有 worktree 运行面：`engine/agent/worktree.rs:1-411` 明确负责“Git worktree execution for agent isolation”，包含创建、运行、结果回传和清理。
- `AgentTool` 的输入已经暴露 `isolation: "worktree"`：`engine/agent/mod.rs:56` 和 `tool_impl.rs:44` 一起说明 worktree 是 Agent 的正式隔离模式之一。
- 运行时 worktree 采用临时目录和自动分支命名：`worktree.rs:86-106` 生成 `agent-worktree-{id}`，`worktree.rs:275-342` 根据变更决定保留或清理。
- 后台 Agent 也复用同一套 worktree 生命周期：`supervisor.rs:597-762` 先准备 worktree，再在退出时按变更数决定保留或清理。
- 变更统计是 fail-closed 的：`engine/agent/mod.rs:104-137` 的 `count_worktree_changes()` 依赖 `git status` / `git rev-list`，失败时返回 `None`，后续清理逻辑不会冒险删除。
- 这意味着“子 Agent worktree 隔离”能力是存在的，但 Bun 文档里的路径布局、hook 方式和会话恢复流程并没有一一对齐，因此整章结论应记为 `部分实现`，其中 `.claude/worktrees` 和 `WorktreeCreate` / `WorktreeRemove` 属于 `故意裁剪`。

## 已实现汇总

- `AgentTool` 已有完整输入模型，包含 `subagent_type`、`run_in_background` 和 `isolation`：`engine/agent/mod.rs:30-62`。
- 内置 subagent registry 已存在：`ipc/builtin_agents.rs:1-149`。
- `SubagentStart` / `SubagentStop` hooks 已接入：`engine/agent/dispatch.rs:168-238`、`supervisor.rs:100-154`、`428-463`。
- 后台 Agent 生命周期已接入 task store 和 AgentTree：`supervisor.rs:74-189`、`497-541`。
- `SendMessage`、`TeamSpawn`、`TaskList`、`TaskStop` 已在工具注册表中：`tools/registry.rs:51-70`。
- mailbox 读写与队友消息流转已实现：`teams/mailbox.rs:71-177`、`teams/runner.rs:281-448`。
- worktree 隔离已支持，并且有结果保留 / 清理分支：`engine/agent/worktree.rs:1-411`。

## 未实现 / 部分实现 / 待确认 / 故意裁剪

- `未实现`：独立的 coordinator 专用模式入口、专用 gate、以及 `subscribe_pr_activity` 一类的 PR 事件订阅工具。当前 Rust 侧只看到 Agent Teams / in-process swarm 的替代路径。
- `部分实现`：`coordinator-and-swarm.mdx` 里的 swarm 语义在 Rust 侧是“in-process teammate + mailbox + task store”版本，和 Bun 的 coordinator / swarm 分层不是同构实现。
- `部分实现`：`worktree-isolation.mdx` 的上游目录布局与工具链更完整，Rust 侧已经有 worktree，但路径和生命周期更偏简单直接。
- `故意裁剪`：`teams/mod.rs:39-44` 已明确只保留 `in_process::InProcessBackend`，因此 tmux / iTerm2 一类后端不在当前实现范围内。
- `故意裁剪`：上游 `WorktreeCreate` / `WorktreeRemove` 的 hook 驱动创建与销毁流程，在 `cc-rust` 里没有对应的同构实现。
- `待确认`：Bun 文档中的 coordinator 专用 prompt、team lead / worker 工具白名单分层，以及 PR 订阅的精确行为，如果后续要写成“与 Bun 完全对齐”，还需要再做一次逐文件核查。

## 后续动作

1. 如果后续要继续补 `context`、`extensibility`、`safety`、`tools` 四章，沿用同一模板分别生成映射表。
2. 如果后续要把 `coordinator-and-swarm.mdx` 写成更接近“逐条对照”，建议单独补一轮对 `commands/team_cmd.rs`、`teams/runner.rs`、`tools/send_message.rs`、`tools/tasks.rs` 的行为核查。
3. 如果要追求与 Bun 的 worktree 章节完全对齐，需要补 `WorktreeCreate` / `WorktreeRemove` 的 hook 对照，再判断是否属于显式裁剪还是待实现。
