# Bun Docs Implementation Map

本文档是 `docs/bun-docs-documentation-plan.md` 的执行入口，用于汇总
`F:\AIclassmanager\cc\claude-code-bun\docs` 中 `agent`、`context`、
`extensibility`、`safety`、`tools` 五类文档与 cc-rust 当前实现之间的映射。

本轮输出采用“主索引 + 分章节文档”的结构。具体实现证据、文件路径和差异
说明放在各章节文档中；本文件只保留总览、状态规则和跨章节汇总。

## 输入范围

| 分类 | Bun 文档目录 | 输出文档 |
| --- | --- | --- |
| Agent | `F:\AIclassmanager\cc\claude-code-bun\docs\agent` | [`agent-implementation-map.md`](agent-implementation-map.md) |
| Context | `F:\AIclassmanager\cc\claude-code-bun\docs\context` | [`context-implementation-map.md`](context-implementation-map.md) |
| Extensibility | `F:\AIclassmanager\cc\claude-code-bun\docs\extensibility` | [`extensibility-implementation-map.md`](extensibility-implementation-map.md) |
| Safety | `F:\AIclassmanager\cc\claude-code-bun\docs\safety` | [`safety-implementation-map.md`](safety-implementation-map.md) |
| Tools | `F:\AIclassmanager\cc\claude-code-bun\docs\tools` | [`tools-implementation-map.md`](tools-implementation-map.md) |

## 状态规则

| 状态 | 含义 |
| --- | --- |
| 已实现 | cc-rust 已有稳定实现，并有明确实现入口 |
| 部分实现 | 核心路径存在，但缺少 Bun 版某些行为 |
| 未实现 | 没有对应能力，或只有占位 |
| 待确认 | 尚未形成足够证据，不能稳定判断 |
| 故意裁剪 | 项目明确决定不完全对齐 Bun 版 |

所有“已实现”“部分实现”“未实现”“故意裁剪”结论都应在分章节文档中保留
文件路径依据。

## 总览

| 分类 | 上游文档数 | 章节文档 | 汇总状态 |
| --- | ---: | --- | --- |
| Agent | 3 | [`agent-implementation-map.md`](agent-implementation-map.md) | 1 已实现，2 部分实现；存在故意裁剪 |
| Context | 4 | [`context-implementation-map.md`](context-implementation-map.md) | 1 已实现，3 部分实现 |
| Extensibility | 5 | [`extensibility-implementation-map.md`](extensibility-implementation-map.md) | 2 已实现，3 部分实现；本地 loopback SSE 与 manager reconnect API 已接入，远程认证仍未完整 |
| Safety | 5 | [`safety-implementation-map.md`](safety-implementation-map.md) | 2 已实现，3 部分实现；存在明确未实现项 |
| Tools | 5 | [`tools-implementation-map.md`](tools-implementation-map.md) | 4 已实现，1 部分实现 |

## 分文档状态

| 分类 | Bun 文档 | 状态 | 关键结论 |
| --- | --- | --- | --- |
| Agent | `coordinator-and-swarm.mdx` | 部分实现 | Rust 侧有 Agent Teams、mailbox、TaskList / TaskStop、TeamSpawn / SendMessage；独立 coordinator 模式、PR 订阅与 tmux / 多终端 swarm 没有同构实现。 |
| Agent | `sub-agents.mdx` | 已实现 | `AgentTool`、内置 agent、同步 / 后台生命周期、hooks、AgentTree、fork 与 worktree 侧路均有实现入口。 |
| Agent | `worktree-isolation.mdx` | 部分实现 | 子 Agent worktree 隔离和清理已存在，但路径布局、hook 入口和恢复流程与 Bun 上游不同。 |
| Context | `compaction.mdx` | 部分实现 | 本地压缩、Microcompact Boundary、Session Memory Compact、boundary、手动 `/compact` / Session Memory Compact / 自动模型摘要 / 内部 snip/context-collapse preservedSegment 元数据、PTL 恢复与 hook 路径存在；feature gate、Partial Compact 与完整恢复语义仍未完全同构。 |
| Context | `project-memory.mdx` | 部分实现 | memory CRUD、`MEMORY.md` 入口索引、`CLAUDE.md` 注入、Project / Global / Team memory 主提示词注入存在；Auto memory 已由 `auto_memory_enabled` 门控注入，最近 session-insights 会按 workspace、时间窗口、tag 和当前 session 排除规则回注；生命周期抽取已改用确定性 insight helper。 |
| Context | `system-prompt.mdx` | 已实现 | 静态段、动态段、缓存边界、`CLAUDE.md` 注入、append / override 顺序均已落地。 |
| Context | `token-budget.mdx` | 部分实现 | 预算判断、续跑逻辑、环境变量覆盖和 `[1m]` 窗口解析存在；`TokenUsageReport` 已暴露启发式预算诊断，但仍不是 provider 级精确 token 统计。 |
| Extensibility | `custom-agents.mdx` | 部分实现 | 定义、编辑、运行链路已通；安全边界主要依赖通用工具过滤与隔离。 |
| Extensibility | `hooks.mdx` | 已实现 | hooks 配置、执行和权限联动已形成闭环。 |
| Extensibility | `mcp-configuration.mdx` | 部分实现 | MCP 配置、发现、管理可用；stdio、本地 loopback HTTP SSE、manager 级 connect/disconnect/reconnect、短指数退避重试与 channel notification 事件路由已接入，远程 HTTPS / OAuth / HTTP / WS 仍不完整。 |
| Extensibility | `mcp-protocol.mdx` | 部分实现 | JSON-RPC stdio 主路径、本地 loopback SSE endpoint / POST 通道、`notifications/claude/channel` 路由和 manager 级短退避重试存在；认证和完整 remote transport 覆盖仍不完整。 |
| Extensibility | `skills.mdx` | 已实现 | skills frontmatter、加载、注册、调用、fork 执行与命令入口已形成闭环。 |
| Safety | `auto-mode.mdx` | 部分实现 | `PermissionMode::Auto`、fallback、classifier result adapter 和危险 allow 规则 strip/restore helper 存在；权限层可消费 fast / thinking 分类结果并移除宽泛 shell / Agent allow 规则，但 Bun 的 LLM transcript classifier runner 和运行时 mode transition strip 接线未完整落地。 |
| Safety | `permission-model.mdx` | 已实现 | allow / ask / deny 规则、mode fallback、hook overlay、session grant 已落地。 |
| Safety | `plan-mode.mdx` | 部分实现 | Enter / Exit plan mode、`/plan`、计划文件和工作流持久化存在；`allowedPrompts` 已接入 Bash pattern 与常见验证意图的确定性 session allow 规则，通用 LLM 语义 classifier 未实现。 |
| Safety | `sandbox.mdx` | 部分实现 | Linux / macOS shell sandbox、网络 / 路径预检、`/sandbox` 命令存在；Windows OS-level sandbox 未实现。 |
| Safety | `why-safety-matters.mdx` | 已实现 | prompt、permissions、hooks、sandbox、plan mode 的纵深防御链路已可映射。 |
| Tools | `what-are-tools.mdx` | 已实现 | Tool trait、schema、registry、权限、结果处理已接入。 |
| Tools | `file-operations.mdx` | 已实现 | Read / Edit / Write、安全写入、变更检测、历史保护均有实现。 |
| Tools | `search-and-navigation.mdx` | 已实现 | Glob、Grep、ToolSearch、LSP、WebSearch、WebFetch 已接入；Glob 已按修改时间倒序返回。 |
| Tools | `shell-execution.mdx` | 已实现 | Bash 具备危险命令检测、sandbox 预检、超时、进程控制和输出流；Rust 额外提供 PowerShell / Repl / Sleep。 |
| Tools | `task-management.mdx` | 部分实现 | Rust 已有 `TodoWrite` V1 兼容入口和 V2 Tasks 工具体系；依赖模型已暴露 Bun 兼容别名，V2 递增 ID / 高水位与基础 owner claim / agent-busy 检查已补齐，跨进程认领模型仍不同。 |

## 已实现能力汇总

- Agent：子 Agent 入口、内置 agent、同步 / 后台执行、AgentTree、hooks、fork 和 worktree 侧路。
- Context：system prompt 组装链路，包括静态段、动态段、缓存边界和 `CLAUDE.md` 注入。
- Extensibility：hooks 和 skills 两条扩展链路已经形成配置、运行时和命令入口闭环。
- Safety：权限模型和纵深防御叙事可完整映射到 cc-rust 的 prompt、permissions、hooks、sandbox、plan mode 层。
- Tools：工具抽象、文件操作、搜索导航、Shell 执行和网络工具主路径已实现。

## 部分实现 / 未实现 / 待确认汇总

- Agent Teams 是 Rust 的 in-process teammate / mailbox 版本，不是 Bun coordinator / swarm 的同构实现；tmux / iTerm2 等多终端后端属于故意裁剪。
- Worktree isolation 已有核心隔离和清理，但 Bun 的 hook 驱动创建 / 销毁、目录布局和恢复流程没有一一对齐。
- Context compaction、project memory、token budget 都已有主体能力，但 Partial Compact、provider 级 token 精确统计、Bun 智能记忆召回、近期工具去噪、已展示去重和封闭四类型分类法仍需补齐或明确裁剪；project memory 已有确定性 `MEMORY.md` 入口索引；token budget 已有结构化启发式诊断报告但 `exact_count_available=false`；session-insights 回注已支持 workspace、时间窗口、tag 和当前 session 排除过滤，生命周期抽取已改用确定性 helper；Microcompact Boundary 已记录就地工具结果压缩事件，preservedSegment 目前已覆盖手动 `/compact` boundary、Session Memory Compact boundary、自动模型摘要 boundary 以及内部 snip/context-collapse boundary。
- Custom agents 已可定义、编辑、运行，但独立安全边界不如 hooks / skills 明确。
- MCP 当前已有 stdio JSON-RPC、本地 loopback HTTP SSE 主路径、channel notification 事件路由、manager 级 reconnect API 与短指数退避重试；上层 `/mcp reconnect` 接线、远程 HTTPS SSE、OAuth、长线自动重连和完整 transport 矩阵仍不完整。
- Auto mode 已有 classifier result adapter，可消费 fast / thinking 的 allow / deny / ask / unavailable 结果；危险 allow 规则剥离/恢复 helper 已能移除宽泛 `Bash` / `PowerShell` / `Agent` 规则、解释器 / package runner / SSH / elevation 前缀和 PowerShell `.exe` 形态；仍缺 Bun 的 LLM transcript classifier runner、prompt 模板、API 调用链，以及进入 / 退出 Auto mode 的运行时 strip / restore 接线。
- Plan mode 已补入 `allowedPrompts` 输入、session allow bridge 和常见验证意图分类；仍缺 Bun 的通用 LLM 语义 classifier。
- Windows OS-level sandbox 未实现；当前 Windows 侧主要是 Rust-level policy checks。
- Tools 的 `TodoWrite` 与 V2 Tasks 均已接入；V2 已补入双向依赖兼容输出、递增 ID、高水位文件和基础 owner claim / agent-busy 检查，但仍与 Bun 的跨进程任务列表锁、teammate 退出重置和完整 task-list-id 解析不同。

## 后续动作

1. 优先确认 Safety 的未实现项：Windows OS-level sandbox、`allowedPrompts` 通用 LLM classifier、Auto mode LLM classifier runner 与运行时 strip / restore 接线。
2. 其次确认 MCP transport 与协议安全：`/mcp reconnect` 接入 manager API、远程 HTTPS SSE、OAuth、长线断线恢复、完整 server / resource 行为。
3. 再确认 Context 端到端链路：Partial Compact、精确 token 统计、智能相关记忆召回。
4. 对 Agent Teams 明确产品边界：继续保留 in-process 版本，还是补 coordinator / swarm 同构模式。
5. 对 Tools 差异建立单独 issue：V2 Tasks 是否要补 Bun 的跨进程任务列表锁、teammate 退出 owner 重置、完整 task-list-id 解析，WebFetch 是否需要 JS rendering。
6. 后续进入实现补齐时，为每个改动建立单独任务，不在本文档中混入代码设计细节。

## 实施进度

| 日期 | 任务 | 状态 | 证据 | 验证 |
| --- | --- | --- | --- | --- |
| 2026-05-05 | Safety / Plan mode `allowedPrompts` | 已完成确定性 Bash pattern bridge；语义 classifier 仍部分实现 | `crates/claude-code-rs/src/tools/plan_mode.rs` 接受 `allowedPrompts`，批准后写入 `plan_allowed_prompts` session rules | `cargo test -p claude-code-rs tools::plan_mode::tests:: -- --nocapture`，10 passed |
| 2026-05-05 | Safety / Sandbox `allowedCommands` | 已完成 workspace sandbox command allow bridge | `crates/claude-code-rs/src/tools/execution/security.rs` 统一判断，`engine/lifecycle/deps.rs` 与 `tools/execution/pipeline.rs` 接入 central permission fallback | `cargo test -p cc-sandbox allowed_command -- --nocapture` passed；`cargo test -p claude-code-rs central_permission_sandbox_allowed_command -- --nocapture` 被当前工作树未提交的 `crates/cc-compact/src/context_collapse.rs` 编译错误阻塞 |
| 2026-05-05 | Extensibility / MCP SSE config safety | 已完成远程 SSE 配置安全校验；SSE runtime 仍部分实现 | `crates/cc-mcp/src/client.rs` 在 `sse` connect 前校验 URL 与 headers，拒绝非 loopback 明文 HTTP、缺失 URL、CR/LF header 注入 | `cargo test -p cc-mcp sse -- --nocapture`，4 passed |
| 2026-05-05 | Tools / Glob mtime ordering | 已完成 Bun 文档排序语义补齐 | `crates/claude-code-rs/src/tools/fs/glob_tool.rs` 收集文件修改时间，按修改时间倒序返回，并以路径升序作为稳定兜底 | `cargo test -p claude-code-rs tools::fs::glob_tool::tests:: -- --nocapture`，12 passed |
| 2026-05-05 | Tools / TodoWrite V1 compatibility | 已完成独立 `TodoWrite` 工具入口；V2 任务模型仍部分实现 | `crates/claude-code-rs/src/tools/tasks.rs` 增加 V1 全量替换 todo store、全部完成清空与验证提示；`registry.rs` 与 `tool_search.rs` 接入 | `cargo test -p claude-code-rs todo_write -- --nocapture`，4 passed；`cargo test -p claude-code-rs tools::registry::tests::test_find_tool_by_name -- --nocapture`，1 passed |
| 2026-05-05 | Context / Memory prompt injection | 已完成 Project / Global / Team memory 主提示词注入；auto-memory 与 session-insights 回注仍部分实现 | `crates/claude-code-rs/src/engine/system_prompt.rs` 调用 `cc_session::memdir::build_memory_context()` 并注入 `# Memory Context` | `cargo test -p claude-code-rs engine::system_prompt::tests:: -- --nocapture`，30 passed |
| 2026-05-05 | Context / Auto-memory prompt gate | 已完成 `autoMemoryEnabled` 控制的 Auto memory 主提示词注入；session-insights 回注仍部分实现 | `crates/claude-code-rs/src/engine/lifecycle/submit_message.rs` 与 `startup/fast_paths.rs` 将设置传入 `build_system_prompt()`，`system_prompt.rs` 调用 `build_memory_context_with()` | `cargo test -p claude-code-rs engine::system_prompt::tests:: -- --nocapture`，31 passed |
| 2026-05-05 | Context / Session insights prompt replay | 已完成最近 session-insights 主提示词回放；作用域过滤与抽取策略仍部分实现 | `crates/cc-services/src/session_memory.rs` 格式化 `<session-insights>`，`submit_message.rs` 与 `fast_paths.rs` 将其传入 `build_system_prompt_with_session_memory()` | `cargo test -p cc-services session_memory -- --nocapture`，6 passed；`cargo test -p claude-code-rs engine::system_prompt::tests:: -- --nocapture`，32 passed |
| 2026-05-05 | Context / Session insights workspace scope | 已完成 session-insights 按当前 workspace 回放；抽取策略仍部分实现 | `crates/cc-services/src/session_memory.rs` 为条目记录 `workspace` 并按 workspace 过滤，`engine/lifecycle/mod.rs` 保存当前 cwd | `cargo test -p cc-services session_memory -- --nocapture`，7 passed；`cargo test -p claude-code-rs engine::system_prompt::tests:: -- --nocapture`，32 passed |
| 2026-05-05 | Context / Dynamic context window | 已完成 `CLAUDE_CODE_MAX_CONTEXT_TOKENS` 与 `[1m]` 窗口解析；provider 级精确 token 统计仍部分实现 | `crates/cc-utils/src/tokens.rs` 解析动态窗口，`crates/cc-compact/src/auto_compact.rs` 复用同一入口 | `cargo test -p cc-utils tokens -- --nocapture`，11 passed；`cargo test -p cc-compact auto_compact -- --nocapture`，6 passed |
| 2026-05-05 | Context / Session Memory Compact | 已完成无 API 的 session-insights 压缩优先分支；preservedSegment 与 Partial Compact 仍部分实现 | `crates/cc-compact/src/session_memory_compact.rs` 生成 session-memory 摘要和最近窗口，`engine/lifecycle/deps.rs` 在 auto-compact 触发时优先接入 | `cargo test -p cc-compact session_memory_compact -- --nocapture`，3 passed；`cargo test -p claude-code-rs engine::lifecycle::deps -- --nocapture`，11 passed |
| 2026-05-05 | Context / Compact preserved segment metadata | 已完成手动 `/compact` boundary 的 preservedSegment 注解；Partial Compact 与自动压缩边界覆盖仍部分实现 | `crates/cc-types/src/message.rs` 增加 `PreservedSegment`，`cc-compact/src/compaction.rs` 生成 preserved segment，`claude-code-rs/src/commands/compact.rs` 在 boundary 中记录摘要消息和保留消息 UUID | `cargo test -p cc-compact compaction -- --nocapture`，9 passed；`cargo test -p claude-code-rs commands::compact -- --nocapture`，4 passed；`cargo test -p claude-code-rs daemon::routes::tests::daemon_sse_broadcasts_compact_boundaries -- --nocapture`，1 passed；`cargo test -p cc-session session_export -- --nocapture`，13 passed |
| 2026-05-05 | Safety / Plan mode `allowedPrompts` classifier | 已完成常见验证意图到 Cargo allow 规则的确定性分类；通用 LLM classifier 仍部分实现 | `crates/claude-code-rs/src/tools/plan_mode.rs` 将 “run tests and lint”等提示映射为 `Bash(cargo test*)` / `Bash(cargo clippy*)`，显式 Bash pattern 仍直通，否定意图会被拒绝 | `cargo test -p claude-code-rs tools::plan_mode::tests:: -- --nocapture`，13 passed；`cargo test -p cc-permissions session_grant -- --nocapture`，4 passed |
| 2026-05-05 | Context / Session insights age window | 已完成 session-insights 回注的默认 30 天时间窗口；抽取内容策略仍部分实现 | `crates/cc-services/src/session_memory.rs` 为 `SessionMemoryConfig` 增加 `max_context_age_seconds`，`format_memory_context_for_workspace()` 同时按 workspace 与年龄过滤 | `cargo test -p cc-services session_memory -- --nocapture`，8 passed |
| 2026-05-05 | Tools / Task dependency aliases | 已完成 V2 Tasks 的 Bun 依赖兼容面；递增 ID / 高水位 / 认领竞争仍部分实现 | `crates/claude-code-rs/src/tools/tasks.rs` 接受 `blocked_by` / `blockedBy` 创建别名，并在任务 JSON 中输出 `depends_on`、`blocked_by`、`blockedBy` 与反向 `blocks` | `cargo test -p claude-code-rs tools::tasks::tests:: -- --nocapture` 被当前工作树未提交的 `ToolExecResult.hook_stopped_continuation` 构造点缺字段错误阻塞 |
| 2026-05-06 | Extensibility / MCP loopback SSE runtime | 已完成本地 loopback HTTP SSE endpoint / POST 主路径；远程 HTTPS / OAuth / reconnect 仍部分实现 | `crates/cc-mcp/src/client.rs` 支持 loopback SSE GET、endpoint 解析、JSON-RPC POST；`crates/cc-mcp/src/transport.rs` 分发 SSE `message` 响应 | `cargo check -p cc-mcp`；`cargo test -p cc-mcp sse -- --nocapture`，5 passed；`cargo test -p cc-mcp --lib`，31 passed |
| 2026-05-06 | Extensibility / MCP manager reconnect API | 已完成 manager 层单服务 connect/disconnect/reconnect；上层 `/mcp reconnect` / IPC 接线仍部分实现 | `crates/cc-mcp/src/manager.rs` 暴露 `connect_server()`、`disconnect_server()`、`reconnect_server()`，重连失败不会保留 stale client | `cargo check -p cc-mcp`；`cargo test -p cc-mcp manager -- --nocapture`，5 passed；`cargo test -p cc-mcp --lib`，34 passed |
| 2026-05-06 | Context / Session insights tag filtering | 已完成 session-insights 回注 include/exclude tag 过滤；抽取内容策略仍部分实现 | `crates/cc-services/src/session_memory.rs` 为 `SessionMemoryConfig` 增加 `context_include_tags` / `context_exclude_tags`，回注时 exclude 优先、include 为空则不过滤 | `cargo check -p cc-services`；`cargo test -p cc-services session_memory -- --nocapture`，9 passed；`cargo test -p cc-services --lib`，45 passed |
| 2026-05-06 | Context / Session insight extraction helper | 已完成 `cc-services` 内确定性 insight 抽取 helper；`try_extract_session_memory()` 生命周期接入仍部分实现 | `crates/cc-services/src/session_memory.rs` 增加 `extract_session_insight()`，保留用户意图、跳过短确认语、推断 tags，并按字符边界截断 | `cargo check -p cc-services`；`cargo test -p cc-services session_memory -- --nocapture`，12 passed；`cargo test -p cc-services --lib`，48 passed |
| 2026-05-06 | Context / Session insight lifecycle extraction | 已完成 `try_extract_session_memory()` 接入确定性 insight helper；当前 session 过滤仍待确认 | `crates/claude-code-rs/src/engine/lifecycle/mod.rs` 从最近用户意图与 assistant 文本生成 session insight，保存 helper 推断的 tags | `cargo check -p claude-code-rs`；`cargo test -p claude-code-rs test_try_extract_session_memory_uses_structured_insight -- --nocapture`，1 passed |
| 2026-05-06 | Tools / Task incremental IDs and highwater | 已完成 V2 Tasks 的递增 ID 与高水位防复用；任务认领竞争仍部分实现 | `crates/claude-code-rs/src/tools/tasks.rs` 通过 `.highwatermark` / `.highwatermark.lock` 分配新任务 ID，删除后不复用，旧 numeric task 可 bootstrap 下一 ID | `cargo check -p claude-code-rs`；`cargo test -p claude-code-rs tools::tasks::tests:: -- --nocapture`，40 passed |
| 2026-05-06 | Tools / Task owner claim checks | 已完成基础 owner claim、blocked dependency 和 agent-busy 检查；跨进程任务列表锁仍部分实现 | `crates/claude-code-rs/src/tools/tasks.rs` 持久化 `owner`，`TaskUpdate(status="in_progress")` 走 `claim_task()`，返回 `task_not_found` / `already_claimed` / `already_resolved` / `blocked` / `agent_busy` 等 reason | `cargo test -p claude-code-rs tools::tasks::tests:: -- --nocapture`，43 passed；`cargo check -p claude-code-rs` 通过但当前工作树的 `api/google_provider.rs` 脏改动仍产生 4 个既有 warning |
| 2026-05-06 | Context / Session insights current-session filter | 已完成 session-insights 回注排除当前 session；Bun 智能记忆召回仍部分实现 | `crates/cc-services/src/session_memory.rs` 新增 `format_memory_context_for_workspace_excluding_session()`，`submit_message.rs` 与 `deps.rs` 在正常提示词和 auto-compact 路径传入当前 session ID | `cargo test -p cc-services session_memory -- --nocapture`，13 passed；`cargo check -p cc-services`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Context / Internal compact preservedSegment | 已完成 snip 与 context-collapse 内部 boundary preservedSegment；Partial Compact 仍部分实现 | `crates/cc-compact/src/snip.rs` 与 `crates/cc-compact/src/context_collapse.rs` 将 boundary UUID 作为 summary，并记录首条保留消息与最近窗口消息 UUID | `cargo test -p cc-compact snip -- --nocapture`，2 passed；`cargo test -p cc-compact context_collapse -- --nocapture`，3 passed；`cargo test -p cc-compact compaction -- --nocapture`，9 passed；`cargo check -p cc-compact`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Context / Auto compact model boundary | 已完成自动模型摘要压缩 boundary preservedSegment；Partial Compact 仍部分实现 | `crates/cc-compact/src/compaction.rs` 新增 `build_post_compact_messages_with_boundary()`，`engine/lifecycle/deps.rs` 在模型摘要 auto-compact 成功后返回 boundary + 摘要 + 恢复上下文 | `cargo test -p cc-compact compaction -- --nocapture`，10 passed；`cargo check -p cc-compact`；`cargo check -p claude-code-rs` 通过但当前未提交的 `api/providers.rs` 仍有 1 个 unrelated dead_code warning |
| 2026-05-06 | Context / Session Memory Compact boundary | 已完成无 API session-insights 压缩 boundary preservedSegment；Partial Compact 仍部分实现 | `crates/cc-compact/src/session_memory_compact.rs` 返回 boundary + session-insights 摘要 + 最近窗口，boundary preservedSegment 记录摘要消息 UUID 和保留窗口 UUID | `cargo test -p cc-compact session_memory_compact -- --nocapture`，3 passed；`cargo test -p cc-compact compaction -- --nocapture`，10 passed；`cargo check -p cc-compact`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Context / Microcompact Boundary | 已完成旧工具结果就地压缩的轻量 boundary；Partial Compact 仍部分实现 | `cc-types/src/message.rs` 增加 `MicrocompactBoundary` / `MicrocompactMetadata`，`cc-compact/src/microcompact.rs` 在替换旧大型 tool_result 后追加 boundary 并记录 `compacted_tool_ids` | `cargo test -p cc-compact microcompact -- --nocapture`，3 passed；`cargo test -p cc-compact pipeline -- --nocapture`，5 passed；`cargo check -p cc-compact`；`cargo check -p cc-session`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Extensibility / MCP manager retry backoff | 已完成 manager 级短指数退避重试；上层 reconnect 接线与长线断线恢复仍部分实现 | `crates/cc-mcp/src/manager.rs` 在 `connect_server()` 内通过 `connect_ready_client_with_retries()` 对连接/初始化失败重试 3 次，退避 50/100/200ms 并封顶 250ms | `cargo test -p cc-mcp manager -- --nocapture`，5 passed；`cargo test -p cc-mcp connect_retry_delay -- --nocapture`，1 passed；`cargo test -p cc-mcp --lib`，35 passed；`cargo check -p cc-mcp` |
| 2026-05-06 | Context / Token usage report | 已完成结构化启发式 token 预算诊断；provider 级精确 `countTokens` 仍部分实现 | `crates/cc-utils/src/tokens.rs` 增加 `TokenUsageReport` / `TokenCountMethod` 与 `estimate_context_usage()`，报告估算量、窗口、阈值、剩余量、利用率、超阈值状态，并显式标记 `exact_count_available=false` | `cargo test -p cc-utils tokens -- --nocapture`，12 passed；`cargo check -p cc-utils`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Context / Memory entrypoint index | 已完成确定性 `MEMORY.md` 入口索引；Sonnet 智能召回与封闭四类型分类法仍部分实现 | `crates/cc-session/src/memdir.rs` 写入/删除记忆时刷新 `MEMORY.md`，索引限制 200 行 / 25KB 并在 `<memory-context>` 每个非空 scope 前注入 `### MEMORY.md Index` | `cargo test -p cc-session memdir -- --nocapture`，15 passed；`cargo test -p cc-session --lib`，71 passed；`cargo check -p cc-session`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Extensibility / MCP channel notifications | 已完成 `notifications/claude/channel` 事件路由；完整远程 transport / auth 仍部分实现 | `crates/cc-mcp/src/transport.rs` 在 stdio 与 SSE notification 路径解析 channel payload，`cc-mcp/src/lib.rs` 增加 `McpSubsystemEvent::ChannelNotification`，`ipc/runtime.rs` 适配为 `McpEvent::ChannelNotification` | `cargo test -p cc-mcp notification_event -- --nocapture`，3 passed；`cargo test -p cc-mcp --lib`，38 passed；`cargo check -p cc-mcp`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Safety / Auto mode classifier adapter | 已完成权限层 classifier result adapter；LLM transcript classifier runner 仍部分实现 | `crates/cc-permissions/src/decision.rs` 增加 `AutoClassifierDecision` / `AutoClassifierStage` / `AutoClassifierVerdict` 与 `has_permissions_to_use_tool_with_hook_and_auto_classifier()`，支持 allow / deny / ask / unavailable / transcript-too-long 到权限决策的降级映射 | `cargo test -p cc-permissions auto_mode_classifier -- --nocapture`，4 passed；`cargo test -p cc-permissions --lib`，98 passed；`cargo check -p cc-permissions`；`cargo check -p claude-code-rs` |
| 2026-05-06 | Safety / Auto mode dangerous allow rules | 已完成权限层危险 allow 规则 strip/restore helper；进入 / 退出 Auto mode 的运行时接线仍部分实现 | `crates/cc-permissions/src/dangerous.rs` 增加 `strip_dangerous_permissions_for_auto_mode()` / `restore_dangerous_permissions_after_auto_mode()`，剥离宽泛 `Bash` / `PowerShell` / `Agent` allow 规则和 `python` / `node` / `npm run` / `npx` / `ssh` / `sudo` / `Invoke-Expression` / `Start-Process` / `Add-Type` 等 code execution / elevation 前缀，并覆盖 PowerShell `.exe` 形态，同时保留窄规则 | `cargo test -p cc-permissions auto_mode -- --nocapture`，9 passed；`cargo test -p cc-permissions --lib`，101 passed；`cargo check -p cc-permissions`；`cargo check -p claude-code-rs` |
