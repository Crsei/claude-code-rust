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

本轮补充核查还读取了上游 Bun 源码中的 `src/coordinator/coordinatorMode.ts`、`src/coordinator/workerAgent.ts`、`src/utils/toolPool.ts`、`src/constants/tools.ts`、`src/utils/worktree.ts`、`src/utils/hooks.ts`，用于收敛原先的 `待确认` 项。

只做只读核查，不修改源码，不运行长测试。

## 上游文档清单

| 上游文档 | 主题 | cc-rust 对应实现入口 |
| --- | --- | --- |
| `coordinator-and-swarm.mdx` | Coordinator / swarm / mailbox / task 分发 | `crates/claude-code-rs/src/teams/mod.rs:11`，`crates/claude-code-rs/src/commands/team_cmd.rs:43`，`crates/claude-code-rs/src/tools/send_message.rs:23`，`crates/claude-code-rs/src/tools/team_spawn.rs:26`，`crates/claude-code-rs/src/tools/tasks.rs:421` |
| `sub-agents.mdx` | 子 Agent、内置 agent、hooks、后台生命周期、AgentTree | `crates/claude-code-rs/src/engine/agent/mod.rs:30`，`crates/claude-code-rs/src/engine/agent/tool_impl.rs:88`，`crates/claude-code-rs/src/engine/agent/dispatch.rs:168`，`crates/claude-code-rs/src/engine/agent/supervisor.rs:74`，`crates/claude-code-rs/src/ipc/builtin_agents.rs:1` |
| `worktree-isolation.mdx` | 子 Agent worktree 隔离、生命周期、清理策略 | `crates/claude-code-rs/src/tools/worktree.rs:1`，`crates/claude-code-rs/src/engine/agent/worktree.rs:1`，`crates/claude-code-rs/src/engine/agent/supervisor.rs:546`，`crates/claude-code-rs/src/engine/agent/tool_impl.rs:44` |

## 实现映射表

| 上游文档 | 结论 | cc-rust 侧核心依据 | 备注 |
| --- | --- | --- | --- |
| `coordinator-and-swarm.mdx` | `部分实现` | `teams/mod.rs:11-44`、`teams/coordinator.rs`、`commands/coordinator.rs`、`tools/registry.rs`、`send_message.rs:23-435`、`team_spawn.rs:26-359`、`teams/runner.rs:64-448`、`tasks.rs:421-476,1972-2029,2086-2198`、`tools/pr_activity.rs` | Agent Teams、coordinator gate / prompt / tool policy、worker agent、in-process teammate、Mailbox、TaskList / TaskStop、PR activity 本地订阅闭环均已落地；tmux / iTerm2 等外部 swarm 后端仍按当前实现范围保留为裁剪项。 |
| `sub-agents.mdx` | `已实现` | `engine/agent/mod.rs:30-62,247-356`、`tool_impl.rs:88-229`、`dispatch.rs:168-238`、`supervisor.rs:74-189`、`ipc/builtin_agents.rs:1-180`、`ipc/agent_settings.rs:180-245,545-583` | 子 Agent 入口、内置 agent、agent 定义工具过滤、background / sync 生命周期、hooks、AgentTree、worktree 隔离和 fork 侧路都已落地。 |
| `worktree-isolation.mdx` | `部分实现` | `worktree_hooks.rs`、`tools/worktree.rs:1-520`、`engine/agent/worktree.rs:1-342`、`engine/agent/supervisor.rs:546-762`、`engine/agent/mod.rs:104-156`、`cc-config/src/paths.rs` | 有用户会话 worktree 工具、子 Agent worktree 隔离、`WorktreeCreate` / `WorktreeRemove` hook 接线和 `{CC_RUST_HOME}/worktrees` 隔离路径；sparse checkout、session restore 与 Bun 的 `.claude/worktrees` 同构布局仍未实现。 |

## 逐文档分析

### coordinator-and-swarm.mdx

- 上游核心主题是 Coordinator Mode、Agent Swarm、Mailbox、TaskList / TaskStop、以及 PR activity 订阅。
- `cc-rust` 已经实现了一个更偏“in-process swarm”的替代面：`teams/mod.rs:11-44` 说明它把 Agent Teams 作为单独模块组织起来；`teams/mod.rs:74-84` 提供 `is_agent_teams_enabled()` 和 `is_agent_teams_active()`，把 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 和会话级 `team_context` 作为激活条件。
- `/team` 入口已经接上：`commands/team_cmd.rs:43-435` 覆盖 `create`、`spawn`、`send`、`kill`、`leave`、`delete`，并把 active team 镜像回 `AppState::team_context`。
- `TeamSpawn` 已接到 Agent 侧：`tools/team_spawn.rs:26-359` 负责创建 in-process teammate、写入 team file、更新 `AppState::team_context`，并且默认只支持 `in-process` backend；`tools/registry.rs:42-79` 也把 `TeamSpawnTool`、`SendMessageTool`、`TaskListTool`、`TaskStopTool` 注册进工具池。
- `SendMessage` 已实现 mailbox 定向与广播能力：`tools/send_message.rs:23-435` 负责路由消息到队友 mailbox；`teams/mailbox.rs:71-177` 负责读写、锁和清理；`teams/runner.rs:281-448` 负责轮询 mailbox、处理协议消息和空闲通知。
- `TaskList` / `TaskStop` 已实现：`tools/tasks.rs:2086-2198` 提供任务列表与取消，配合 `supervisor.rs:74-189` 把后台 Agent 绑定到持久任务。
- `plan` 相关的审批链路也有对应：`tools/plan_mode.rs` 和 `teams/runner.rs:330-386` 支持计划审批消息往返。
- 原缺口在于 Bun 的独立 coordinator 机制没有在 Rust 源码里出现。Phase 1-4 已补 `Coordinator` feature gate、`CLAUDE_CODE_COORDINATOR_MODE`、coordinator prompt / session override、`/coordinator` 入口、worker 默认 agent，以及 `subscribe_pr_activity` / `unsubscribe_pr_activity` 的本地可测事件流；Bun 的 coordinator gate / session-mode sync / worker context / system prompt 仍作为语义对照来源。
- Bun 的 coordinator 工具白名单由 `src/constants/tools.ts:105-110` 定义为 `Agent`、`TaskStop`、`SendMessage`、`SyntheticOutput`，并在 `src/utils/toolPool.ts:35-40` 额外放行 `subscribe_pr_activity` / `unsubscribe_pr_activity` 后缀。Rust 侧已在 `tools/registry.rs` 用 `ToolPolicy` 区分 default / coordinator / worker / teammate 工具池；其中 `SyntheticOutput` 当前没有同构工具。
- Bun 的 coordinator worker 专用 agent 在 `src/coordinator/workerAgent.ts:41-67` 定义，工具集来自 `ASYNC_AGENT_ALLOWED_TOOLS` 减去内部编排工具。Rust 侧 `ipc/builtin_agents.rs:27-140` 没有 `worker` 内置 agent；`teams/runner.rs:89-98` 给 in-process teammate 装配 `get_all_tools()`，因此没有 Bun 那套 team lead / worker 工具白名单分层。
- PR 订阅精确行为已补成本地可测闭环：`tools/pr_activity.rs` 暴露 `subscribe_pr_activity` / `unsubscribe_pr_activity`，coordinator 工具策略放行这两个工具，订阅状态写入 `{CC_RUST_HOME}/pr-activity-subscriptions.json`，daemon 的 `/webhook/github` 会按 pull request / review / issue comment 事件匹配订阅并投递 mailbox；真实 GitHub App / MCP 传输仍是后续集成点。
- 另一个显著差异是后端策略：`teams/mod.rs:39-44` 明确把当前实现收敛到 `in_process::InProcessBackend`，因此 Bun 文档里的 tmux / 多终端 swarm 路径属于 `故意裁剪`。

### sub-agents.mdx

- 上游主题是 `AgentTool.call()` 的完整链路、fork 子进程、内置 agent、hook 注入、后台生命周期、worktree 隔离和结果回传。
- `AgentTool` 本体已经存在：`engine/agent/mod.rs:30-62` 定义了 `AgentTool`、`MAX_AGENT_DEPTH`、`subagent_type`、`run_in_background`、`name`、`mode` 和 `isolation` 等输入字段。
- 命名 teammate 的分支已经接入：`engine/agent/tool_impl.rs:112-138` 会先判断 `teammate_spawn_request()`，命中后转到 `TeamSpawnTool`，这条路径直接对应上游文档里的“命名 agent / teammate”能力。
- 內置 agent 已经注册：`ipc/builtin_agents.rs:1-180` 列出 `general-purpose`、`Explore`、`Plan`、`code-reviewer`、`statusline-setup`；`ipc/agent_settings.rs:545-583` 还把 `ExitPlanMode`、`SendMessage`、`TeamSpawn` 等名字预置进可见工具集合。
- 工具池和权限边界是独立组装的：`tools/registry.rs:42-79` 把 `Agent`、`EnterPlanMode`、`ExitPlanMode`、`TaskList`、`TaskStop`、`SendMessage`、`TeamSpawn` 放进默认注册表；`tools/send_message.rs:144-155` 和 `tools/team_spawn.rs:138-167` 也分别对 team 上下文或 backend 支持范围做了校验。
- 子 Agent 工具过滤已补充核查：`engine/agent/mod.rs:247-356` 会从完整 registry 取工具后，根据 `active_agent_definition()` 的 `tools` / `disallowed_tools` 做 allow / deny 过滤；`builtin_agents.rs:48-109` 中 `Explore`、`Plan`、`code-reviewer` 都限制为 `Glob`、`Grep`、`Read`，测试 `builtin_explore_agent_receives_only_read_only_tools()` 锁定该行为（`engine/agent/mod.rs:474-487`）。
- 生命周期钩子已接入：`engine/agent/dispatch.rs:168-238` 显式触发 `SubagentStart` / `SubagentStop`；`engine/agent/supervisor.rs:100-154`、`428-463` 在后台路径里也会再次触发这些 hook。
- 后台 Agent 已实现：`engine/agent/tool_impl.rs:190-229` 进入 `spawn_background_agent()`；`engine/agent/supervisor.rs:74-189` 负责注册 task、创建 cancellation token、绑定树节点；`supervisor.rs:374-463` 负责结束、结果、树快照和 hook 收尾。
- `worktree` 作为子 Agent 隔离模式也已经纳入：`engine/agent/worktree.rs:1-420` 负责 worktree 内执行、结果保留/清理和失败回退。
- 轻量 fork 侧路也存在：`engine/agent/fork.rs:1-130` 提供无持久化的子引擎执行，支持 `parent_messages` 以复用上下文。
- 因此，这份 Bun 文档里的“子 Agent 机制”在 Rust 侧整体可视为 `已实现`；但如果要追求与 Bun 的路径分层、门控和 prompt-cache 细节逐项一致，仍需要后续复核。

### worktree-isolation.mdx

- 上游文档强调 `.claude/worktrees/<slug>` 的目录结构、`WorktreeCreate` / `WorktreeRemove` hook、`EnterWorktreeTool` / `ExitWorktreeTool` 以及 fail-closed 的删除逻辑。
- `cc-rust` 已经有 worktree 运行面：`engine/agent/worktree.rs:1-420` 明确负责“Git worktree execution for agent isolation”，包含创建、运行、结果回传和清理。
- `AgentTool` 的输入已经暴露 `isolation: "worktree"`：`engine/agent/mod.rs:56` 和 `tool_impl.rs:44` 一起说明 worktree 是 Agent 的正式隔离模式之一。
- 运行时 worktree 采用临时目录和自动分支命名：`worktree.rs:86-106` 生成 `agent-worktree-{id}`，`worktree.rs:275-342` 根据变更决定保留或清理。
- 后台 Agent 也复用同一套 worktree 生命周期：`supervisor.rs:546-762` 先准备 worktree，再在退出时按变更数决定保留或清理。
- 变更统计是 fail-closed 的：`engine/agent/mod.rs:104-156` 的 `count_worktree_changes()` 依赖 `git status` / `git rev-list`，失败时返回 `None`，后续清理逻辑不会冒险删除。
- 用户会话层也已有 `EnterWorktree` / `ExitWorktree` 工具：`tools/worktree.rs:203-285` 创建不可嵌套的 git worktree，`tools/worktree.rs:350-412` 在删除前执行 fail-closed 变更检查，`tools/worktree.rs:426-520` 支持 `keep` 或 `remove`。Phase 5 后默认目录使用 `{CC_RUST_HOME}/worktrees/cc-worktree-{slug}`，仍不使用 Bun 的 `.claude/worktrees/<slug>` 与 `worktree/<slug>` 布局。
- Bun 的 hook-based worktree 链路位于 `src/utils/worktree.ts:715-727`、`825-827`、`912-918`、`967-973`，会通过 `WorktreeCreate` / `WorktreeRemove` 替代 git 创建/删除。Rust 侧已新增共享 `worktree_hooks.rs`，并在 `tools/worktree.rs`、`engine/agent/worktree.rs`、`engine/agent/supervisor.rs` 接入 create/remove hook；hook 缺失时回退 git create/remove，hook remove 未明确成功、路径越界或清理无法验证时保留 worktree。
- 这意味着“子 Agent worktree 隔离”和 hook 替代链路都已存在，但 Bun 文档里的 `.claude/worktrees` 同构布局、sparse checkout 与 session restore 仍未一一对齐，因此整章结论仍应记为 `部分实现`。

## 补充核查：文件与行为

### 文件核查收敛

| 原待确认点 | Bun 侧文件证据 | cc-rust 侧文件证据 | 收敛结论 |
| --- | --- | --- | --- |
| coordinator 专用 prompt / gate | `coordinatorMode.ts:36-41` 定义 `COORDINATOR_MODE` + `CLAUDE_CODE_COORDINATOR_MODE` gate；`coordinatorMode.ts:80-109` 注入 worker 工具上下文；`coordinatorMode.ts:111-369` 生成 coordinator prompt。 | `cc-config/src/features.rs` 新增 `Coordinator` gate（`CLAUDE_CODE_COORDINATOR_MODE`）；`teams/coordinator.rs` 集中 coordinator prompt / session override；`engine/system_prompt.rs` 按 gate 注入 coordinator section；`commands/coordinator.rs` 提供运行时启停入口。 | `已实现` |
| coordinator / worker 工具白名单分层 | `constants/tools.ts:55-71` 定义 async agent 工具；`77-86` 定义 teammate 专属任务/消息工具；`105-110` 定义 coordinator 工具；`workerAgent.ts:35-45` 从 async 工具里剔除内部编排工具。 | `tools/registry.rs` 新增 `ToolPolicy` 分层并限制 coordinator lead / worker 工具；`ipc/builtin_agents.rs` 新增 `worker` 内置 agent；`teams/runner.rs` 和 in-process teammate 创建链路按策略注入工具。 | `部分实现`：coordinator/worker 分层已落地；上游 `SyntheticOutput` 在 Rust 当前工具面中没有同构能力。 |
| PR activity subscription 精确行为 | `toolPool.ts:8-18,35-40` 允许 `subscribe_pr_activity` / `unsubscribe_pr_activity` 后缀；`coordinatorMode.ts:128-133` 明确提示 coordinator 直接管理 PR 订阅。 | `tools/pr_activity.rs` 实现订阅/取消订阅和事件匹配；`tools/registry.rs` 只在 coordinator 策略暴露订阅工具；`daemon/routes.rs` 的 `/webhook/github` 路由 GitHub PR/review/comment 事件到 mailbox；状态写入 `{CC_RUST_HOME}/pr-activity-subscriptions.json`。 | `部分实现`：本地 webhook + store + mailbox 闭环已实现；真实 GitHub App / MCP 传输仍是后续集成点。 |
| WorktreeCreate / WorktreeRemove hook 对照 | `utils/worktree.ts:715-727`、`912-918` 优先执行 `WorktreeCreate`；`825-827`、`967-973` 对 hook-based worktree 执行 `WorktreeRemove`。 | `worktree_hooks.rs` 定义共享 schema 与路径边界；`tools/worktree.rs`、`engine/agent/worktree.rs`、`engine/agent/supervisor.rs` 在创建/删除路径接入 `WorktreeCreate` / `WorktreeRemove`；`cc-config/src/paths.rs` 新增 `{CC_RUST_HOME}/worktrees`。 | `已实现`：hook create/remove 替代链路已接线；sparse checkout / session restore 仍按 worktree 章节记录为未实现。 |

### 行为核查收敛

| 行为面 | cc-rust 当前行为 | 对齐判断 |
| --- | --- | --- |
| `/team` 命令族 | `team_cmd.rs:43-52` 分派 `status/list/create/spawn/send/kill/leave/delete`；`create` 写入并激活 `TeamContext`；`spawn` 创建 in-process teammate；`kill/delete` 通过 `InProcessBackend` 停止任务并更新 team file。 | 覆盖 Agent Teams 基础生命周期；不等同 Bun coordinator mode。 |
| `TeamSpawn` | `team_spawn.rs:165-267` 校验只支持 `in-process`、按显式/当前/隐式 team 创建成员并启动 backend；`281-352` 回写 `AppState::team_context` 并返回 `agent_id` / `task_id`。 | 对齐 in-process teammate spawn；不支持 tmux/iTerm2 backend。 |
| `SendMessage` | `send_message.rs:144-192` 无 active team 时返回错误；`215-262` 定向写 mailbox；`264-308` 广播到非自己且 active 的成员；`371-435` 支持计划审批 request/response 协议。 | mailbox 行为已实现；PR 订阅消息不是该工具能力。 |
| Teammate runner | `runner.rs:89-98` 创建带完整工具池的 child `QueryEngine`；`121-130` 先执行初始 prompt；`288-320` 轮询未读 mailbox 并把普通消息转成下一轮 prompt；`331-367` 对 shutdown request 自动批准。 | in-process loop 已实现；没有 Bun 的 worker 工具裁剪或独立 coordinator prompt。 |
| Task 分发 / 认领 | `tasks.rs:421-476` claim 时检查 owner、已解决状态、依赖阻塞和同 owner busy；`1972-2029` 暴露 `owner`、`check_agent_busy` / `checkAgentBusy`；`2086-2198` 提供 `TaskList` / `TaskStop`。 | task store / claim 基础行为已实现；跨进程强一致竞争锁仍按 tools 映射表记为部分实现。 |
| Worktree 清理 | `tools/worktree.rs:350-412` 删除前 fail-closed；`engine/agent/worktree.rs:275-342` 和 `supervisor.rs:665-762` 对子 Agent worktree 按变更保留或清理。 | 核心安全行为已实现；路径、session restore、sparse checkout 与 hook-based 创建/删除不对齐。 |

## 已实现汇总

- `AgentTool` 已有完整输入模型，包含 `subagent_type`、`run_in_background` 和 `isolation`：`engine/agent/mod.rs:30-62`。
- 内置 subagent registry 已存在：`ipc/builtin_agents.rs:1-180`。
- `SubagentStart` / `SubagentStop` hooks 已接入：`engine/agent/dispatch.rs:168-238`、`supervisor.rs:100-154`、`428-463`。
- 后台 Agent 生命周期已接入 task store 和 AgentTree：`supervisor.rs:74-189`、`497-541`。
- `SendMessage`、`TeamSpawn`、`TaskList`、`TaskStop` 已在工具注册表中：`tools/registry.rs:42-79`。
- mailbox 读写与队友消息流转已实现：`teams/mailbox.rs:71-177`、`teams/runner.rs:281-448`。
- worktree 隔离已支持，并且有结果保留 / 清理分支：`engine/agent/worktree.rs:1-420`。

## 未实现 / 部分实现 / 待确认 / 故意裁剪

- `未实现`：真实 GitHub App / MCP PR activity 传输、worktree sparse checkout、worktree session restore，以及 Bun `.claude/worktrees` 的完全同构路径布局。
- `部分实现`：`coordinator-and-swarm.mdx` 里的 swarm 语义在 Rust 侧是“in-process teammate + mailbox + task store”版本，和 Bun 的 coordinator / swarm 分层不是同构实现。
- `部分实现`：普通子 Agent 的 `tools` / `disallowed_tools` 过滤、coordinator lead / worker 工具白名单分层、worker 内置 agent 已实现；上游 `SyntheticOutput` 在 Rust 当前工具面中没有同构能力。
- `部分实现`：`worktree-isolation.mdx` 的 hook create/remove 与 `{CC_RUST_HOME}/worktrees` 隔离根已实现；sparse checkout、session restore 与 Bun `.claude/worktrees` 完全同构布局仍未实现。
- `故意裁剪`：`teams/mod.rs:39-44` 已明确只保留 `in_process::InProcessBackend`，因此 tmux / iTerm2 一类后端不在当前实现范围内。
- `待确认`：当前未发现需要继续保留为 `待确认` 的 agent 核心项。原 `待确认` 的 coordinator prompt、team lead / worker 工具白名单、PR 订阅行为、WorktreeCreate / WorktreeRemove hook 对照已在本轮收敛并按 Phase 1-5 落地或明确列为后续未实现项。

## 实现阶段建议

### 当前实现进度

- 2026-05-06：Phase 0 已开始落地，新增 `TeamSpawn` unsupported backend、`SendMessage` active team / 定向 / 广播、runner mailbox plain / shutdown、`TaskList` / `TaskStop` cancel flow、worktree fail-closed 相关回归测试；同时把依赖全局 worktree session 的测试标为串行，避免并行污染。
- 2026-05-06：Phase 1 已落最小骨架，新增 `Coordinator` feature gate（`CLAUDE_CODE_COORDINATOR_MODE`）、`teams::coordinator` prompt builder，并在默认 system prompt 中按 gate 注入 `# Coordinator Mode` section。
- 2026-05-06：Phase 2 已完成第一轮落地，新增 `ToolPolicy` 分层（`DefaultAgent` / `Coordinator` / `CoordinatorWorker` / `InProcessTeammate`），主会话在 coordinator gate 开启时切换到 lead 工具池，in-process teammate runner 切换到受限工具池，并在 coordinator 模式下默认用 `worker` agent prompt / 工具边界启动 teammate。
- 2026-05-06：Phase 3 已完成最小运行时闭环，新增 `/coordinator` 命令入口，可在当前会话启停 coordinator gate、创建/绑定 active team、展示 lead 工具策略；`/team spawn` 与 `TeamSpawn` 在 coordinator 模式下默认创建 `worker` teammate，并继续复用现有 `SendMessage` / `TaskList` / `TaskStop` 通信和任务控制链路。
- 2026-05-06：Phase 4 已完成本地可测的 PR activity subscription 闭环，新增 `subscribe_pr_activity` / `unsubscribe_pr_activity` coordinator-only 工具、`{CC_RUST_HOME}/pr-activity-subscriptions.json` 隔离存储，以及 `/webhook/github` 对 pull request / review / issue comment 事件的订阅匹配和 mailbox 投递；真实 GitHub App / MCP 传输仍作为后续集成点。
- 2026-05-06：Phase 5 已完成 worktree hook parity 的可测骨架，新增 `WorktreeCreate` / `WorktreeRemove` 事件入口和共享 hook schema，用户 `EnterWorktree` / `ExitWorktree`、同步 Agent worktree、后台 supervisor worktree 创建/清理路径均接入 hook；默认 worktree 根切换到 `{CC_RUST_HOME}/worktrees`，删除路径在路径越界、hook remove 未明确处理或清理无法验证时保留 worktree。
- 2026-05-06：Phase 6 已完成 coordinator/team/tasks headless 回归、PR webhook 模拟回归与 worktree hook 实际路径回归，并同步收口 `WORK_STATUS.md` / `IMPLEMENTATION_GAPS.md`。

### Phase 0：锁定现有行为基线

目标：先保护已经实现的 in-process team、mailbox、普通 subagent 工具过滤和 worktree 清理行为，避免后续补 coordinator 时回归。

- 为 `TeamSpawn`、`SendMessage`、`TaskList` / `TaskStop` 补最小回归测试，覆盖 active team 校验、定向消息、广播、任务停止和 unsupported backend。
- 为 `teams/runner.rs` 补 teammate mailbox loop 的单元或集成测试，覆盖普通消息、plan approval 协议、shutdown request 自动批准。
- 保留 `engine/agent/mod.rs` 中普通 subagent `tools` / `disallowed_tools` 过滤测试，并增加一个 deny-list 回归用例。
- 为 `tools/worktree.rs` 和 `engine/agent/worktree.rs` 补 fail-closed 删除测试，确认 `git status` / `rev-list` 失败时不删除 worktree。

验收：现有 Agent Teams、普通 subagent、worktree 隔离行为被测试锁住；不改变任何生产逻辑。

### Phase 1：引入 Coordinator 模式骨架

目标：补齐 Bun 的 coordinator gate、session mode 和 system prompt，但先不改变 worker 工具权限。

- 在 `cc-config/src/features.rs` 增加 coordinator feature gate，环境变量建议与上游兼容为 `CLAUDE_CODE_COORDINATOR_MODE`，同时保留 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 作为 team 能力 gate。
- 在 `teams/mod.rs` 或新的 `teams/coordinator.rs` 中集中放置 coordinator 激活判断、session mode 状态和 prompt 构造入口。
- 将 Bun `coordinatorMode.ts:111-369` 的 coordinator prompt 语义迁移为 Rust prompt builder，重点包含 worker context、任务分发策略、mailbox 使用规则、TaskList / TaskStop 使用规则。
- 在 QueryEngine 系统提示词组装处接入 coordinator prompt；未开启 gate 时保持现有系统提示词不变。

验收：开启 coordinator gate 后，系统提示词包含 coordinator 专用指导；关闭 gate 时现有 team / subagent 行为和 prompt 快照不变。

### Phase 2：补齐工具白名单分层

目标：把“普通 subagent 工具过滤已实现”和“coordinator / worker 分层缺失”拆开，建立明确的工具策略。

- 在工具注册或 agent 构造链路中引入工具策略枚举，例如 `DefaultAgent`、`Coordinator`、`CoordinatorWorker`、`InProcessTeammate`。
- 为 coordinator 限制工具集：`Agent`、`SendMessage`、`TaskStop`，以及后续 PR subscription 工具；如 Rust 没有 `SyntheticOutput` 等价能力，先记录为不适配项。
- 在 `ipc/builtin_agents.rs` 增加 `worker` 内置 agent，按 Bun worker 语义给出专用 prompt 和工具 allow-list。
- 修改 `teams/runner.rs`，不要继续给 teammate 无条件注入完整 `get_all_tools()`；改为按 teammate / worker 策略过滤。
- 保持普通 subagent 的 agent definition allow / deny 过滤不变，避免影响 `Explore`、`Plan`、`code-reviewer`。

验收：coordinator、worker、普通 subagent、in-process teammate 的可见工具集合各自有测试；worker 不再获得内部编排或高风险无关工具。

### Phase 3：打通 Coordinator 运行时闭环

目标：让 coordinator 不只是 prompt，而是能稳定调度 team worker、发送消息、查看/停止任务。

- 增加 coordinator 入口路径，可由启动 gate、显式命令或 headless IPC 模式激活；入口只负责进入 coordinator session，不直接替代 `/team` 基础命令。
- 复用已有 `TeamSpawnTool` 创建 worker，但将默认 agent 类型切到 Phase 2 新增的 `worker`。
- 复用 `SendMessage` mailbox 作为 coordinator 和 worker 的通信面，确保定向、广播、plan approval 协议和 shutdown request 行为兼容。
- 复用 `TaskList` / `TaskStop` 做任务可见性和取消，不新增平行 task store。

验收：一个 coordinator session 可以 spawn worker、给 worker 发消息、读取任务状态、停止 worker；关闭 coordinator 后原 `/team` 命令族仍可独立工作。

### Phase 4：实现 PR Activity Subscription

目标：补齐 `subscribe_pr_activity` / `unsubscribe_pr_activity` 的工具面和事件流，不把 `/review` prompt 当作订阅实现。

- 新增 PR subscription 工具模块，并只在 coordinator 工具策略中暴露；普通 agent 默认不可见。
- 明确订阅状态存储位置，优先放在 `.cc-rust/` 隔离路径下，避免写入原版 Codex / Claude 路径。
- 将 daemon 的 GitHub webhook stub 扩展为可路由事件：校验签名、识别 PR、匹配订阅、写入 coordinator mailbox 或事件队列。
- `unsubscribe_pr_activity` 需要幂等，重复取消不存在的订阅不应破坏其他订阅。
- 如果本阶段无法接入真实 GitHub App / MCP transport，先实现本地可测的 subscription store + webhook routing，再把外部接入标为后续集成点。

验收：订阅、重复订阅、取消订阅、webhook 命中、webhook 未命中、签名失败都有测试；coordinator 能收到 PR activity 通知。

### Phase 5：补齐 Worktree Hook Parity

目标：在保留现有 fail-closed 安全逻辑的前提下，补 `WorktreeCreate` / `WorktreeRemove` hook 替代链路。

- 在 hooks 事件模型中增加 `WorktreeCreate` / `WorktreeRemove` 事件，并定义输入输出 schema。
- 在 `tools/worktree.rs`、`engine/agent/worktree.rs`、`engine/agent/supervisor.rs` 的创建和删除路径中接入 hook：hook 可用且成功时采用 hook 返回路径；不可用时回退现有 git worktree 流程。
- 路径布局不要直接写入原版路径；默认应使用 `.cc-rust/worktrees/<slug>` 或现有 cc-rust 隔离目录。若必须兼容 Bun 的 `.claude/worktrees/<slug>`，需要显式配置开关和文档说明。
- 删除路径必须继续 fail-closed：无法确认变更数、hook remove 失败、路径越界时均保留 worktree 并返回人工处理信息。
- 评估是否补 sparse checkout 和 session restore；如果不做，明确列为未实现而不是待确认。

验收：hook create、hook remove、hook 缺失 fallback、hook 失败 fallback、路径越界、变更未清理等场景都有测试；现有 worktree 清理安全行为不回退。

### Phase 6：端到端验证与文档收口（已完成）

目标：把 coordinator、worker、PR subscription、worktree hook 的新增行为串成可回归的用户路径。

- Headless 回归已补齐：`crates/claude-code-rs/tests/e2e_terminal/phase6.rs` 覆盖 coordinator 启停、worker spawn/send、`/tasks` 列表与 mailbox 落盘。
- PR subscription / webhook 回归已覆盖：`crates/claude-code-rs/src/daemon/routes.rs` 的 GitHub webhook 测试验证 PR activity 订阅匹配与 mailbox 投递。
- worktree hook 回归已补齐：`crates/claude-code-rs/src/tools/worktree.rs` 覆盖 hook-backed create/remove 以及 git fallback 两条路径。
- 文档已同步：`docs/WORK_STATUS.md` 记录 phase 6 完成口径，`docs/IMPLEMENTATION_GAPS.md` 记录 phase 6 不再视为 gap。
- 最终验证已执行：`cargo fmt --all`、定向 `cargo test`、`cargo build --release`。

验收：新增功能有测试证据，文档状态与实现一致，release build 无 warning。

## 后续动作

1. 如果后续要继续补 `context`、`extensibility`、`safety`、`tools` 四章，沿用同一模板分别生成映射表。
2. 如果要追求与 Bun coordinator 完全同构，下一步不是继续核查，而是补设计决策：是否引入 `COORDINATOR_MODE` gate、coordinator prompt、coordinator tool filter、worker 内置 agent 与 PR activity subscription MCP 适配。
3. 如果要追求与 Bun worktree 章节完全同构，下一步是补实现设计：`.claude/worktrees/<slug>` 路径、`worktree/<slug>` 分支、session restore、sparse checkout。
