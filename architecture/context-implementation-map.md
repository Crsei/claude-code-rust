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
| `compaction.mdx` | [`cc-compact/src/pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`cc-compact/src/session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs), [`cc-compact/src/compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L99), [`claude-code-rs/src/query/loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202) | 部分实现 | 本地压缩、Session Memory Compact、boundary、PTL 恢复、hook 已有；preservedSegment 注解、feature gate 与 Bun 的完整恢复语义仍未完全同构。 |
| `project-memory.mdx` | [`cc-session/src/memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L72), [`cc-config/src/claude_md.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/claude_md.rs#L51), [`claude-code-rs/src/engine/system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs) | 部分实现 | 记忆 CRUD、`CLAUDE.md` 注入、Project/Global/Team memory 主提示词注入都存在；Auto memory 已由 `auto_memory_enabled` 门控注入，最近 session-insights 也会按 workspace 回注；抽取策略仍需继续对齐。 |
| `system-prompt.mdx` | [`claude-code-rs/src/engine/system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L361), [`claude-code-rs/src/engine/prompt_sections.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/prompt_sections.rs#L17), [`claude-code-rs/src/engine/lifecycle/submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs#L238) | 已实现 | 静态段、动态段、缓存边界、`CLAUDE.md` 注入、append/override 顺序都已落地。 |
| `token-budget.mdx` | [`cc-utils/src/tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs), [`cc-compact/src/auto_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/auto_compact.rs), [`claude-code-rs/src/query/token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9) | 部分实现 | 预算判断、续跑逻辑、`CLAUDE_CODE_MAX_CONTEXT_TOKENS` 与 `[1m]` 窗口解析存在；仍主要依赖启发式估算，不是 Bun 文档里那种 provider 级精确 token 统计。 |

## 逐文档分析

### `compaction.mdx`

**上游关注点**

- Bun 把压缩拆成多层：局部工具结果裁剪、会话压缩、传统摘要、boundary 标记、PTL 紧急恢复。

**cc-rust 当前实现**

- `run_context_pipeline()` 已实现工具结果预算、snip、microcompact、自动压缩检查四步主流程。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65)
- `should_auto_compact()` 明确跳过 `compact` 和 `session_memory` 来源，避免递归触发。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L99)
- `session_memory_compact_if_needed()` 已提供无 API 的 Session Memory Compact：使用 `<session-insights>` 作为旧历史摘要，保留 10K-40K token 的最近窗口，并在保留 tool_result 时向前纳入对应 tool_use。[`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs)
- `QueryEngineDeps::autocompact()` 在 auto-compact 触发且当前 workspace 有 session-insights 时，优先走 Session Memory Compact，再回退到模型摘要或本地管线。[`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs)
- `build_post_compact_messages()` 会在摘要后重建上下文，并恢复最近文件引用。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L139)
- `create_compact_boundary()` 和 `get_messages_after_compact_boundary()` 已提供 boundary 生成与回溯能力。[`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L184), [`messages.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/messages.rs#L106)
- `handle_prompt_too_long()` 提供 PTL 重试路径；`reactive_compact()` 失败时会回退到终态。[`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202), [`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L154)
- `QueryEngineDeps::autocompact()` 在有 API client 时会额外调用模型生成摘要，再拼出 post-compact 消息。[`deps.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/deps.rs#L258)

**状态判断**

- `部分实现`。
- 原因不是“没有压缩”，而是压缩主链路和 Session Memory Compact 都已经存在；差异点在于 Bun 文档里的 preservedSegment 注解、feature gate 组合、Partial Compact 和某些恢复策略，在当前 Rust 实现里没有看到完整的一一对应。

### `project-memory.mdx`

**上游关注点**

- Bun 的项目记忆同时包含文件级持久化、回忆、注入和自动收集。

**cc-rust 当前实现**

- `cc-session::memdir` 已实现四个 scope：`Global`、`Project`、`Team`、`Auto`，并且给出明确路径映射。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L38), [`paths.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/paths.rs#L96)
- `write_memory()`、`read_memory()`、`delete_memory()`、`list_memories()`、`search_memories()` 都已实现。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L111)
- `build_memory_context_with()` 已能把 project/global/team/auto 记忆组装成 `<memory-context>`，并且 team/auto 有门控条件。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L248)
- `build_system_prompt()` 现在会把 `build_memory_context_with()` 生成的 Project / Global / Team memory 注入到 `# Memory Context` 段落，并在 `auto_memory_enabled` 为 true 时纳入 Auto memory，形成主提示词端到端路径。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs)
- `/memory` 命令已经把四个 scope 暴露到 UI/CLI，并支持查看、写入、删除、搜索和打开目录。[`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L41), [`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L214)
- `build_claude_md_context()` 会把祖先目录里的 `CLAUDE.md` 合并进上下文，且 `build_system_prompt()` 会把这段内容注入到系统提示词里。[`claude_md.rs`](F:/AIclassmanager/cc/rust/crates/cc-config/src/claude_md.rs#L51), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L517)
- 另有独立的 `SessionMemoryService`，会把对话中抽取的简要 insight 持久化到 `~/.cc-rust/session-insights/`，新条目记录 workspace；`submit_message` 和 `--dump-system-prompt` 会把当前 workspace 最近 5 条格式化为 `<session-insights>` 并回注到 `# Memory Context`。[`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs), [`mod.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/mod.rs), [`submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs), [`fast_paths.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/startup/fast_paths.rs)

**状态判断**

- `部分实现`。
- `CLAUDE.md`、Project / Global / Team memory、受 `auto_memory_enabled` 门控的 Auto memory 和当前 workspace 最近 session-insights 都已经接入主提示词；剩余差异是 session-insights 抽取策略仍是简化实现，需要继续确认是否要按时间窗口、标签或更接近 Bun 的语义进行过滤。

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
- `should_auto_compact()` 也采用 80% 阈值，并复用 `cc-utils` 的动态窗口解析，因此 1M 模型不会在 200K 附近误触发压缩。[`auto_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/auto_compact.rs)
- `check_token_budget()` 处理任务预算：低于阈值时发出 nudge 继续，高于阈值时停止，并记录连续继续与递减收益。[`token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9), [`state.rs`](F:/AIclassmanager/cc/rust/crates/cc-types/src/state.rs#L68)
- `query/loop_impl.rs` 在主循环里调用 `check_token_budget()`，并把 `Continue::TokenBudgetContinuation` 注入回消息流。[`loop_impl.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_impl.rs#L622)
- `handle_max_output_tokens()` 提供输出上限的升级与恢复消息，属于另一条和 token budget 相邻但不同的恢复链路。[`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202)
- `build_messages_request()` 会根据模型夹紧 `max_tokens`，但这属于请求侧保护，不是 Bun 文档里的精确 token 统计逻辑。[`helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/helpers.rs#L194)

**状态判断**

- `部分实现`。
- 当前实现已经有预算判断、动态窗口解析和恢复动作，但仍是启发式估算为主，没有看到 Bun 文档里那种 provider 级精确计数入口。

## 已实现汇总

- 系统提示词拼装链路已经落地：静态段、动态段、缓存边界、`CLAUDE.md` 注入、append/override 顺序都能在源码里直接定位。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L361), [`prompt_sections.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/prompt_sections.rs#L51)
- 记忆存储、命令面和主提示词注入已经实现：`cc-session::memdir` 支持四个 scope 的 CRUD 和搜索，`/memory` 也能查看、编辑和打开这些目录；Project / Global / Team memory 会进入 `# Memory Context`，Auto memory 会在 `auto_memory_enabled` 开启时进入同一段落，当前 workspace 最近 session-insights 也会回放到同一段落。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L111), [`commands/memory.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/commands/memory.rs#L41), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs)
- 压缩主链路已经实现：tool result budget、snip、microcompact、Session Memory Compact、自动压缩、boundary、PTL 恢复和 hook 都有对应代码。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs), [`compaction.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/compaction.rs#L184)
- token 预算已经接入主循环：任务预算继续/停止、max_output_tokens 恢复、自动压缩阈值都不是占位。[`token_budget.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/token_budget.rs#L9), [`loop_helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/query/loop_helpers.rs#L202)

## 未实现 / 部分实现 / 待确认 / 故意裁剪

### 部分实现

- `compaction.mdx`：已有完整压缩管线和 Session Memory Compact，但 Bun 文档里的 preservedSegment 注解、feature gate 组合与 Partial Compact 还没有看到同构实现。[`pipeline.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/pipeline.rs#L65), [`session_memory_compact.rs`](F:/AIclassmanager/cc/rust/crates/cc-compact/src/session_memory_compact.rs)
- `project-memory.mdx`：记忆 CRUD、`CLAUDE.md` 注入、Project / Global / Team memory 主提示词注入、`auto_memory_enabled` 门控的 Auto memory 注入和 workspace-scoped session-insights 回注都存在；session-insights 的抽取策略仍需继续对齐。[`memdir.rs`](F:/AIclassmanager/cc/rust/crates/cc-session/src/memdir.rs#L248), [`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs), [`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs)
- `token-budget.mdx`：有预算判断、动态窗口解析和恢复，但主要依赖启发式估算，不是精确 token 统计。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs)

### 待确认

- `build_system_prompt()` 返回的 `user_context` / `system_context` 当前只看到在 `QueryParams` 里流转；`build_messages_request()` 只序列化 `system_prompt`、messages 和 tools。需要确认这是有意保留的元数据，还是后续还要展开成独立输入层。[`system_prompt.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/system_prompt.rs#L552), [`helpers.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/helpers.rs#L100)
- `SessionMemoryService` 当前会把当前 workspace 最近 5 条 session-insights 注入提示词；仍需确认 Bun 语义是否要求按当前 session、时间窗口或标签进一步过滤。[`session_memory.rs`](F:/AIclassmanager/cc/rust/crates/cc-services/src/session_memory.rs), [`submit_message.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/engine/lifecycle/submit_message.rs)

### 未实现

- 在本次 `context` 范围内，没有看到 Bun 文档那种 provider 级精确 token 计数入口或 `countTokens` 同构实现。当前路径仍是动态窗口 + 估算与阈值判断。[`tokens.rs`](F:/AIclassmanager/cc/rust/crates/cc-utils/src/tokens.rs)

### 故意裁剪

- 未发现能从源码直接证明的“故意裁剪”项。当前更像是实现分叉和接入层缺口，而不是显式声明过的缩减。

## 后续动作

1. 如果要继续对齐 Bun 的 `project-memory` 语义，下一步应决定 `SessionMemoryService` 的 insight 是否要按当前 session / 时间窗口 / 标签进一步过滤，并补齐比“截取最近 assistant 文本”更接近 Bun 的抽取策略。
2. 如果要继续对齐 `token-budget` 语义，补齐精确 token 统计的 provider 路径，或者把“仅启发式估算”明确写成故意裁剪。
3. 如果要继续写 `extensibility`、`safety`、`tools` 章节，建议沿用同样的结构：上游文档清单、实现映射表、逐文档分析、汇总、缺口、后续动作。
