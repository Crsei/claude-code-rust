# cc-rust 未完备项与全量构建 TODO

> **阶段切换 (2026-04-22)**：本仓库已从 "rust-lite 精简版" 切换到 **全量构建 (Full Build)** 阶段。
> 本文原先承担的角色是"登记已接受的缩减/延期"，现在重新定义为：**对上游完整版尚未对齐的 TODO 清单**。
> 原 §2、§5 中的条目默认视为待补齐，不再等于"不做"。具体规则见 [`../CLAUDE.md`](../CLAUDE.md) 顶部"当前阶段"说明。
>
> 本文把 `docs/` 中分散的"缩减实现 / 设计限制 / 未完备项"集中到一个入口。
> 当前完成度基线看 [`WORK_STATUS.md`](WORK_STATUS.md)，用户可感知问题看 [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md)，历史已完成方案与变更记录已归档到 [`archive/`](archive/)。
>
> 若要看与 `claude-code-bun` 的对标差异、`run in chrome` 判断、Web UI 评估和 REPL 结构规划，见 [`claude-code-bun-gap-plan.md`](claude-code-bun-gap-plan.md)。

## 1. 当前仍未完成或仅部分完成

| 范围 | 当前状态 | 说明 |
|------|----------|------|
| API providers | 未完成 (单独立项) | Bedrock、Vertex 仍未实现；由独立 issue 追踪 |
| Team Memory 客户端同步 | 未实现 | 服务端代理已落地 (`src/daemon/team_memory_proxy.rs` + `ui/team-memory-server/`)；前端尚未调用，计划见 `superpowers/plans/2026-04-11-team-memory-sync.md` |

> 以下项在历史文档中曾标注为 stub，经代码核对已在 `rust-lite` 分支中收口，保留在本节做历史追踪：
>
> - **IPC `clear_messages`** — 已由 `QueryEngine::clear_messages()` (`src/engine/lifecycle/mod.rs:245`) 实现，`/clear` 路径在 `src/ipc/ingress.rs:332-339` 调用 engine 清空并回传 `conversation_replaced`。
> - **权限 Phase 2 Hook 拦截** — `src/tools/execution/pipeline.rs:124-211` 先跑 `run_pre_tool_hooks`，再把结果折进 `has_permissions_to_use_tool_with_hook` (`src/permissions/decision.rs:259-362`)，hook 的 deny/ask/allow 会按规范顺序生效。
> - **Vim 状态机** — `ui/src/vim/state-machine.ts` 已覆盖 normal/insert/visual 三模式、导航 (h/l/0/$/^/w/b/e)、operator (d/y/c)、单键 (x/X/p/u/D/C) 与 visual 选区操作；KNOWN_ISSUES 中目前无相关 open 项。
> - **Agent Teams 用户面** — `/team` 斜杠命令 + `TeamSpawn` 工具 + Team Dashboard 已落地，详见 §1.1。

### 1.1 Agent Teams 收口状态

rust-lite 对 Agent Teams 的最终收口是"**in-process 闭环 + 用户面全量**"：

- **闭环核心** — `src/teams/` 的 10 个子模块 (types/protocol/mailbox/context/identity/in_process/helpers/constants/runner/backend) 驱动同进程多代理 mailbox，teammate 作为 tokio 任务在 `task_local!` 身份隔离下运行。
- **工具层** — `SendMessage` 工具处理消息路由和协议消息；`TeamSpawn` 工具 (`src/tools/team_spawn.rs`) 让模型从对话里直接拉起新 teammate，必要时自动创建 session 绑定的团队。
- **REPL 层** — `/team` 斜杠命令家族 (`src/commands/team_cmd.rs`) 覆盖 `create / list / status / spawn / send / kill / leave / delete`。
- **UI 层** — `ui/src/components/TeamPanel.tsx` 订阅 `BackendMessage::TeamEvent`，展示活跃 team、成员在线状态、未读计数、最近消息。
- **启用条件** — `is_agent_teams_active(app_state)` 同时接受 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` env var 与 `AppState::team_context` 存在两种启用方式，后者让 `/team create` 或 `TeamSpawn` 调用在会话内就能解锁 team 功能。

**rust-lite 中明确不实现**：tmux / iTerm2 终端后端。`backend::PaneBackend` trait 作为完整版接口占位保留，in-process 是唯一执行后端。需要终端分屏的用户请用完整版 claude-code。

## 2. 全量构建待补齐 TODO（原「已完成但仍为缩减实现」）

> **状态反转**：下表条目**不再**被视为"已接受的 Lite 缩减"。它们是全量构建阶段需要按上游对齐的 TODO。触及以下模块时，默认按上游完整行为补齐，而不是"保持现状"。详细的原版代码路径/行数对照见 [`archive/COMPLETED_SIMPLIFIED.md`](archive/COMPLETED_SIMPLIFIED.md)。

| 模块 | 待补齐的行为（参考上游） |
|------|----------|
| BashTool | PowerShell 分支、sandbox、进程组管理、危险命令拒绝列表、heredoc 校验、Git 操作跟踪 |
| FileEditTool | diff 预览、冲突检测、文件锁检查、编辑历史、自动缩进修正 |
| FileWriteTool | 临时文件后 rename 的安全写入、备份/恢复、大小限制、权限保持、二进制内容检查 |
| FileReadTool | 符号链接解析、大文件智能分页、文件编码检测 |
| SkillTool | 依赖解析、热重载、版本管理、完整 frontmatter 校验、MCP skill builder |
| TaskTools | 磁盘持久化、依赖图、后台任务管理、超时控制、独立 UI |
| ToolSearchTool | TF-IDF 排名、全文索引、deferred schema 加载 |
| PlanMode | auto-mode/classifier gate、团队审批流、计划持久化、实现关联跟踪 |
| LSP | `didChange` 增量同步、`publishDiagnostics` 被动反馈、补全建议、插件侧配置整合 |
| WebFetch | JS 渲染、Cookie 管理、代理支持、重定向限制、Content-Type 智能处理 |
| AgentTool | 多后端 spawn (tmux/iTerm2)、团队上下文集成、spawnMultiAgent、工具白名单过滤、background 模式完整化、工具定义去重 |

补齐流程：
1. 读上游实现（`F:\AIclassmanager\cc\src\tools\<name>\**` 或 `claude-code-bun` 同名模块）。
2. 改 Rust 端实现，补测试。
3. 条目从本表删除，迁移到 [`archive/COMPLETED_FULL.md`](archive/COMPLETED_FULL.md)。
4. 同步更新 [`archive/COMPLETED_SIMPLIFIED.md`](archive/COMPLETED_SIMPLIFIED.md) 里该模块的状态行。

如某项确实要"故意保留缩减"（平台差异、许可、明确裁剪），把它从本表移到新的 §7 "Intentional 裁剪"，并在 PR 说明理由——不要悄悄留在本节。

## 3. 已知设计限制与运行时 caveats

这些问题已经在文档中明确记录，但尚未补齐：

| 范围 | 状态 | 说明 |
|------|------|------|
| UI resize 回流 | Open | 终端缩放后内容不会可靠重排 |
| 窄终端欢迎页布局 | Open | Tips 文本截断、ASCII logo 破碎 |
| Background agent + worktree | Open（设计限制） | `run_in_background` 与 `isolation: "worktree"` 不能同时生效 |
| Background agent 权限回调 | Open（设计限制） | 子引擎无 `permission_callback`，默认模式下需 Ask 的工具会被直接拒绝 |
| Background agent 取消 | Open（设计限制） | 未保存 `JoinHandle`，用户 abort/退出时后台代理不会被统一取消 |

## 4. 仍在进行或仅有方案文档的工作

以下文档仍是活跃入口，不应归档为“已完成”：

- [`computer-use-implementation-checklist.md`](computer-use-implementation-checklist.md)：Computer Use 落地清单，仍是待实施能力
- [`session-export-implementation-guide.md`](session-export-implementation-guide.md)：目标明确，但 Rust 侧仍缺基础设施
- [`ipc-refactor-plan.md`](ipc-refactor-plan.md)：IPC 结构重构计划，尚未完全收束
- [`traceable-logging-plan.md`](traceable-logging-plan.md)：可追溯日志体系，仍是 Draft
- [`superpowers/plans/2026-04-11-team-memory-sync.md`](superpowers/plans/2026-04-11-team-memory-sync.md)：Team Memory 客户端同步待做
- [`superpowers/specs/2026-04-11-team-memory-sync-design.md`](superpowers/specs/2026-04-11-team-memory-sync-design.md)：对应设计仍是现行参考
- [`superpowers/plans/2026-04-12-tools-commands-test-coverage.md`](superpowers/plans/2026-04-12-tools-commands-test-coverage.md)：测试覆盖补齐计划仍有效
- [`superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md`](superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md)：PTY 交互测试扩展计划仍有效

## 5. 历史 Deferred 清单（进入全量构建后需要重评）

> **状态反转**：下列能力原先被登记为"`rust-lite` 明确不做"。进入全量构建阶段后，这些**不再自动等于"不实现"**——任何触及它们的新工作默认按上游完整版对齐，除非重新评估后确认保留延期并写入 §7 "Intentional 裁剪"。

原 lite 延期范围，保留作为历史对照：

- 远程控制与多端集成：`/remote-control`、`/desktop`、`/mobile`、`bridge/`、`remote/`
- 服务端/传输扩展：`server/`、SSE/WebSocket/Worker transport、MCP server mode
- 远程/运营能力：`Monitor`、`PushNotification`、`SubscribePR`、`Workflow`、遥测与 MDM 同步
- Ant-only 命令与内部工具：`/agents-platform`、`/ant-trace`、`CtxInspect`、`OverflowTest`、`Tungsten` 等

完整列表以 [`WORK_STATUS.md`](WORK_STATUS.md) §3 为准。重评时按模块类别单独判断：
- **Ant-only 内部工具**：继续不实现的可信度高；仍建议写入 §7。
- **远程/多端/服务端扩展**：默认按上游补齐；如决定延期，必须有明确理由与截止期。
- **遥测 / MDM / analytics**：看后续产品路线重评，不再默认视为"永远不做"。

## 6. 代码层技术债入口

`TECH_DEBT.md` 里的这些问题仍然是活跃债务来源：

- `#![allow(unused)]` 清理仍未做完
- `test_ctx()` 等测试样板仍未完全收束到共享 helper
- 模型别名映射仍未完全合并到单一查找表
- 工具输入解析风格仍不统一
- IPC 协议仍缺显式版本策略

如果只想看"现在不能指望它已经完善"的地方，优先读本文；如果要进入修复实施，再去看对应的源文档。

## 7. Intentional 裁剪（明确不跟随上游）

> 本节用于登记**全量构建阶段确认不跟随上游**的裁剪项。与 §2 / §5 的区别：§2 / §5 是"尚未对齐的 TODO"，本节是"经评估后决定不做"。

条目格式：`- <模块/功能>：裁剪理由 | 决策者 | 日期 | 复审触发条件`

示例（占位，按需新增，避免空泛）：

- *（尚未登记）*

新增规则：
1. 任何进入本节的条目必须在 PR 里说明理由，并列出未来需要复审的触发条件（例如"上游发布 v2 协议后复审"）。
2. 不允许仅以"lite 版本不做"作为裁剪理由——那是阶段语境，不再成立。
3. 每季度至少复审一次本节，过期未复审的条目自动回落到 §2 / §5 的 TODO 队列。
