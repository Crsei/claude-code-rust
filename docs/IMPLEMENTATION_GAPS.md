# cc-rust Lite 未完备项与注意事项汇总

> 本文把 `docs/` 中分散的“注意点 / 缩减实现 / 设计限制 / 未完备项”集中到一个入口。
> 当前完成度基线看 [`WORK_STATUS.md`](WORK_STATUS.md)，用户可感知问题看 [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md)，历史已完成方案与变更记录已归档到 [`archive/`](archive/)。
>
> 若要看与 `claude-code-bun` 的对标差异、`run in chrome` 判断、Web UI 评估和 REPL 结构规划，见 [`claude-code-bun-gap-plan.md`](claude-code-bun-gap-plan.md)。

## 1. 当前仍未完成或仅部分完成

| 范围 | 当前状态 | 说明 |
|------|----------|------|
| API providers | 未完成 (单独立项) | Bedrock、Vertex 仍未实现；由独立 issue 追踪 |
| Agent Teams | Feature-gated (实验性) | 见 §1.1 — 默认不上主路径，lite 不提供 Dashboard / tmux / iTerm2 后端 |
| Team Memory 客户端同步 | 未实现 | 服务端代理已落地 (`src/daemon/team_memory_proxy.rs` + `ui/team-memory-server/`)；前端尚未调用，计划见 `superpowers/plans/2026-04-11-team-memory-sync.md` |

> 以下三项在历史文档中曾标注为 stub，经代码核对已在 `rust-lite` 分支中收口，保留在本节做历史追踪：
>
> - **IPC `clear_messages`** — 已由 `QueryEngine::clear_messages()` (`src/engine/lifecycle/mod.rs:245`) 实现，`/clear` 路径在 `src/ipc/ingress.rs:332-339` 调用 engine 清空并回传 `conversation_replaced`。
> - **权限 Phase 2 Hook 拦截** — `src/tools/execution/pipeline.rs:124-211` 先跑 `run_pre_tool_hooks`，再把结果折进 `has_permissions_to_use_tool_with_hook` (`src/permissions/decision.rs:259-362`)，hook 的 deny/ask/allow 会按规范顺序生效。
> - **Vim 状态机** — `ui/src/vim/state-machine.ts` 已覆盖 normal/insert/visual 三模式、导航 (h/l/0/$/^/w/b/e)、operator (d/y/c)、单键 (x/X/p/u/D/C) 与 visual 选区操作；KNOWN_ISSUES 中目前无相关 open 项。

### 1.1 Agent Teams 收口政策

`src/teams/` 存在 10 个子模块 (types/protocol/mailbox/context/identity/in_process/helpers/constants/runner/backend) + `SendMessageTool`。**rust-lite 中的明确收口方针**：

- **保留**：in-process backend + mailbox + 协议 + runner + `SendMessage` 工具。这些构成"同进程多代理 mailbox"的最小闭环。
- **不实现**：tmux / iTerm2 终端后端 (`backend.rs` 中的 `PaneBackend` trait 仅作类型保留)、Team Dashboard、`/team` 类斜杠命令。需要终端分屏协作的用户请用完整版 claude-code。
- **默认不可见**：整个模块通过 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` env var 门控。`SendMessageTool::is_enabled()` 直接查询 `is_agent_teams_enabled()`，未启用时不会进入 `base_tools()` 的过滤结果 (`src/tools/registry.rs:68-74`)，主路径不会暴露任何 team 工具。

模块顶部 doc 已补充此收口声明，避免"文档承认未完成但代码继续裸露主路径"的悬置状态。

## 2. 已完成但仍为缩减实现

这些模块已经可用，但与完整版本相比仍有明确裁剪，详细背景来自 [`archive/COMPLETED_SIMPLIFIED.md`](archive/COMPLETED_SIMPLIFIED.md)。

| 模块 | 主要缺口 |
|------|----------|
| BashTool | 无 PowerShell 分支、无 sandbox、无进程组管理、无危险命令拒绝列表 |
| FileEditTool | 无 diff 预览、冲突检测、文件锁检查、编辑历史、自动缩进修正 |
| FileWriteTool | 无临时文件后 rename 的安全写入、无备份/恢复、无大小限制、无权限保持 |
| FileReadTool | 无符号链接解析、无大文件智能分页、无编码检测 |
| SkillTool | 无依赖解析、热重载、版本管理、复杂 frontmatter 校验 |
| TaskTools | 仅内存存储；无持久化、依赖图、后台管理、超时控制、独立 UI |
| ToolSearchTool | 无 TF-IDF 排名、全文索引、deferred schema 加载 |
| PlanMode | 无 auto-mode/classifier gate、审批流、计划持久化、实现关联跟踪 |
| LSP | 无 `didChange` 增量同步、`publishDiagnostics` 被动反馈、补全建议、插件侧配置整合 |
| WebFetch | 无 JS 渲染、Cookie 管理、代理支持、重定向限制、Content-Type 智能处理 |

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

## 5. 明确不在 rust-lite 范围内

以下能力在当前 lite 路线中明确延期或不实现：

- 远程控制与多端集成：`/remote-control`、`/desktop`、`/mobile`、`bridge/`、`remote/`
- 服务端/传输扩展：`server/`、SSE/WebSocket/Worker transport、MCP server mode
- 远程/运营能力：`Monitor`、`PushNotification`、`SubscribePR`、`Workflow`、遥测与 MDM 同步
- Ant-only 命令与内部工具：`/agents-platform`、`/ant-trace`、`CtxInspect`、`OverflowTest`、`Tungsten` 等

完整列表以 [`WORK_STATUS.md`](WORK_STATUS.md) 的 Deferred 段落为准。

## 6. 代码层技术债入口

`TECH_DEBT.md` 里的这些问题仍然是活跃债务来源：

- `#![allow(unused)]` 清理仍未做完
- `test_ctx()` 等测试样板仍未完全收束到共享 helper
- 模型别名映射仍未完全合并到单一查找表
- 工具输入解析风格仍不统一
- IPC 协议仍缺显式版本策略

如果只想看“现在不能指望它已经完善”的地方，优先读本文；如果要进入修复实施，再去看对应的源文档。
