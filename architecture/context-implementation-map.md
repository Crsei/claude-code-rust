# Context Implementation Map

本文只覆盖 `docs/bun-docs-documentation-plan.md` 中的 `Context` 章节，映射 Bun 上游 `docs/context/*.mdx` 与 cc-rust 当前实现之间的对应关系。本文只做文档核对，不判断代码质量，也不扩展到 `agent`、`extensibility`、`safety`、`tools` 章节。

## 范围

- 只看 Bun 上游这四篇文档：
  - `compaction.mdx`
  - `project-memory.mdx`
  - `system-prompt.mdx`
  - `token-budget.mdx`
- 只核查 cc-rust 当前实现，不推断未来规划。
- 只记录能从源码直接证明的结论；无法从源码闭环证明的内容标为 `待确认`。
- 状态只使用以下五类：`已实现`、`部分实现`、`未实现`、`待确认`、`故意裁剪`。

## 上游文档清单

| Bun 文档 | 主题 | 本次核查重点 |
| --- | --- | --- |
| `compaction.mdx` | 上下文压缩、boundary、PTL 恢复、hook | 输入消息如何压缩、边界如何落盘、是否有自动/响应式回退 |
| `project-memory.mdx` | 项目记忆、全局记忆、回忆与注入 | 记忆目录、CRUD、回忆入口、是否注入到系统提示词 |
| `system-prompt.mdx` | 系统提示词拼装、缓存边界、CLAUDE.md 注入 | 静态段/动态段、缓存边界、覆盖与追加顺序 |
| `token-budget.mdx` | token 预算、阈值、截断、恢复 | 预算判断、阈值触发、估算 vs 精确统计 |

## 实现映射表

| Bun 文档 | cc-rust 主要入口 | 状态 | 核心结论 |
| --- | --- | --- | --- |
| `compaction.mdx` | [`cc-compact/src/pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`cc-compact/src/session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs), [`cc-compact/src/compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L99), [`claude-code-rs/src/query/loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202) | 部分实现 | 本地压缩、Microcompact Boundary、Session Memory Compact、boundary、手动 `/compact` / Session Memory Compact / 自动模型摘要 / 内部 snip/context-collapse preservedSegment 元数据、PTL 恢复、hook 已有；feature gate、Partial Compact 与 Bun 的完整恢复语义仍未完全同构。 |
| `project-memory.mdx` | [`cc-session/src/memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L72), [`cc-config/src/claude_md.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/claude_md.rs#L51), [`claude-code-rs/src/engine/system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs) | 部分实现 | 记忆 CRUD、`MEMORY.md` 入口索引、封闭 `user/feedback/project/reference` 类型元数据、`CLAUDE.md` 注入、Project/Global/Team memory 主提示词注入都存在；Auto memory 已由 `auto_memory_enabled` 门控注入，最近 session-insights 会按 workspace、时间窗口、可配置 tag 和当前 session 排除规则回注；生命周期抽取已改用确定性 insight helper。 |
| `system-prompt.mdx` | [`claude-code-rs/src/engine/system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L361), [`claude-code-rs/src/engine/prompt_sections.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/prompt_sections.rs#L17), [`claude-code-rs/src/engine/lifecycle/submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs#L238) | 已实现 | 静态段、动态段、缓存边界、`CLAUDE.md` 注入、append/override 顺序都已落地。 |
| `token-budget.mdx` | [`cc-utils/src/tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs), [`cc-compact/src/auto_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/auto_compact.rs), [`claude-code-rs/src/query/token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9), [`claude-code-rs/src/api/client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs) | 部分实现 | 预算判断、续跑逻辑、`CLAUDE_CODE_MAX_CONTEXT_TOKENS` 与 `[1m]` 窗口解析存在；`estimate_context_usage()` 仍提供离线启发式诊断，API 层已新增 Anthropic/Azure/Gemini provider exact 计数诊断路径，但 auto-compact 阈值仍未接入 near-threshold exact fallback。 |

## 逐文档分析

### `compaction.mdx`

**上游关注点**

- Bun 把压缩拆成多层：局部工具结果裁剪、会话压缩、传统摘要、boundary 标记、PTL 紧急恢复。

**cc-rust 当前实现**

- `run_context_pipeline()` 已实现工具结果预算、snip、microcompact、自动压缩检查四步主流程。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65)
- `microcompact_messages()` 会对旧的大型工具结果做就地摘要，并在发生替换时追加 `MicrocompactBoundary` 系统消息，记录 `trigger`、`pre_tokens`、`tokens_saved`、`compacted_tool_ids` 和 `cleared_attachment_uuids`。[`microcompact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/microcompact.rs#L26), [`message.rs`](F:/AIclassmanager/cc/rust/crates/cc-types/src/message.rs#L120)
- `should_auto_compact()` 明确跳过 `compact` 和 `session_memory` 来源，避免递归触发。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L99)
- `session_memory_compact_if_needed()` 已提供无 API 的 Session Memory Compact：使用 `<session-insights>` 作为旧历史摘要，保留 10K-40K token 的最近窗口，在保留 tool_result 时向前纳入对应 tool_use，并返回带 preservedSegment 的 CompactBoundary。[`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs#L50), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs#L87)
- `QueryEngineDeps::autocompact()` 在 auto-compact 触发且当前 workspace 有 session-insights 时，优先走 Session Memory Compact，再回退到模型摘要或本地管线。[`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs)
- `build_post_compact_messages()` 会在摘要后重建上下文，并恢复最近文件引用；`build_post_compact_messages_with_boundary()` 会为模型摘要自动压缩预置 CompactBoundary，并在 boundary preservedSegment 中记录摘要消息 UUID 与恢复上下文消息 UUID。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L144), [`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L188)
- `create_compact_boundary()` 和 `get_messages_after_compact_boundary()` 已提供 boundary 生成与回溯能力。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L184), [`messages.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/messages.rs#L106)
- `CompactMetadata` 已包含 `preserved_segment`。手动 `/compact` 生成的 boundary 会记录摘要消息 UUID 和本地管线保留消息 UUID；Session Memory Compact boundary 会记录 session-insights 摘要消息 UUID 和最近窗口消息 UUID；自动模型摘要 boundary 会记录摘要消息 UUID 和恢复上下文消息 UUID；snip 与 context-collapse 生成的内部 boundary 也会把 boundary UUID 作为 summary，并记录被保留的首条消息与最近窗口消息 UUID。[`message.rs`](F:/AIclassmanager/cc/rust/crates/cc-types/src/message.rs#L130), [`compact.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/compact.rs#L67), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs#L87), [`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L188), [`snip.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/snip.rs#L87), [`context_collapse.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/context_collapse.rs#L81)
- `handle_prompt_too_long()` 提供 PTL 重试路径；`reactive_compact()` 失败时会回退到终态。[`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202), [`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L154)
- `QueryEngineDeps::autocompact()` 在有 API client 时会额外调用模型生成摘要，再拼出带 CompactBoundary 的 post-compact 消息。[`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs#L258), [`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs#L373)

**状态判断**

- `部分实现`。
- 原因不是“没有压缩”，而是压缩主链路、Microcompact Boundary、Session Memory Compact、手动 `/compact` boundary、Session Memory Compact boundary、自动模型摘要 boundary 和内部 snip/context-collapse boundary 的 preservedSegment 注解都已经存在；差异点在于 Bun 文档里的 feature gate 组合、Partial Compact 和某些恢复策略，在当前 Rust 实现里没有看到完整的一一对应。

### `project-memory.mdx`

**上游关注点**

- Bun 的项目记忆同时包含文件级持久化、回忆、注入和自动收集。

**cc-rust 当前实现**

- `cc-session::memdir` 已实现四个 scope：`Global`、`Project`、`Team`、`Auto`，并且给出明确路径映射。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L38), [`paths.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/paths.rs#L96)
- `cc-session::memdir` 现在也有 Bun 对齐的封闭四类型分类：`MemoryType::{User, Feedback, Project, Reference}`，JSON 序列化字段为 `type`，并兼容旧 `category` 字段作为 fallback；`MEMORY.md` 索引和 `<memory-context>` 注入会显示有效类型标签。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs)
- `write_memory()`、`read_memory()`、`delete_memory()`、`list_memories()`、`search_memories()` 都已实现；写入和删除会刷新记忆目录下的 `MEMORY.md` 入口索引，最后一条记忆删除后会清理空索引。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L318), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L348)
- `MEMORY.md` 索引由 `build_memory_index()` / `refresh_memory_index()` 管理，格式为链接列表，单条 hook 取记忆正文首个非空行并截断到 150 字符；读取和生成时限制为 200 行、25KB，超限时追加截断提示。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L18), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L235), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L251)
- `build_memory_context_with()` 已能把 project/global/team/auto 记忆组装成 `<memory-context>`，并且 team/auto 有门控条件；每个非空 scope 会先注入 `### MEMORY.md Index`，若磁盘上没有旧索引则按当前 JSON 记忆即时生成同等索引。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L270), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L437)
- `build_system_prompt()` 现在会把 `build_memory_context_with()` 生成的 Project / Global / Team memory 注入到 `# Memory Context` 段落，并在 `auto_memory_enabled` 为 true 时纳入 Auto memory，形成主提示词端到端路径。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs)
- `/memory` 命令已经把四个 scope 暴露到 UI/CLI，并支持查看、写入、删除、搜索和打开目录。[`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L41), [`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L214)
- `build_claude_md_context()` 会把祖先目录里的 `CLAUDE.md` 合并进上下文，且 `build_system_prompt()` 会把这段内容注入到系统提示词里。[`claude_md.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/claude_md.rs#L51), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L517)
- 另有独立的 `SessionMemoryService`，会把对话中抽取的简要 insight 持久化到 `~/.cc-rust/session-insights/`，新条目记录 workspace；`submit_message` 会把当前 workspace 最近 5 条、默认 30 天内、满足 tag include/exclude 规则且不属于当前 session 的条目格式化为 `<session-insights>` 并回注到 `# Memory Context`。`--dump-system-prompt` 没有活动 session，因此仍只按 workspace/time/tag 过滤。[`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs#L243), [`submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs#L356), [`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs#L289), [`fast_paths.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/startup/fast_paths.rs)
- `cc-services::session_memory::extract_session_insight()` 已提供确定性抽取 helper：会把最近用户意图和 assistant 结论折叠为短 insight，跳过过短确认语，按关键字推断 `implementation`、`testing`、`debugging`、`architecture`、`mcp`、`memory` 等 tags，并按字符边界截断。`QueryEngine::try_extract_session_memory()` 已改用该 helper 保存条目。[`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs), [`mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/mod.rs)

**状态判断**

- `部分实现`。
- `CLAUDE.md`、Project / Global / Team memory、受 `auto_memory_enabled` 门控的 Auto memory、`MEMORY.md` 入口索引、封闭四类型分类元数据和当前 workspace 最近 session-insights 都已经接入主提示词；session-insights 回注已有 workspace、时间窗口、可配置 tag 与当前 session 排除过滤，生命周期抽取也已改用确定性 insight helper。剩余差异主要是 Bun 的 Sonnet 智能记忆召回、近期工具去噪和已展示去重没有同构实现。

### `system-prompt.mdx`

**上游关注点**

- Bun 强调系统提示词的静态段/动态段、缓存边界、`CLAUDE.md`、append/override 优先级，以及缓存失效控制。

**cc-rust 当前实现**

- `build_system_prompt()` 明确把系统提示词拆成静态段、`DYNAMIC_BOUNDARY`、动态段、`CLAUDE.md` 注入和 append prompt。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L361)
- `prompt_sections.rs` 提供 `cached_section()`、`uncached_section()`、`resolve_sections()` 和 `clear_cache()`，对应缓存段与动态段管理。[`prompt_sections.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/prompt_sections.rs#L51)
- `build_effective_system_prompt()` 处理 override > agent > custom > default 的优先级，append prompt 则在末尾追加。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L566)
- `submit_message.rs` 把 `build_system_prompt()` 的结果带入 `QueryParams`，再由 `build_messages_request()` 序列化为 API 的 system blocks。[`submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs#L238), [`helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/helpers.rs#L100)
- `build_system_prompt()` 还返回 `user_context` / `system_context`，对应 Bun 文档里“上下文输入来源”的分离概念。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L552)

**状态判断**

- `已实现`。
- 这是本次核查里最接近 Bun 文档原样结构的一块：静态段、动态段、边界、缓存与注入都能在 Rust 树中找到明确实现。

### `token-budget.mdx`

**上游关注点**

- Bun 将 token 预算拆成窗口估算、阈值触发、输出保留、截断和恢复策略。

**cc-rust 当前实现**

- `estimate_messages_tokens()` 使用启发式 token 估算，`is_over_token_limit()` 用 80% 窗口阈值判断是否超限；窗口大小支持 `CLAUDE_CODE_MAX_CONTEXT_TOKENS` 覆盖和模型名 `[1m]` 后缀。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs)
- `estimate_context_usage()` 已把当前预算状态暴露为 `TokenUsageReport`：包含估算 token、context window、80% 阈值、距阈值剩余量、利用率、是否超阈值、计数方法、是否有精确计数和 provider 字段；同步本地路径仍标记为 `TokenCountMethod::Heuristic` 且 `exact_count_available=false`，API 层 provider exact 路径可生成 `TokenCountMethod::ProviderExact` 报告。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs#L38), [`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs#L141), [`client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs)
- `ApiClient::count_input_tokens_exact()` 已提供 Anthropic/Azure/Gemini 精确计数入口；`CC_RUST_EXACT_TOKEN_DIAGNOSTICS=1` 时，查询发送前会记录 provider exact token diagnostics，失败则回退继续发送原请求。[`client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs), [`google_provider.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/google_provider.rs), [`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs)
- `should_auto_compact()` 也采用 80% 阈值，并复用 `cc-utils` 的动态窗口解析，因此 1M 模型不会在 200K 附近误触发压缩。[`auto_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/auto_compact.rs)
- `check_token_budget()` 处理任务预算：低于阈值时发出 nudge 继续，高于阈值时停止，并记录连续继续与递减收益。[`token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9), [`state.rs`](F:/AIclassmanager/cc/rust/crates/cc-types/src/state.rs#L68)
- `query/loop_impl.rs` 在主循环里调用 `check_token_budget()`，并把 `Continue::TokenBudgetContinuation` 注入回消息流。[`loop_impl.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_impl.rs#L622)
- `handle_max_output_tokens()` 提供输出上限的升级与恢复消息，属于另一条和 token budget 相邻但不同的恢复链路。[`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202)
- `build_messages_request()` 会根据模型夹紧 `max_tokens`，但这属于请求侧保护，不是 Bun 文档里的精确 token 统计逻辑。[`helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/helpers.rs#L194)

**状态判断**

- `部分实现`。
- 当前实现已经有预算判断、动态窗口解析、结构化启发式诊断、恢复动作，以及 Anthropic/Azure/Gemini provider exact 诊断入口；仍是 `部分实现`，因为 auto-compact near-threshold exact fallback、OpenAI/Bedrock/Vertex provider parity 和更完整的 provider failure fallback 测试还没有闭环。

## 已实现汇总

- 系统提示词拼装链路已经落地：静态段、动态段、缓存边界、`CLAUDE.md` 注入、append/override 顺序都能在源码里直接定位。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L361), [`prompt_sections.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/prompt_sections.rs#L51)
- 记忆存储、命令面和主提示词注入已经实现：`cc-session::memdir` 支持四个 scope 的 CRUD、搜索、`MEMORY.md` 入口索引和封闭 `user/feedback/project/reference` 类型元数据，`/memory` 也能查看、编辑和打开这些目录；Project / Global / Team memory 会进入 `# Memory Context`，Auto memory 会在 `auto_memory_enabled` 开启时进入同一段落，当前 workspace、默认 30 天内且满足 tag include/exclude 规则、并排除当前 session 的最近 session-insights 也会回放到同一段落。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L318), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L251), [`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L41), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs), [`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs#L243)
- 压缩主链路已经实现：tool result budget、snip、microcompact、Microcompact Boundary、Session Memory Compact、自动压缩、boundary、preservedSegment、PTL 恢复和 hook 都有对应代码。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`microcompact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/microcompact.rs#L26), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs), [`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L188), [`snip.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/snip.rs#L87), [`context_collapse.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/context_collapse.rs#L81)
- token 预算已经接入主循环：任务预算继续/停止、max_output_tokens 恢复、自动压缩阈值和 `TokenUsageReport` 结构化启发式诊断都不是占位；Anthropic/Azure/Gemini provider exact 诊断入口也已存在，但 auto-compact 阈值仍未使用 exact fallback。[`token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9), [`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202), [`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs#L141), [`client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs)

## 未实现 / 部分实现 / 待确认 / 故意裁剪

### 部分实现

- `compaction.mdx`：已有完整压缩管线、Microcompact Boundary、Session Memory Compact、手动 `/compact` boundary、Session Memory Compact boundary、自动模型摘要 boundary 和内部 snip/context-collapse boundary preservedSegment 元数据，但 Bun 文档里的 feature gate 组合、Partial Compact 与完整恢复语义还没有看到同构实现。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`microcompact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/microcompact.rs#L26), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs#L87), [`compact.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/compact.rs#L67), [`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L188), [`snip.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/snip.rs#L87), [`context_collapse.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/context_collapse.rs#L81)
- `project-memory.mdx`：记忆 CRUD、`MEMORY.md` 入口索引、封闭四类型分类元数据、`CLAUDE.md` 注入、Project / Global / Team memory 主提示词注入、`auto_memory_enabled` 门控的 Auto memory 注入、workspace/time-window/tag scoped session-insights 回注、当前 session 排除过滤，以及确定性生命周期抽取都存在；Bun 的 Sonnet 智能召回、近期工具去噪与已展示去重仍未同构。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L251), [`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L437), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs), [`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs#L243), [`mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/mod.rs)
- `token-budget.mdx`：有预算判断、动态窗口解析、结构化启发式诊断、恢复和 Anthropic/Azure/Gemini provider exact 诊断入口；auto-compact near-threshold exact fallback、OpenAI/Bedrock/Vertex provider parity 和 failure fallback 覆盖仍未完成。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs), [`client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs), [`google_provider.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/google_provider.rs)

### 待确认

- `build_system_prompt()` 返回的 `user_context` / `system_context` 当前只看到在 `QueryParams` 里流转；`build_messages_request()` 只序列化 `system_prompt`、messages 和 tools。需要确认这是有意保留的元数据，还是后续还要展开成独立输入层。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L552), [`helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/helpers.rs#L100)
- 当前未发现需要继续标成“待确认”的 session-insights 回注过滤项；服务层已有 workspace/time/tag/current-session 过滤，dump-system-prompt 因无活动 session 只应用 workspace/time/tag 过滤。[`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs#L243), [`submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs#L356)

### 未实现

- 在本次 `context` 范围内，仍未完成 auto-compact near-threshold exact fallback、OpenAI/Bedrock/Vertex provider exact parity，以及 provider exact failure fallback 的完整阈值测试。Anthropic/Azure/Gemini 计数入口已进入 `部分实现` 范围。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs), [`client/mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/api/client/mod.rs)

### 故意裁剪

- 未发现能从源码直接证明的“故意裁剪”项。当前更像是实现分叉和接入层缺口，而不是显式声明过的缩减。

## 后续动作

1. 如果要继续对齐 Bun 的 `project-memory` 语义，下一步应评估是否补 Bun 的智能相关记忆召回、近期工具去噪、已展示去重和更完整的 prompt 回注端到端测试；封闭四类型分类元数据已补入。
2. 如果要继续对齐 `token-budget` 语义，下一步应把 Anthropic/Azure/Gemini exact count 从诊断路径接入 near-threshold auto-compact fallback，并重新评估 OpenAI/Bedrock/Vertex provider parity。
3. 如果要继续写 `extensibility`、`safety`、`tools` 章节，建议沿用同样的结构：上游文档清单、实现映射表、逐文档分析、汇总、缺口、后续动作。
