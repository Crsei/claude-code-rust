# cc-rust Lite 未完备项与注意事项汇总

> 本文把 `docs/` 中分散的“注意点 / 缩减实现 / 设计限制 / 未完备项”集中到一个入口。
> 当前完成度基线看 [`WORK_STATUS.md`](WORK_STATUS.md)，用户可感知问题看 [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md)，历史已完成方案与变更记录已归档到 [`archive/`](archive/)。
>
> 若要看与 `claude-code-bun` 的对标差异、`run in chrome` 判断、Web UI 评估和 REPL 结构规划，见 [`claude-code-bun-gap-plan.md`](claude-code-bun-gap-plan.md)。

## 1. 当前仍未完成或仅部分完成

| 范围 | 当前状态 | 说明 |
|------|----------|------|
| API providers | 未完成 | Bedrock、Vertex 仍是 `unimplemented!()` 存根 |
| Agent Teams | 部分实现 | feature-gated；Runner 仅 log，Tmux/iTerm2 后端未实现，无 Dashboard / `/team` 命令 |
| 权限系统 | 部分实现 | Phase 2 hook 拦截仍是 stub，当前直接 fall through |
| IPC | 部分实现 | `clear_messages` 仅通知前端，engine 端无对应清空逻辑 |
| 前端交互 | 部分实现 | Vim 状态机只有模式切换完整，按键处理仍不完整 |
| Team Memory | 部分实现 | 服务端最小代理已落地，客户端同步仍待实现 |

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
