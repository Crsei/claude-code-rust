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
| API providers | 部分完成 (单独立项) | Bedrock / Vertex 已有 provider adapter 与基础测试；Bedrock 仍缺原生 AWS EventStream，Vertex 仍缺 direct service-account JWT exchange。补齐或裁剪决策见 `architecture/mvp-optimization-plans/MVP-001-api-providers-plan.md` |
| Team Memory 客户端同步 | 未实现 | 服务端代理已落地 (`src/daemon/team_memory_proxy.rs` + `ui/team-memory-server/`)；前端尚未调用，计划见 `superpowers/plans/2026-04-11-team-memory-sync.md` |

> 以下项在历史文档中曾标注为 stub，经代码核对已在 `rust-lite` 分支中收口，保留在本节做历史追踪：
>
> - **IPC `clear_messages`** — 已由 `QueryEngine::clear_messages()` (`crates/claude-code-rs/src/engine/lifecycle/mod.rs:245`) 实现，`/clear` 路径在 `crates/claude-code-rs/src/ipc/ingress.rs:332-339` 调用 engine 清空并回传 `conversation_replaced`。
> - **权限 Phase 2 Hook 拦截** — `crates/claude-code-rs/src/tools/execution/pipeline.rs:124-211` 先跑 `run_pre_tool_hooks`，再把结果折进 `has_permissions_to_use_tool_with_hook` (`crates/claude-code-rs/src/permissions/decision.rs:259-362`)，hook 的 deny/ask/allow 会按规范顺序生效。
> - **Vim 状态机** — `ui/src/vim/state-machine.ts` 已覆盖 normal/insert/visual 三模式、导航 (h/l/0/$/^/w/b/e)、operator (d/y/c)、单键 (x/X/p/u/D/C) 与 visual 选区操作；KNOWN_ISSUES 中目前无相关 open 项。
> - **Agent Teams 用户面** — `/team` 斜杠命令 + `TeamSpawn` 工具 + Team Dashboard 已落地，详见 §1.1。

### 1.1 Agent Teams 收口状态

rust-lite 对 Agent Teams 的最终收口是"**in-process 闭环 + 用户面全量**"（2026-05-05 核验通过）：

- **闭环核心** — `crates/claude-code-rs/src/teams/` 的 10 个子模块 (types/protocol/mailbox/context/identity/in_process/helpers/constants/runner/backend) 驱动同进程多代理 mailbox，teammate 作为 tokio 任务在 `task_local!` 身份隔离下运行。`runner.rs` 顶部仍有 `#![allow(unused)]`，存在未收束的死代码/符号（见 TECH_DEBT）。
- **工具层** — `SendMessage` 工具 (`crates/claude-code-rs/src/tools/send_message.rs:67`) `is_enabled()` 总返回 `true`，call 时检查 team_context 做优雅拒绝；`TeamSpawn` 工具 (`crates/claude-code-rs/src/tools/team_spawn.rs:150`) 让模型从对话里直接拉起新 teammate，必要时自动创建 session 绑定的团队。
- **REPL 层** — `/team` 斜杠命令家族 (`crates/claude-code-rs/src/commands/team_cmd.rs`) 覆盖 `create / list / status / spawn / send / kill / leave / delete` 8 个子命令。
- **UI 层** — `ui/src/components/TeamPanel.tsx` 订阅 `BackendMessage::TeamEvent`（通过 `protocol.ts:674` 的 `team_event` 类型），展示活跃 team、成员在线状态、未读计数、最近消息。
- **IPC 层** — `crates/claude-code-rs/src/ipc/agent_handlers.rs:132` 的 `build_team_status_events()` 读盘后发出 `TeamEvent::StatusSnapshot`；`crates/claude-code-rs/src/ipc/ingress.rs:420-431` 在 `/team` 命令执行后同步 `team_context` 并推送状态快照。
- **启用条件** — `is_agent_teams_active(app_state)` (`crates/claude-code-rs/src/teams/mod.rs:87`) 同时接受 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` env var 与 `AppState::team_context` 存在两种启用方式，后者让 `/team create` 或 `TeamSpawn` 调用在会话内就能解锁 team 功能。

**MVP-005 后端策略 (2026-04-28)**：tmux / iTerm2 终端 pane 后端正式登记为 §7 Intentional 裁剪。cc-rust 只支持 in-process backend；`backend::PaneBackend` trait 作为上游对齐审查边界保留，但 `backend::SUPPORTED_BACKENDS` (`crates/claude-code-rs/src/teams/backend.rs:23`) 只包含 `InProcess`，所有 runtime spawn 路径都通过 `InProcessBackend` 执行。

## 2. 全量构建待补齐 TODO（原「已完成但仍为缩减实现」）

> **状态反转**：下表条目**不再**被视为"已接受的 Lite 缩减"。它们是全量构建阶段需要按上游对齐的 TODO。触及以下模块时，默认按上游完整行为补齐，而不是"保持现状"。详细的原版代码路径/行数对照见 [`archive/COMPLETED_SIMPLIFIED.md`](archive/COMPLETED_SIMPLIFIED.md)。

### 2.1 已补齐或基本补齐（2026-05-05 代码核对）

| 模块 | 当前结论 | 证据 / 备注 |
|------|----------|-------------|
| FileWriteTool | 已补齐 | `crates/claude-code-rs/src/tools/fs/safe_write.rs` 已覆盖临时文件 + rename、恢复备份、大小限制、权限保持、二进制拒绝；`crates/claude-code-rs/src/tools/fs/file_write.rs` 返回 safe_write 诊断 |
| FileReadTool | 已补齐 | `crates/claude-code-rs/src/tools/fs/file_read.rs` 已覆盖 symlink canonicalize/metadata、UTF-8/UTF-16/BOM 检测、UTF-8 lossy fallback、大文件默认分页与 `next_offset` |
| SkillTool | 核心已补齐 | `crates/cc-skills/src/lib.rs` / `loader.rs` 已覆盖依赖解析、版本冲突、兼容版本、hot reload、frontmatter 诊断；若后续需要上游 MCP skill builder，可按插件/脚手架能力单独立项 |
| LSP | 已补齐 | `crates/claude-code-rs/src/lsp_service/client.rs` 已实现 `didChange` ranged updates 与 `publishDiagnostics` 被动接收；`crates/claude-code-rs/src/tools/lsp.rs` / `crates/claude-code-rs/src/lsp_service/mod.rs` 已提供 completion 与 diagnostics snapshot |
| BashTool heredoc 校验 | 已补齐子项 | `crates/cc-utils/src/bash.rs` 的 `validate_heredocs()` 已覆盖未闭合 delimiter、quoted delimiter、`<<-`、同一命令行多个 heredoc、quoted text / arithmetic shift 规避；`BashTool::validate_input()` 执行前拒绝畸形 heredoc |
| BashTool Git 操作跟踪 | 已补齐子项 | `crates/cc-utils/src/git_operation_tracking.rs` 已对齐上游 shell-agnostic 检测，覆盖 commit/amend/cherry-pick、push branch、merge/rebase、`gh pr`、`glab mr create` 与 curl PR endpoint；`BashTool` / `PowerShellTool` 成功结果会附带 `git_operations` 元数据 |
| BashTool 进程树/取消语义 | 已补齐子项 | `crates/claude-code-rs/src/tools/exec/process_control.rs` 为 Bash/PowerShell 统一配置 Unix process group / Windows `taskkill /T /F`，超时和 abort signal 会终止进程树并返回 `termination` 元数据；PowerShell 已从 `cmd.output()` 改为显式 spawn 以复用同一终止语义 |
| Bash/PowerShell 危险命令拒绝列表 | 已补齐子项 | `crates/cc-permissions/src/dangerous.rs` 已补齐上游 destructive warning 覆盖面：`git push --force-with-lease`、`git clean` dry-run 例外、`git stash drop/clear`、SQL drop/truncate、PowerShell `Remove-Item`/`Clear-Content`/磁盘与系统 cmdlet；`PowerShellTool` 通过执行安全门调用 PowerShell 专用检测 |
| PowerShell security validator 高风险规则 | 已补齐子项 | `crates/cc-permissions/src/dangerous.rs` 已覆盖上游 `powershellSecurity.ts` 第一批高风险拦截：`Invoke-Expression`/`iex`、嵌套 `powershell`/`pwsh`、download cradle、`Add-Type`、COM object、`Start-Process` 提权或再拉 PowerShell、WMI/CIM 进程创建；执行安全门测试覆盖 PowerShell 拦截路径 |
| PowerShell security validator 高风险规则（二） | 已补齐子项 | `crates/cc-permissions/src/dangerous.rs` 继续覆盖 standalone download utilities（`Start-BitsTransfer`/`certutil -urlcache`/`bitsadmin /transfer`）、script file execution、`ForEach-Object -MemberName`、`Invoke-Item`、scheduled task persistence、env scope mutation、module/script loading、alias/variable runtime-state mutation 与 PowerShell alternative parameter prefixes |
| PowerShell security validator 目标语法规则 | 已补齐子项 | `crates/cc-permissions/src/dangerous.rs` 以执行前硬拦形式覆盖动态调用 `Invoke-Expression`、危险 cmdlet script block、`ForEach-Object` script block、stop-parsing `--%` 与明显危险的 .NET static method 调用（Process/Assembly/Marshal/WebClient）；完整 AST parser parity 仍单列在 §2.2 |
| PowerShell security validator AST 启发式规则 | 已补齐子项 | `crates/cc-permissions/src/dangerous.rs` 增加 quote-aware 扫描与 CLM allowlist，覆盖一般 dynamic command name（`& $cmd` / `& (...)`）、dot-sourced dynamic command、`$()` subexpression、expandable string、splatting、member/static member invocation 与非 CLM allowlist type literal；原生 AST parser fidelity 仍单列在 §2.2 |
| Bash/PowerShell sandbox 文件系统 preflight | 已补齐子项 | `crates/cc-sandbox/src/runner.rs` 的 `preflight_shell_command()` 已接入显式写目标检查，覆盖 shell redirection、常见 Bash 写命令与 PowerShell 写 cmdlet，并按 read-only/workspace/allowWrite/denyWrite 返回 sandbox policy error |
| Bash/PowerShell sandbox fail-closed 用户面 | 已补齐子项 | `crates/cc-sandbox/src/runner.rs` 已在 OS primitive 不可用且 `sandbox.failIfUnavailable=true` 时硬失败；`/sandbox require` / `/sandbox optional` 现在可在会话内切换 fail-closed 与 best-effort fallback，并在 `/sandbox` 状态中暴露当前边界 |
| FileEditTool 读后冲突检测 | 已补齐子项 | `Read` 完整文本读取会写入共享 `FileStateCache`；`Edit` 校验和写入前按文件内容 hash 拒绝未读文件或读后被外部修改的文件，并在成功编辑后刷新缓存，避免覆盖用户/格式化器改动 |
| FileEditTool 文件锁/readonly 写前检查 | 已补齐子项 | `Edit` 在 validate/call 阶段尝试以读写句柄打开目标文件，提前拒绝 readonly、PermissionDenied、WouldBlock 与 Windows sharing violation（5/32/33）等锁定或不可写状态，避免等到覆盖写入时才失败 |
| FileEditTool 编辑历史备份 | 已补齐子项 | `Edit` 写入改走 `safe_write_text()`，每次覆盖前创建恢复备份并在 tool result / FileChanged hook payload 暴露 `edit_history.backup_path`，同时保留 atomic replace 与权限保持诊断 |
| FileEditTool 自动缩进修正 | 已补齐子项 | `Edit` 在 `old_string` 精确匹配失败时会查找唯一的“去除 leading whitespace 后等价”代码块，并把 `new_string` 的 leading whitespace 映射到文件中的实际缩进；歧义匹配保持拒绝 |
| FileEditTool live transcript 接线 | 已补齐子项 | `Edit` 成功结果把 concise model content 与 UI-only `display_preview` 分离；`SdkUserReplay` / headless IPC / Rust TUI 保留 `tool_use_result`，并用 `file_edit_tool_updated_message` 在 prompt/transcript 中渲染结构化 diff 预览 |
| AgentTool 工具白名单与去重 | 已补齐子项 | `crates/claude-code-rs/src/engine/agent/mod.rs` 创建 child `QueryEngineConfig` 前会按 `subagent_type` 解析内置/用户/项目 agent 定义，应用 `tools` allow-list、`disallowedTools` deny-list、`Bash(...)` 等规则规格的基础工具名解析，并按工具名去重；Explore/Plan/code-reviewer 等只读内置 agent 不再继承全量工具 |
| AgentTool 团队上下文继承 | 已补齐子项 | `crates/cc-engine/src/types/config.rs` 的 `AgentContext` 携带父会话 `team_context`，`QueryEngine::new()` 初始化子 agent AppState 时恢复该上下文，`build_child_config()` 从父 `ToolUseContext` 注入当前团队；子 agent 中的 `SendMessage` 不再因默认 AppState 丢失团队上下文 |
| AgentTool 多 agent 调度入口 | 已补齐子项 | `Agent` schema 对齐上游 `name` / `team_name` / `mode` 参数；提供 `name` 时走现有 `TeamSpawn` in-process teammate 路径，继承显式或当前 team context，输出 `status: "teammate_spawned"` / `teammate_id` / `team_name`，并把 `mode: "plan"` 传递为 teammate plan-mode requirement；tmux/iTerm2 pane 后端仍按 §7 Intentional 裁剪 |
| TaskTools `TaskOutput` 阻塞/超时读取 | 已补齐子项 | `TaskOutput` schema 已补 `block` / `timeout`（0..600000ms，默认 30000ms）并返回上游兼容 `retrieval_status: success / timeout / not_ready` + nested `task`；保留旧 flat output 字段给既有调用方；等待循环会轮询 task store 并响应 abort signal |

### 2.2 仍需补齐的工具 parity

| 模块 | 待补齐的行为（参考上游） |
|------|----------|
| BashTool | PowerShell 原生 AST parser fidelity（`elementTypes` / `children` / `nameType` / parse error fail-closed / statement securityPatterns 等结构化语义）与 Windows Restricted Token / Job Object OS-level primitive；Stage 3c.2 执行前硬拦、heredoc、Git 操作跟踪、进程树终止、destructive denylist、高风险 security validator、AST 启发式安全检查、显式写目标 FS preflight 与 fail-closed 用户面子项已落地 |
| TaskTools | 远程/多类型后台任务 supervisor parity；磁盘持久化、基础依赖字段、输出保留、`TaskOutput` 阻塞/超时读取、后台 local-agent 取消和 `/tasks` 独立 UI 基础已完成 |
| PlanMode | auto-mode/classifier gate、团队审批流、计划持久化、实现关联跟踪 |
| WebFetch | JS 渲染、Cookie 管理、代理支持、重定向限制、Content-Type 智能处理 |

### 2.3 更新后的顺序执行计划（逐项领取）

每个条目完成时都按同一收口流程处理：读上游实现 → 改 Rust 端 → 补单元/e2e → `cargo fmt --all --check` + 对应构建 → 更新本文件与 archive → 单独提交。

1. **BashTool 剩余安全/沙箱复核**：对照 `src/tools/BashTool/**` 与 PowerShell validator，评估是否需要引入原生 PowerShell AST parser（或等价结构化 parser）来补齐 `elementTypes` / `children` / `nameType` / parse error fail-closed / statement securityPatterns 等非正则语义；同时评估 Windows Restricted Token / Job Object OS-level primitive 是否进入实现队列或移入 §7 Intentional 裁剪。
2. **TaskTools 后台任务 parity**：在现有持久化、取消、`TaskOutput` 阻塞/超时读取基础上补远程/多类型后台任务 supervisor parity，并验证 `/tasks` UI 与 task store 的状态一致性。
3. **PlanMode 执行闭环**：补 auto-mode/classifier gate、团队审批流、计划持久化与实现关联追踪；完成后用 plan 创建、恢复、审批、执行关联的 e2e 覆盖。
4. **WebFetch browser-grade 能力**：按 `architecture/mvp-optimization-plans/MVP-009-web-fetch-browser-grade-plan.md` 逐步补 JS 渲染、Cookie jar、代理、重定向限制与 Content-Type 智能处理。
5. **API providers 决策/实现**：按 `architecture/mvp-optimization-plans/MVP-001-api-providers-plan.md` 重评 Bedrock 原生 AWS EventStream 与 Vertex direct service-account JWT exchange；实现或写入 §7 Intentional 裁剪，不再停留在“部分完成”。
6. **Team Memory 客户端同步**：接通 `src/daemon/team_memory_proxy.rs` / `ui/team-memory-server/` 的前端调用路径，补同步、断线恢复与冲突处理测试。
7. **UI caveats 收束**：修复 §3 的终端 resize 回流与窄终端欢迎页布局；完成后迁移到 archive 或 `KNOWN_ISSUES.md` closed 记录。
8. **活跃方案文档清理**：逐个复核 §4 文档，能落地的拆成实现任务，过期或已覆盖的归档，仍有效的保留 owner/下一步。
9. **历史 Deferred 重评**：按 §5 类别决定实现、延期或 §7 Intentional 裁剪；不得继续用 "lite 不做" 作为理由。
10. **最终全量复核**：跑覆盖相关工具面的单元/e2e 与 release build，确认本节没有残留 TODO，更新 `WORK_STATUS.md` / archive 后收尾。

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

> Background agent + worktree、权限回调、取消/退出清理已由 `crates/claude-code-rs/src/engine/agent/supervisor.rs` 收口；历史 caveat 不再作为 Open 项保留。如后续发现回归，再写入本节。

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

- Agent Teams tmux/iTerm2 pane backend：cc-rust 当前主运行环境包含 Windows，外部 pane 后端会引入 tmux/iTerm2/窗口管理器耦合、跨平台清理语义和额外交互面；MVP-005 决定不实现外部 pane backend，而是把 in-process backend 做成唯一受支持路径并补齐生命周期控制。`PaneBackend` trait 保留为未来上游 parity 审查边界。| Codex | 2026-04-28 | 用户明确需要可见终端 pane、上游 pane protocol 成为产品必需项，或 cc-rust roadmap 切换到 Unix terminal-pane 优先发布

新增规则：
1. 任何进入本节的条目必须在 PR 里说明理由，并列出未来需要复审的触发条件（例如"上游发布 v2 协议后复审"）。
2. 不允许仅以"lite 版本不做"作为裁剪理由——那是阶段语境，不再成立。
3. 每季度至少复审一次本节，过期未复审的条目自动回落到 §2 / §5 的 TODO 队列。
