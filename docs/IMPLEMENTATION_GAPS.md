# cc-rust 未完备项与全量构建 TODO

> 更新日期: 2026-05-07 | 当前阶段: 全量构建 / Full Build

本文只登记仍未补齐、仍需重评或明确 intentional crop 的内容。已确认实现或已关闭的历史记录已迁移到：

- [archive/COMPLETED_FULL.md](archive/COMPLETED_FULL.md)
- [archive/COMPLETED_SIMPLIFIED.md](archive/COMPLETED_SIMPLIFIED.md)
- [archive/completed-gap-closures-2026-05-07.md](archive/completed-gap-closures-2026-05-07.md)

开放问题与代码审查发现统一看 [KNOWN_ISSUES.md](KNOWN_ISSUES.md)。当前完成度基线看 [WORK_STATUS.md](WORK_STATUS.md)。

## 1. 当前仍未完成或仅部分完成

| 范围 | 当前状态 | 说明 |
| --- | --- | --- |
| API providers | 部分完成 | Bedrock 原生 AWS EventStream、Vertex direct service-account JWT exchange 已补；仍需收束 Azure 命名/能力矩阵与真实 provider/e2e 覆盖。 |
| Team Memory 客户端同步 | 代码已接通，待验证/文档收口 | `ui/team-memory-server/sync.ts` / `watcher.ts` 与 Rust daemon spawn 参数已接通；仍需同步、断线恢复、冲突处理 e2e。 |
| TaskTools remote/multi-type runtime | 基础完成，runtime parity 未完 | 持久化、依赖字段、输出保留、`TaskOutput` 阻塞/超时、task taxonomy、remote metadata、recoverable marker、restore timer reset、remote review timeout guard、local-agent 取消和 `/tasks` UI 基础已完成；仍需 remote/multi-type poller/reconnect runtime parity。 |
| PlanMode auto-mode parity | 基础完成，classifier parity 未完 | 保守 classifier、计划持久化、approval lifecycle、实现任务关联、团队审批 mailbox、plan file 写入白名单已落地；仍需 full auto-mode LLM classifier parity 和 `allowedPrompts` 语义分类收口。 |
| WebFetch browser-grade 能力 | 部分完成 | redirect budget / cross-host diagnostic、Content-Type 分发、环境代理/`NO_PROXY`、Cookie/credential 边界已完成；JS 渲染仍待实现或裁剪决策。 |
| Daemon supervisor/worker ownership | 阶段主干完成，完整 ownership 未完 | 当前 HTTP/SSE 控制面已读 supervisor/worker 状态并写入 command/event 协议；真实 submit/abort 执行 ownership 仍有兼容路径。 |

## 2. 活跃方案文档

以下文档仍是活跃入口，不应归档为“已完成”：

- [computer-use-implementation-checklist.md](computer-use-implementation-checklist.md): Computer Use 落地清单，仍是待实施能力。
- [session-export-implementation-guide.md](session-export-implementation-guide.md): Rust 侧仍缺完整导出基础设施。
- [ipc-refactor-plan.md](ipc-refactor-plan.md): IPC 结构重构计划尚未完全收束。
- [traceable-logging-plan.md](traceable-logging-plan.md): 可追溯日志体系仍是 Draft。
- [daemon-usability-plan.md](daemon-usability-plan.md): daemon 可用化主干已分阶段落地，但仍有 worker/route ownership 余量。
- [superpowers/plans/2026-04-11-team-memory-sync.md](superpowers/plans/2026-04-11-team-memory-sync.md): Team Memory 客户端同步仍需 e2e 与文档收口。
- [superpowers/specs/2026-04-11-team-memory-sync-design.md](superpowers/specs/2026-04-11-team-memory-sync-design.md): Team Memory 验证清单仍有效。
- [superpowers/plans/2026-04-12-tools-commands-test-coverage.md](superpowers/plans/2026-04-12-tools-commands-test-coverage.md): 测试覆盖补齐计划仍有效。
- [superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md](superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md): PTY 交互测试扩展计划仍有效。
- [superpowers/specs/2026-04-20-workspace-split-design.md](superpowers/specs/2026-04-20-workspace-split-design.md): workspace split 后续 phase 仍开放。

## 3. 已知设计限制与 runtime caveats

| 范围 | 状态 | 说明 |
| --- | --- | --- |
| UI resize 回流 | 部分收口 | Rust TUI 已有 width-aware virtual scroll 回归；TS/OpenTUI fullscreen/maximize 白行问题仍在 [KNOWN_ISSUES.md](KNOWN_ISSUES.md) 跟踪。 |
| Rust TUI shell output | 基础完成，自动策略未完 | renderer 已支持 expanded/collapsed/detail view；最新 shell output 自动展开仍待 runtime context 接线。 |
| Rust TUI Ctrl+R history | 基础完成，持久历史未完 | 当前只搜索本次 TUI session prompt；跨会话历史 reader/API 未接。 |
| Browser MCP real-server path | 未验证 | 配置/提示/渲染基础存在，但尚未对真实 browser MCP server 做截图/console/network 端到端验证。 |

## 4. 历史 Deferred 重评队列

历史 `rust-lite` deferred 不再自动等于“不实现”。触及时按以下类别处理：

- 远程控制与多端集成：`/remote-control`、`/desktop`、`/mobile`、`bridge/`、`remote/`。
- 服务端/传输扩展：`server/`、SSE/WebSocket/Worker transport、MCP server mode。
- 远程/运营能力：`Monitor`、`PushNotification`、`SubscribePR`、`Workflow`、遥测与 MDM 同步。
- Ant-only 命令与内部工具：`/agents-platform`、`/ant-trace`、`CtxInspect`、`OverflowTest`、`Tungsten` 等。

重评规则：

1. 默认按上游完整行为补齐。
2. 若决定延期，必须有明确理由与复审条件。
3. 若决定不跟随上游，必须登记到 §6 "Intentional 裁剪"。
4. 不允许仅以 "lite 版本不做" 作为理由。

## 5. 代码层技术债入口

`TECH_DEBT.md` 中仍然活跃的债务来源包括：

- `#![allow(unused)]` 清理未完成。
- `test_ctx()` 等测试样板未完全收束到共享 helper。
- 模型别名映射未完全合并到单一查找表。
- 工具输入解析风格不统一。
- IPC 协议缺显式版本策略。

## 6. Intentional 裁剪

本节登记全量构建阶段确认不跟随上游的裁剪项。格式：

`- <模块/功能>: <裁剪理由> | <决策者> | <日期> | <复审触发条件>`

- Agent Teams tmux/iTerm2 pane backend: cc-rust 当前主运行环境包含 Windows，外部 pane backend 会引入 tmux/iTerm2/窗口管理器耦合、跨平台清理语义和额外交互面；MVP-005 决定不实现外部 pane backend，而是把 in-process backend 做成唯一受支持路径并补齐生命周期控制。`PaneBackend` trait 保留为未来上游 parity 审查边界。 | Codex | 2026-04-28 | 用户明确需要可见终端 pane、上游 pane protocol 成为产品必需项，或 cc-rust roadmap 切换到 Unix terminal-pane 优先发布。
- BashTool Windows Restricted Token / Job Object OS-level primitive: 上游 `@anthropic-ai/sandbox-runtime` 当前只对 macOS、Linux 与 WSL2 暴露 sandbox 支持，PowerShell permission UI 明确没有 sandbox toggle；cc-rust 不自研 Windows token/job sandbox，保留 Rust-level FS/network preflight、`/sandbox require` fail-closed 与 unavailable 诊断。 | Codex | 2026-05-05 | 上游发布 Windows sandbox-runtime backend、PowerShell sandbox toggle 成为产品必需项，或安全策略要求 Windows OS-level enforcement。

新增规则：

1. 任何进入本节的条目必须在 PR 中说明理由，并列出未来复审触发条件。
2. 每季度至少复审一次本节。
3. 过期未复审的条目回落到 TODO 队列。
