# cc-rust 工作状态总览

> 更新日期: 2026-05-14 | 分支历史名: `rust-lite` | 当前阶段: 全量构建 / Full Build

本文件只保留当前阶段仍需要判断和执行的状态。已经确认实现、已关闭或只具历史价值的阶段记录统一看：

- [archive/COMPLETED_FULL.md](archive/COMPLETED_FULL.md)
- [archive/completed-gap-closures-2026-05-07.md](archive/completed-gap-closures-2026-05-07.md)
- [archive/issues/](archive/issues/)

缩减实现、未完备项和 intentional crop 统一看 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md)。开放问题与代码审查发现统一看 [KNOWN_ISSUES.md](KNOWN_ISSUES.md)。最终发布顺序、发布门禁和预期效果看 [FINAL_RELEASE_PLAN.md](FINAL_RELEASE_PLAN.md)。

## 当前结论

cc-rust 已不再按历史 "Lite" 边界维护。触及上游能力时，默认按 `F:\AIclassmanager\cc\src\**` 或 `F:\AIclassmanager\cc\claude-code-bun\**` 的完整行为对齐；确需保留裁剪时，必须写入 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) 的 "Intentional 裁剪"。

当前已确认完成并归档的主线包括：

- API 基线：Anthropic、OpenAI compatible、Google Gemini、Azure、Bedrock、Vertex 均有运行时支持；真实 provider/e2e 覆盖仍是后续质量门。
- 认证：API key、系统 Keychain、OAuth PKCE、token 持久化与刷新已落地。
- 工具基线：Bash、PowerShell、Read、Write、Edit、Grep、Glob、Agent、Skill、LSP、Tasks、Web、Brief、Sleep 等主路径已落地。
- Agent Teams：in-process backend、`/team`、`TeamSpawn`、`SendMessage`、Team Dashboard 已收口；tmux/iTerm2 pane backend 是 intentional crop。
- Extensibility：hooks、skills、custom-agent active runtime safety、MCP stdio/local SSE/remote SSE/Streamable HTTP/OAuth/reconnect/tool refresh 已按当前标准面闭环。
- Ratatui UI：P0/P1 基础面已完成；运行时 residual 见 [KNOWN_ISSUES.md](KNOWN_ISSUES.md)，未跟踪 parity 缺口见 [ratatui-ui-parity-untracked-gap-plan-2026-05-08.md](plan/ratatui-ui-parity-untracked-gap-plan-2026-05-08.md)。
- Runtime storage：`CC_RUST_HOME` / `~/.cc-rust/` 路径隔离已落地，旧计划归档。

## 活跃待办

| 范围 | 当前状态 | 下一步 |
| --- | --- | --- |
| API providers | 基线完成，质量门未完全收束 | 收束 Azure 命名/能力矩阵与真实 Bedrock/Vertex/Azure provider e2e 覆盖。 |
| Team Memory 客户端同步 | 代码路径已接通，验证与文档收口未完 | 补同步、断线恢复、冲突处理 e2e；通过后归档旧 Team Memory plan/spec。 |
| TaskTools | 多数基础已完成，remote/multi-type poller parity 仍开放 | 对齐远程/多类型后台任务 poller/reconnect runtime。 |
| PlanMode | 保守 classifier、持久化、审批和 plan file 白名单已完成 | 补 full auto-mode LLM classifier parity，并覆盖 plan 创建/恢复/审批/e2e。 |
| WebFetch | redirect/MIME/proxy/credential 边界已完成 | 补 browser-grade JS 渲染或明确裁剪。 |
| Daemon | Phase 1-7 主干记录已落地，仍有 worker/route ownership 余量 | 继续把真实 submit/abort 与 scheduler ownership 从兼容路径迁入 supervisor/worker 架构。 |
| Crate migration | Phase 0-12 implementation slices landed; build/test gates green; guard closeout not complete | 保持 [crate-migration-phase-plan-2026-05-14.md](plan/crate-migration-phase-plan-2026-05-14.md) 为活跃计划；Rust TUI 已回退为 root-owned，下一步清理剩余 root-style imports、allow attributes，并复审 Codex compatibility path hits。 |
| UI/runtime issues | P0/P1 基础完成，存在 residuals 和未跟踪 parity 缺口 | 运行时 residual 见 [KNOWN_ISSUES.md](KNOWN_ISSUES.md)；`⚠️ 部分` / `❌ 缺失` 的未跟踪功能按 [ratatui-ui-parity-untracked-gap-plan-2026-05-08.md](plan/ratatui-ui-parity-untracked-gap-plan-2026-05-08.md) 分阶段处理。 |
| 文档状态一致性 | 本轮已收敛顶层入口 | 后续每完成一个模块，都同步迁移完成记录到 archive，避免活跃 TODO 文档堆积完成历史。 |

## 活跃文档入口

- [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md): 未完备项、全量构建 TODO、intentional crop。
- [FINAL_RELEASE_PLAN.md](FINAL_RELEASE_PLAN.md): 最终发布顺序、门禁、未实现/不完美项和预期效果。
- [KNOWN_ISSUES.md](KNOWN_ISSUES.md): 当前开放问题、代码审查发现、文档状态问题。
- [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md), [CLI_REFERENCE.md](CLI_REFERENCE.md), [USAGE_GUIDE.md](USAGE_GUIDE.md): 用户命令与使用说明。
- [DAEMON_OPERATIONS.md](DAEMON_OPERATIONS.md), [daemon-usability-plan.md](daemon-usability-plan.md): daemon 当前操作面与后续计划。
- [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md), [ratatui-ui-parity-untracked-gap-plan-2026-05-08.md](plan/ratatui-ui-parity-untracked-gap-plan-2026-05-08.md): Rust TUI 对标与后续 UI parity。
- [STORAGE.md](STORAGE.md): cc-rust 路径隔离与数据目录规则。
- [traceable-logging-plan.md](traceable-logging-plan.md): 可追溯日志体系 draft。

## 历史 Deferred

历史 deferred 不再等于 "不做"。远程控制、多端集成、服务端扩展、遥测/MDM、Ant-only 命令和内部工具都需要在触及时重新评估：

- 要实现：补到对应 plan / implementation task。
- 要延期：保留在 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) TODO 区。
- 要裁剪：写入 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) "Intentional 裁剪"，说明理由、决策者、日期和复审触发条件。

## Ratatui UI parity OMX final verification (2026-05-10)

The ratatui UI parity OMX batch series has been closed out at documentation level. Available batch summaries/last messages through the final task were reviewed, intentional UI snapshot updates were accepted, and the feasible targeted UI verification set was run. The later crate-migration verification pass on 2026-05-14 made the default workspace test gate green.
