# cc-rust 最终发布计划

> 更新日期: 2026-05-08
> 范围: `F:\AIclassmanager\cc\rust`
> 阶段: 全量构建 / Full Build 发布收口

本文是最终发布前的总控计划。它不替代现有状态文档，而是把仍未实现、实现不完美、需要裁剪决策和需要验证的事项按发布顺序收束起来。

现有事实入口仍是：

- [WORK_STATUS.md](WORK_STATUS.md): 当前完成度基线。
- [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md): 未完备项、runtime caveats、intentional crop。
- [KNOWN_ISSUES.md](KNOWN_ISSUES.md): 当前开放问题和代码审查发现。
- [TECH_DEBT.md](TECH_DEBT.md): 代码层技术债。
- [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md): Rust TUI 对标矩阵。

## 1. 发布定义

最终发布不是“没有任何未来增强”，而是达到以下状态：

1. 支持面内的功能与上游 TypeScript / Bun 版本行为对齐，或者已经在 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) 登记为 intentional crop。
2. 高危开放问题关闭，不再存在会导致静默降级、权限绕过、上下文错误估算、模型路由错误或持久化损坏的已知缺陷。
3. 发布门禁命令在干净工作区通过：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo build --release`。
4. provider、daemon、TUI、headless IPC、MCP、session/export、permissions 等关键路径都有可重复的目标测试或 e2e 证据。
5. 文档不再用历史 `rust-lite` 口径描述当前边界；所有“暂不做”都有复审条件。

## 2. 发布阶段

| 阶段 | 目标 | 退出条件 |
| --- | --- | --- |
| R0: 范围冻结 | 固定最终发布支持面，逐项重评历史 Deferred。 | 所有未跟随上游的能力都进入 intentional crop，或进入后续阶段任务。 |
| R1: 阻塞缺陷清零 | 先修构建、测试、权限、安全、上下文和模型路由阻塞。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) 中高危项关闭或降级并有证据。 |
| R2: 核心能力补齐 | 收束 API、PlanMode、TaskTools、Team Memory、WebFetch、Daemon、Session Export、Computer Use。 | 每条核心链路有目标测试，真实 provider/daemon/browser 路径有 e2e 或明确裁剪。 |
| R3: 产品体验补齐 | 收束 Ratatui UI、Web UI、Browser MCP、插件/配置/设置面和命令面。 | 用户可见主路径无“占位可用但实际不可用”的状态。 |
| R4: 可观测与运维 | 补 traceable logging、audit/session export、daemon soak、发布包和路径隔离复核。 | 发布后问题能定位、导出、复现和回滚。 |
| R5: 发布候选 | 完整门禁、安装/升级/回滚验证、文档冻结。 | 生成 release candidate，发布说明列出已支持能力、裁剪项、已知残余风险。 |

## 3. 发布门禁

### G0 范围与裁剪门禁

- [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) 中的历史 Deferred 不允许继续停留在“以后再说”状态。
- 每个 deferred surface 必须进入三类之一：本次实现、下个版本延期且有复审条件、intentional crop。
- 已确认裁剪项必须包含理由、决策者、日期和复审触发条件。

### G1 构建与测试门禁

发布前必须通过：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

专项门禁至少覆盖：

```bash
cargo test -p claude-code-rs --test e2e_terminal
cargo test -p claude-code-rs --test e2e_browser_mcp
cargo test -p claude-code-rs daemon::routes
cargo test -p claude-code-rs daemon::sse
cargo test -p claude-code-rs commands::daemon_cmd
cargo test -p claude-code-rs tools::hooks
cargo test -p claude-code-rs engine::lifecycle
cargo test -p claude-code-rs query::loop_helpers
```

真实 provider e2e 需要凭据时，发布记录要写明运行环境、跳过原因或替代证据。

### G2 安全与权限门禁

- Auto mode、Plan mode、sandbox、allowed command/prompt、classifier redaction 不允许存在已知绕过。
- 所有“配置存在但损坏”的路径必须返回可见诊断，不能静默当作缺省配置。
- Critical hook、权限 hook、安全 hook 的失败策略必须符合 fail-closed / warn-only 分级。

### G3 状态与持久化门禁

- `~/.cc-rust/` 路径隔离必须覆盖 credentials、settings、skills、daemon、session、plugins 和 cache。
- session 保存/恢复、task store、team mailbox、daemon command/event、plugin metadata 不允许因解析失败静默覆盖。
- Session export / audit export 能导出可追踪、可验证的记录；不完整字段必须标注清楚。

### G4 UI 与运行时门禁

- Ratatui 主交互路径、headless IPC、Web UI、MCP 面板、权限对话框、配置面板必须能完成真实工作流。
- UI snapshot、message suite、关键 e2e 覆盖 shell output、file edit、permission、task、MCP、history/search、resize。
- Browser MCP 与 Computer Use 至少有一条真实或 fake-server e2e，证明图片/tool result 不丢失。

### G5 文档与发布门禁

- `docs/README.md`、[WORK_STATUS.md](WORK_STATUS.md)、[IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md)、[KNOWN_ISSUES.md](KNOWN_ISSUES.md) 与发布说明一致。
- 旧 `Lite` wording 只允许出现在历史说明、分支名说明或 archive。
- 发布说明必须列出：支持矩阵、intentional crop、迁移说明、验证命令、已知残余风险。

## 4. 未实现与不完美项总表

### P0: 发布阻塞

| 范围 | 当前问题 | 预期发布效果 | 证据入口 |
| --- | --- | --- | --- |
| e2e terminal | `tests/e2e_terminal` 仍有未纳入版本控制的 `phase6.rs` 引用。 | e2e terminal 测试可在干净 checkout 运行，不依赖本地遗留文件。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `TEST-001` |
| clippy gate | `cargo clippy -p claude-code-rs --all-targets -- -D warnings` 仍被跨模块 lint 阻塞。 | workspace clippy 作为发布硬门禁，不再需要豁免。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `CLIPPY-001` |
| Auto mode 安全 | `permissions.enableAutoMode=false` 仍可能被启动配置、Web、插件上下文绕过。 | 一个全局策略源决定 Auto mode，所有入口一致执行。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `SAFETY-001` |
| Plan allowed prompts | Auto -> Plan -> ExitPlanMode 后可能追加未剥离危险规则的 Bash allow。 | `allowedPrompts` 写入前经过同一危险规则分类与剥离逻辑。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `SAFETY-002` |
| Sandbox allowed commands | sandbox 不可用时仍可能预批准命令，前缀匹配允许 shell 链式命令搭车。 | sandbox unavailable 时 fail closed；命令匹配使用结构化解析或严格边界。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `SAFETY-003` |
| Secret redaction | JSON 字段形式的 password/apiKey/token 没有被 redaction regex 覆盖。 | classifier、日志、trace、prompt diagnostic 统一走 secret redaction。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `SAFETY-004` |
| Context compact | auto compact 阈值可能重复扣减本地释放 token；exact count 漏 system prompt/tools。 | 模型请求大小估算可信，compact 不会错误跳过或误判。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `CONTEXT-001` / `CONTEXT-002` |
| Model/provider mapping | Bedrock 模型 ID 和 legacy alias 文档/实现不一致。 | provider 模型路由准确；文档、配置校验、运行时错误口径一致。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `MODEL-001` / `MODEL-002` / `DOC-001` |
| Worktree safety | `WorktreeRemove` 路径边界未覆盖 symlink / Windows junction 逃逸。 | 删除/清理路径在 Windows 与 Unix 都经过 resolved-boundary 校验。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `WORKTREE-001` |
| Critical post hooks | critical post/failure hook 错误仍可能在生产工具执行后被丢弃。 | critical hook 失败影响当前 step/tool，optional hook 才 warn-only。 | [TECH_DEBT.md](TECH_DEBT.md) Remaining P1 follow-ups |

### P1: 核心能力必须补齐或裁剪

| 范围 | 当前状态 | 预期发布效果 | 证据入口 |
| --- | --- | --- | --- |
| API providers | Bedrock/Vertex/Azure 基线存在，真实 provider/e2e 与 Azure 能力矩阵未完全收束。 | 每个声明支持的 provider 有配置文档、能力矩阵、真实或受控 e2e；不支持项 fail early。 | [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md), [mvp-optimization-plans/MVP-001-api-providers-plan.md](mvp-optimization-plans/MVP-001-api-providers-plan.md) |
| Team Memory | Rust daemon spawn 参数接通，但同步、断线恢复、冲突处理 e2e 未闭环。 | 多客户端同步可恢复、冲突可诊断，Team Memory 旧 plan/spec 可归档。 | [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) |
| TaskTools remote/multi-type | 本地 task 基础完成，remote/multi-type poller/reconnect parity 未完。 | 远程/多类型后台任务可恢复、可轮询、可取消、可在 `/tasks` 统一查看。 | [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) |
| PlanMode auto-mode | 保守 classifier 与持久化已落地，full LLM classifier parity 和 `allowedPrompts` 分类未完。 | Plan 创建、恢复、审批、退出、规则写入都有统一策略和 e2e。 | [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md), [KNOWN_ISSUES.md](KNOWN_ISSUES.md) |
| WebFetch | redirect/MIME/proxy/credential 完成，JS 渲染待实现或裁剪。 | 支持 browser-grade 渲染，或明确声明只做 HTTP fetch 并写入 intentional crop。 | [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md) |
| Daemon ownership | HTTP/SSE 控制面完成主干，assistant worker 尚未完全拥有 `/api/submit` 执行所有权。 | submit/abort/permission/resize/history 由 worker/scheduler 模型拥有，HTTP route 不再承担兼容执行路径。 | [reference/DAEMON_OPERATIONS.md](reference/DAEMON_OPERATIONS.md), [plan/daemon-usability-plan.md](plan/daemon-usability-plan.md) |
| Session export | 已有 transcript/tool/compact 基础，缺 API request snapshot、context collapse、mode/tag/title、raw vs apiView。 | 导出包可审计、可回放、能说明模型实际看到的上下文和裁剪历史。 | [plan/session-export-implementation-guide.md](plan/session-export-implementation-guide.md) |
| Computer Use | 外置 MCP 和图片链路多数完成，session/storage/export 回归仍有缺口。 | screenshot 图片块在工具结果、下一轮模型请求、session 保存/恢复、导出中不丢失。 | [plan/computer-use-implementation-checklist.md](plan/computer-use-implementation-checklist.md) |
| Browser MCP | 配置/提示/渲染基础存在，真实第三方 server 截图、console、network 端到端未验证。 | fake-server 与真实 browser MCP server 都有可重复验证；权限文案清楚。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `UI-004` |
| IPC subsystem | handler/types/events 仍有跨 LSP/MCP/Plugin/IDE/Skill/AgentSettings 聚合，协议缺版本策略。 | 每个 subsystem 独立 handler/type/event，JSON roundtrip contract tests 覆盖兼容策略。 | [TECH_DEBT.md](TECH_DEBT.md), [plan/ipc-refactor-plan.md](plan/ipc-refactor-plan.md) |
| Plugin diagnostics | 全局 plugin diagnostics 仍可能没有进入 `/reload_plugins` 输出。 | plugin metadata/cache/manifest 错误对用户可见，不再表现为“没有插件”。 | [TECH_DEBT.md](TECH_DEBT.md) Remaining P1 follow-ups |
| Auth compatibility | legacy auth wrapper 仍可能记录后返回 unauthenticated。 | 外部调用迁移到 diagnostic API；兼容 wrapper 不吞掉关键凭据错误。 | [TECH_DEBT.md](TECH_DEBT.md) Remaining P1 follow-ups |

### P2: 产品体验与支持面

| 范围 | 当前状态 | 预期发布效果 | 证据入口 |
| --- | --- | --- | --- |
| Ratatui shell output | renderer 支持 expanded/detail，最新 shell 输出自动展开未接 runtime context。 | 长输出默认策略符合上游体验，用户能快速展开、折叠、查看细节。 | [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md), [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `UI-002` |
| Ratatui history | Ctrl+R 只覆盖当前 session。 | 跨会话 prompt history 有 reader/API，搜索结果带来源和时间。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `UI-003` |
| Ratatui settings | usage、output style、language、thinking toggle、invalid settings、managed settings、sandbox tabs、cost/token warnings 等未完整。 | `/config` 和设置面覆盖最终支持的配置域，错误配置有可见修复路径。 | [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md) §8 |
| Ratatui agents/teams | AgentTree、Coordinator status、Agent progress、Team member card/summary 等缺失。 | 团队与 agent 状态能在 UI 中解释当前执行、阻塞和完成状态。 | [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md) §5 |
| Ratatui dialogs | session preview/export、global search、quick open、log selector、context visualization、API key/OAuth、diagnostics 等仍缺。 | 发布支持面内的对话框可用；不支持项明确裁剪或移到后续版本。 | [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md) §17 |
| Web UI | `web/handlers.rs` 仍有注入 messages 并启动 SSE stream 的 TODO。 | Web UI 能承载真实会话状态、权限事件、tool result、compact boundary 和重放。 | `crates/claude-code-rs/src/web/handlers.rs` |
| Remote channels | `/channels` 和远程 channel 仍是阶段性 stub；schedule remote triggers 未实现。 | Telegram/Lark 等远程入口要么可连接/测试/显示状态，要么作为后续路线写入 crop/roadmap。 | [plan/remote-channel-phase1-telegram-lark-plan-2026-05-08.md](plan/remote-channel-phase1-telegram-lark-plan-2026-05-08.md) |
| Voice | audio/STT 后端当前是明确 unsupported-build stub。 | 若纳入发布支持面，需真实 audio/STT backend；否则 UI/CLI 明确显示不可用原因。 | `crates/claude-code-rs/src/voice/**` |
| Browser native host | `cc-browser` native host binary mode 仍是后续阶段。 | Chrome/native-host 路线若纳入发布，需真实 connect/reconnect/install 验证；否则保持外置 MCP 路线。 | `crates/cc-browser/src/session.rs`, `crates/cc-browser/src/setup.rs` |
| LSP transport | workspace/configuration request response handling 在当前 transport 未实现。 | LSP server 请求处理有响应策略，不再只记录未实现诊断。 | `crates/claude-code-rs/src/lsp_service/client.rs` |
| Branch command | `/branch` 尚未自动切换 engine session pointer。 | 新分支会话切换后，后续消息进入正确 session。 | `crates/claude-code-rs/src/commands/branch.rs` |
| Terminal setup | 部分 terminal setup env 输出仍是 diagnostic-only。 | 支持面内的 terminal setup 能真正修改/指导用户环境；不支持项明确说明。 | `crates/claude-code-rs/src/commands/terminal_setup.rs` |
| Documentation debt | 多处文档仍有历史 Lite wording、mojibake、旧路径和旧完成度。 | 顶层 docs 只反映 Full Build 当前事实；历史内容迁入 archive。 | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) `DOC-003`, [TECH_DEBT.md](TECH_DEBT.md) |

### P3: 发布后增强或已裁剪

| 范围 | 决策 | 发布处理 |
| --- | --- | --- |
| Agent Teams tmux/iTerm2 pane backend | 已登记 intentional crop。 | 发布说明列入裁剪项；复审触发条件沿用 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md)。 |
| Windows OS-level sandbox primitive | 已登记 intentional crop。 | 发布说明列入裁剪项；`/sandbox require` 保持 fail-closed 和 unavailable 诊断。 |
| LogoV2/装饰动画/纯 React 抽象 | Ratatui 不需要 1:1 复制。 | 保持为不适用或后续美化，不阻塞发布。 |
| Remote/service/analytics/MDM/internal Ant-only | 历史 deferred，需要 R0 重评。 | 不允许继续模糊 deferred；本次不做就写 crop 或下版本计划。 |

## 5. 推荐执行顺序

1. 修 P0 构建与安全项：`TEST-001`、`CLIPPY-001`、SAFETY、CONTEXT、MODEL、WORKTREE、critical hook。
2. 收 P1 核心链路：API provider e2e、PlanMode、TaskTools、Team Memory、Daemon ownership、Session Export、Computer Use、Browser MCP。
3. 收 P2 用户体验：Ratatui settings/search/history/shell、Web UI、remote channel 决策、voice/browser/LSP/branch/terminal setup。
4. 做全仓文档收口：迁移完成历史到 archive，清理 Lite/mojibake，补最终发布说明草稿。
5. 跑 release candidate 门禁：完整 cargo gate、真实 provider smoke、daemon soak、TUI snapshot、headless/Web/Brower MCP e2e。

## 6. 模块验收效果

| 模块 | 发布时应达到的效果 |
| --- | --- |
| CLI / QueryEngine | 主循环可恢复、可中断、可压缩、可执行工具；错误不会被误报为成功或缺省状态。 |
| Tools / permissions | 每个工具调用都有权限、hook、执行、结果、审计链路；高风险策略 fail closed。 |
| API providers | 支持矩阵可信，模型别名和 provider ID 一致，真实错误有 actionable diagnostic。 |
| Session / compact / export | 用户能导出原始 transcript、API view、compact/microcompact 记录和 request snapshot。 |
| Daemon | 后台 supervisor/worker/scheduler 有清晰 ownership；HTTP/SSE 只是控制面，不隐藏执行失败。 |
| MCP / Browser / Computer Use | 外部 server 可发现、可授权、可调用；图片和结构化结果能进入模型上下文与保存层。 |
| Ratatui / Web UI | 主路径可完成真实工作，缺失功能不会以空 UI 或占位结果伪装成可用。 |
| Docs / release notes | 用户知道能用什么、不能用什么、为什么不能用、如何验证和回滚。 |

## 7. 发布说明草稿要求

最终 release note 至少包含：

- 支持平台和已验证环境。
- provider 支持矩阵和需要的环境变量/凭据。
- 命令、工具、MCP、TUI、daemon、Web UI 支持矩阵。
- intentional crop 列表和复审触发条件。
- 从历史 `rust-lite` 配置迁移到 Full Build 的注意事项。
- 发布门禁命令及通过摘要。
- 已知残余风险，只允许列中低风险或明确被产品接受的风险。

## 8. 文档维护规则

1. 本文只保留发布总控视图，不展开长实现设计。
2. 具体实现计划继续放在 `docs/plan/`；完成后迁移到 `docs/archive/`。
3. 新增开放问题先写 [KNOWN_ISSUES.md](KNOWN_ISSUES.md)，再在本文按发布优先级引用。
4. 新增功能缺口先写 [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md)，除非它只是纯代码质量问题。
5. 完成一个条目时，同步更新本文、源状态文档和 archive 记录，避免活跃文档保留已完成历史。

## 9. Remote-Control Gateway Release Gate (2026-05-08)

Remote-control gateway can be considered release-gate complete only when the following evidence is attached to the release record:

| Gate | Required evidence |
| --- | --- |
| API contract | `docs/reference/REMOTE_CONTROL_GATEWAY.md` lists every `/remote-control/v1/**` endpoint, request shape, response shape, auth requirement, and stable error code. |
| Local UX | `/remote status`, `/remote adapters`, `/remote runs`, `/remote show`, `/remote events`, `/remote stop`, `/remote doctor`, the command palette metadata, `RemoteSurface`, and status-widget remote indicator are documented. |
| Security | Bad daemon token, bad remote token, bad origin, payload too large, HMAC failure, duplicate idempotency, busy, queue full, and stale response cases have stable diagnostics and tests. |
| Recovery | Daemon startup recovery, queued run visibility, recoverable active runs, pending approval/user retention, and stale session-lock recovery are documented and tested. |
| Adapter scope | Telegram and Lark are documented as connect/health/status/test-message only; full inbound conversational control is explicitly follow-up work. |
| Non-goals | Public hosted gateway, multi-tenant SaaS, full WebSocket parity, remote desktop control, and real mid-turn `steer` are not release claims. |

Remote-control-specific verification commands:

```powershell
cargo test -p gateway
cargo test -p claude-code-rs remote_cmd ui::components::command_palette ui::components::command_surface ui::components::status_widget
cargo test -p claude-code-rs daemon::protocol daemon::routes daemon::sse daemon::supervisor
cargo check -p claude-code-rs --message-format short
```

The Session 16 docs gate verified documentation consistency only. It did not rerun the full code gate; the implementation report records that as a remaining release risk until Session 17/final verification is run.
