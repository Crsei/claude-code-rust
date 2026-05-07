# The Loop 源文档整理

来源：`F:\AIclassmanager\cc\claude-code-bun\docs\conversation\the-loop.mdx`  
源文档标题：`Agentic Loop：AI 自主循环的核心机制`  
源文档 `sourceRef`：`3ec5675 (2026-04-08)`  
整理日期：2026-05-05

本文按 Bun 版文档的叙述方式整理 Claude Code 的 `queryLoop()` agent-loop，并把它映射到当前 `cc-rust` 实现，列出还未实现或尚未完全对齐的地方。

## 1. 源文档的核心定义

源文档把 Agentic Loop 定义为一个持续的“思考、行动、观察”循环。它不是传统聊天机器人“一问一答”的单轮模式，而是围绕 `queryLoop()` 异步生成器反复迭代：

```text
用户目标
  -> 模型思考并输出 assistant message / tool_use
  -> 工具执行
  -> 工具结果回填进消息历史
  -> 模型基于新观察继续推理
  -> 直到没有后续工具调用或触发终止条件
```

源文档强调：每一次迭代都不是静态计划的机械执行，而是基于真实工具结果、最新上下文大小、错误状态、stop hooks 和 token budget 的动态决策。

## 2. 源文档中的四阶段循环

### Phase 1：Pre-Processing Pipeline

模型调用前，源文档定义了 5 个串行上下文处理步骤：

```text
messagesForQuery 原始消息
  -> applyToolResultBudget()
  -> snipCompactIfNeeded()
  -> microcompact()
  -> applyCollapsesIfNeeded()
  -> autocompact()
messagesForQuery 处理后消息
```

各步骤职责：

| 步骤 | 目的 |
| --- | --- |
| `applyToolResultBudget()` | 按 `maxResultSizeChars` 截断或替换超大工具结果。 |
| `snipCompactIfNeeded()` | 在 `HISTORY_SNIP` feature 下裁剪旧历史。 |
| `microcompact()` | 对工具结果做轻量摘要，减少重复 token。 |
| `applyCollapsesIfNeeded()` | 在 `CONTEXT_COLLAPSE` feature 下折叠旧上下文段。 |
| `autocompact()` | 超过阈值时触发自动压缩。 |

关键细节：Snip 和 microcompact 释放的 token 数会传给 autocompact 阈值计算，避免刚释放完空间又重复触发重压缩。

### Phase 2：Streaming Loop

源文档中，模型调用由 `deps.callModel()` 发起，并包在 `attemptWithFallback` 循环里。流式过程中同时做四件事：

| 行为 | 作用 |
| --- | --- |
| 收集 assistant message | 把流式内容归并到 `assistantMessages[]`。 |
| 提取 `tool_use` | 把工具调用块收集到 `toolUseBlocks[]`，并设置 `needsFollowUp = true`。 |
| 启动 `StreamingToolExecutor` | 工具在流式过程中开始执行，不等待完整 assistant stream 结束。 |
| 暂扣可恢复错误 | prompt-too-long、max-output-tokens 等先 withheld，优先尝试恢复。 |

两个源文档中特别重要的 guard：

| Guard | 作用 |
| --- | --- |
| `backfillObservableInput()` | 为 tool_use 回填可观察字段，但只在新增字段时 clone，保护 prompt cache byte identity。 |
| streaming fallback 检测 | fallback 发生时，将已收集 assistant messages 标记 tombstone，清空后重试。 |

### Phase 3：Tool Execution

源文档描述了两种互斥工具执行路径：

```typescript
const toolUpdates = streamingToolExecutor
  ? streamingToolExecutor.getRemainingResults()
  : runTools(toolUseBlocks, assistantMessages, canUseTool, toolUseContext)
```

也就是说：

| 路径 | 触发场景 |
| --- | --- |
| `StreamingToolExecutor.getRemainingResults()` | 流式工具执行已启用，stream 结束时只等待剩余结果。 |
| `runTools(...)` | 没启用流式工具执行时，stream 结束后批量执行工具。 |

工具结果会经过 `normalizeMessagesForAPI()` 标准化，再和原始消息合并，作为下一轮 loop 的输入。

### Phase 4：Terminate Or Continue

每轮迭代结束时，loop 根据 transition 决定 `return` 或 `continue`。

源文档中的主要终止条件：

| 终止原因 | 触发语义 |
| --- | --- |
| `blocking_limit` | token 超过硬限制，且无法通过 autocompact 恢复。 |
| `image_error` | 图片大小 / resize 错误。 |
| `model_error` | 不可恢复模型错误。 |
| `aborted_streaming` | 用户在 streaming 阶段中断。 |
| `prompt_too_long` | 413 且 collapse drain / reactive compact 都失败。 |
| `completed` | 没有 tool_use，正常完成；或错误无法继续。 |
| `stop_hook_prevented` | Stop hook 阻止继续。 |
| `aborted_tools` | 用户在工具执行阶段中断。 |
| `hook_stopped` | 工具执行期间 hook 阻止继续。 |
| `max_turns` | 达到最大轮次。 |

源文档中的主要继续条件：

| 继续原因 | 行为 |
| --- | --- |
| `next_turn` | 有 tool_use，执行工具并追加工具结果后进入下一轮。 |
| `max_output_tokens_escalate` | 首次输出截断时提升 max output tokens 到 64K，静默重试。 |
| `max_output_tokens_recovery` | 提升后仍截断时注入恢复消息，最多重试 3 次。 |
| `collapse_drain_retry` | prompt-too-long 时先提交 pending collapses，释放空间后重试。 |
| `reactive_compact_retry` | collapse drain 不足时触发即时压缩后重试。 |
| `stop_hook_blocking` | Stop hook 注入阻塞消息，强制模型重新思考。 |
| `token_budget_continuation` | 预算阈值触发 nudge，让模型加速收尾。 |

## 3. 源文档中的 State 对象

源文档的 `State` 对象记录每轮 loop 的可恢复状态：

| 字段 | 职责 |
| --- | --- |
| `messages` | 当前对话消息。 |
| `toolUseContext` | 工具上下文和权限状态。 |
| `autoCompactTracking` | 自动压缩跟踪状态。 |
| `maxOutputTokensRecoveryCount` | 输出截断恢复次数。 |
| `hasAttemptedReactiveCompact` | 是否已经尝试 reactive compact。 |
| `maxOutputTokensOverride` | 输出 token 上限覆盖。 |
| `pendingToolUseSummary` | 异步工具摘要。 |
| `stopHookActive` | Stop hook 是否已激活，防止递归。 |
| `turnCount` | 当前轮次。 |
| `transition` | 上一次继续的原因。 |

源文档强调：每次 `continue` 创建新的 State，而不是就地修改；`transition` 让下一轮能识别恢复路径，避免无限循环。

## 4. 映射到 cc-rust 当前实现

当前 Rust 项目有两层 loop：

| 层级 | Rust 入口 | 说明 |
| --- | --- | --- |
| 外层生命周期 | [`submit_message.rs`](../../crates/claude-code-rs/src/engine/lifecycle/submit_message.rs) | 输入处理、系统 prompt、query loop 调用、SDK result 输出。 |
| 内层 agent loop | [`loop_impl.rs`](../../crates/claude-code-rs/src/query/loop_impl.rs) | 每轮 context、streaming、tool execution、terminal / continue 决策。 |

Rust 主链路：

```text
QueryEngine::submit_message()
  -> process_user_input()
  -> build_system_prompt()
  -> query(params, deps)
    -> deps.microcompact()
    -> deps.autocompact()
    -> deps.call_model_streaming()
    -> StreamAccumulator
    -> extract_tool_uses()
    -> execute_tool_calls()
    -> transition
  -> SdkMessage::Result
```

`loop_impl.rs` 已把 agent-loop 注释为八步状态机：

| Rust 步骤 | 对应源文档阶段 | 当前状态 |
| --- | --- | --- |
| Step 1 setup | State 初始化 | 已实现。 |
| Step 1b background agent results | 扩展输入观察 | 已实现项目自有能力。 |
| Step 2 context | Phase 1 | 部分实现。 |
| Step 3 API streaming | Phase 2 | 已实现基础 streaming。 |
| Step 4 post-streaming | Phase 2 / 3 之间 | 已实现 assistant message 累积。 |
| Step 5 terminal check | Phase 4 | 部分实现。 |
| Step 6 tool execution | Phase 3 | 已实现 post-stream 工具执行。 |
| Step 7 attachments | Phase 3 补充输出 | placeholder / 部分实现。 |
| Step 8 continue | Phase 4 | 已实现 next turn 和部分恢复路径。 |

## 5. Rust 已对齐的能力

| 源文档能力 | Rust 当前实现 |
| --- | --- |
| 外层提交生命周期 | `QueryEngine::submit_message()` 已拆成输入、system prompt、query loop、result。 |
| 每轮状态对象 | `QueryLoopState` 已包含 messages、auto compact、max output recovery、stop hook、turn count、transition。 |
| terminal / continue 枚举 | `Terminal` 和 `Continue` 已覆盖大部分参考 transition 名称。 |
| 基础 streaming | `deps.call_model_streaming()` + `StreamAccumulator` 已流式转发并收集 assistant message。 |
| 工具结果进入下一轮 | `execute_tool_calls()` 生成 `tool_result` user message 后继续。 |
| 工具权限和 hooks | `QueryEngineDeps::execute_tool()` 已接入 PreToolUse、PermissionRequest、PostToolUse 等。 |
| max-output-tokens 恢复 | 已有 64K escalate 和最多 3 次 continuation recovery。 |
| reactive compact | prompt-too-long 后可尝试一次 reactive compact。 |
| stop hook blocking | 已有 stop hook blocking continuation。 |
| token budget | 已有 `check_token_budget()`。 |
| tool use summary | 已有 pending summary / summary yield 路径。 |
| tool result budget / snip / microcompact / autocompact | `cc-compact` 管线已有对应基础实现。 |

## 6. 未实现 / 未完全对齐清单

### P0：StreamingToolExecutor 没有接入主 query loop

源文档中，工具可以在模型流式输出 `tool_use` 后立即开始执行。Rust 当前主 loop 是先完整消费 stream，再从完整 assistant message 中提取工具调用，最后执行工具。

当前证据：

| 位置 | 说明 |
| --- | --- |
| [`loop_impl.rs`](../../crates/claude-code-rs/src/query/loop_impl.rs) | 使用 `StreamAccumulator` 收完整 assistant，再调用 `extract_tool_uses()`。 |
| [`coordinator.rs`](../../crates/claude-code-rs/src/tools/execution/coordinator.rs) | `StreamingToolExecutor` 类型存在，但没有成为主 loop 路径。 |

需要补齐：

1. 在 stream event 阶段识别完整 `tool_use`。
2. 将识别到的工具调用交给 `StreamingToolExecutor`。
3. stream 结束后只等待 remaining results。
4. 保证权限、hooks、progress、abort 行为与现有 `execute_tool_calls()` 一致。

### P0：模型 fallback 和 tombstone retry 缺主流程

源文档中的 `attemptWithFallback` 会在 fallback 时清空已收集 assistant messages、合成 tool_result、去除 signature blocks、切换 fallback model，并将旧消息 tombstone。

Rust 当前已有相关类型和参数，但没有完整主流程：

| 位置 | 状态 |
| --- | --- |
| `QueryParams::fallback_model` | 字段存在。 |
| `QueryYield::Tombstone` | 类型存在。 |
| `submit_message.rs` | 收到 tombstone 时只 debug log。 |
| `loop_impl.rs` | 未发现使用 `fallback_model` 的 retry wrapper。 |

需要补齐：

1. 在模型调用外层实现 `attemptWithFallback` 等价逻辑。
2. fallback 发生时 tombstone 已收集 assistant content。
3. 清理跨模型不可回放的 signature / thinking block。
4. 将 fallback 系统消息和 tombstone 事件进入 session / UI / SDK 可观察层。

### P1：Prompt-too-long 缺 collapse drain retry

源文档的 413 恢复顺序是：

```text
collapse_drain_retry
  -> reactive_compact_retry
  -> prompt_too_long terminal
```

Rust 当前 `handle_prompt_too_long()` 只实现 reactive compact，然后失败即 terminal。`Continue::CollapseDrainRetry` 枚举已存在，但未形成真实恢复路径。

需要补齐：

1. 定义 pending collapse 的提交和 drain 语义。
2. 在 reactive compact 前优先执行 collapse drain。
3. 防止连续 collapse drain 造成无限循环。

### P1：Context collapse 仍是 placeholder

源文档中的 `applyCollapsesIfNeeded()` 是 autocompact 前的独立阶段。Rust 的 `cc-compact` 管线已经预留 context collapse 注释，但尚未实现实际折叠逻辑。

需要补齐：

1. 定义可折叠历史段、摘要格式和可回溯引用。
2. 处理 tool_use / tool_result 的配对完整性。
3. 将 collapse 释放 token 计入 autocompact 阈值判断。

### P1：Snip / microcompact freed token 未传给 autocompact

源文档明确提到 snip 和 microcompact 释放的 token 会通过 `snipTokensFreed` 影响 autocompact 阈值，避免重复压缩。

Rust 当前 `run_context_pipeline()` 顺序接近，但 auto compact check 没有完整复刻这类 freed-token 传递语义。

需要补齐：

1. 在 pipeline result 中保留每个阶段释放 token。
2. 将 freed token 传入 auto compact 阈值计算。
3. 为“刚 snip 后不重复 autocompact”补回归测试。

### P1：流式错误 withheld / recovery 不完整

源文档里 prompt-too-long、max-output-tokens 等可恢复错误会先 withheld，优先走 fallback 或恢复路径。Rust 当前能处理模型调用返回的 prompt-too-long 和 assistant `stop_reason == "max_tokens"`，但 stream 中途错误、fallback retry、withheld error release 的统一模型还不完整。

需要补齐：

1. 区分 request 建立失败、stream 中途失败、assistant stop reason。
2. 为 recoverable stream error 建立 withheld error 状态。
3. 在恢复耗尽后再释放错误消息。

### P1：`backfillObservableInput()` 没有等价实现

源文档中这个函数用于给 tool_use input 回填可观察字段，同时避免无意义 clone 破坏 prompt cache byte identity。Rust 当前未发现等价主流程。

需要补齐：

1. 列出哪些工具需要 observable input 回填。
2. 实现“只有新增字段才 clone”的消息更新策略。
3. 增加 prompt cache byte identity 相关测试。

### P1：主 loop 没有统一使用通用工具执行管线

Rust 里存在两个工具执行表面：

| 表面 | 状态 |
| --- | --- |
| [`loop_helpers.rs`](../../crates/claude-code-rs/src/query/loop_helpers.rs) + `QueryEngineDeps::execute_tool()` | 主 loop 当前路径。 |
| [`tools/execution/pipeline.rs`](../../crates/claude-code-rs/src/tools/execution/pipeline.rs) | 有 lookup、validation、permission、execution、post-hooks、result-size 等完整阶段，但不是主 loop canonical path。 |

风险：如果主 loop 不使用统一 pipeline，validation、安全检查、结果大小限制可能分散在工具自身实现里，长期容易漂移。

需要补齐：

1. 明确 canonical tool execution path。
2. 合并或复用 validation / security / result-size 阶段。
3. 为 allow、deny、ask、validation failure、oversized result 建 e2e。

### P2：Attachment stage 仍不完整

`loop_impl.rs` Step 7 标注为 attachments，但目前只处理一部分工具返回消息和 max-turns attachment。`Attachment` 类型中已有 edited text file、queued command、structured output、nested memory、skill discovery 等变体，尚未在主 loop 中形成完整汇聚阶段。

需要补齐：

1. 统一 attachment 的来源和生命周期。
2. 明确哪些 attachment 进入模型上下文，哪些只进入 UI / SDK。
3. 补 IPC / SDK mapper 的端到端验证。

### P2：Slash command 预处理仍未完整执行

`process_user_input()` 能识别 slash command，但当前部分已知命令路径会直接返回文本结果并 `should_query = false`，完整异步 command execution 仍未接入外层 submit loop。

需要补齐：

1. 接入 command dispatcher 的真实执行结果。
2. 区分直接返回、生成 user message 继续 query、生成 attachment 三种输出。
3. 覆盖 known / unknown / async command 三类测试。

### P2：Query gates 与行为未完全贯通

`QueryGates` 已有 `streaming_tool_execution`、`emit_tool_use_summaries`、`fast_mode_enabled`，但主 loop 尚未完整用这些 gate 控制行为，尤其是 streaming tool execution。

需要补齐：

1. 为每个 gate 明确默认行为。
2. 在主 loop 关键分支读 gate。
3. 将 gate 状态写入测试 fixture，防止配置漂移。

## 7. 建议补齐顺序

1. 先统一工具执行 canonical path，避免后续 StreamingToolExecutor 接入时复制权限、validation、hook 逻辑。
2. 实现 `attemptWithFallback`、tombstone 和 signature block 清理，因为这会影响 session、UI 和 provider 兼容性。
3. 接入 StreamingToolExecutor，把工具执行从 post-stream 后移到 stream-time 调度。
4. 补 context collapse 和 collapse drain retry，完成 prompt-too-long 恢复链。
5. 补 withheld stream error、backfill observable input、prompt cache byte identity 测试。
6. 收敛 attachment stage、slash command、query gates，并补 SDK / IPC e2e。

## 8. 与 `architecture/agent-loop.md` 的关系

[`../agent-loop.md`](../agent-loop.md) 是从线上参考页出发，对当前 Rust agent-loop 的总览和缺口整理。本文则固定使用本地 Bun 源文档 `the-loop.mdx` 作为输入，保留源文档章节结构，并把每一块映射到 Rust 当前实现。两者结论一致，本文更适合作为后续按 Bun 文档追 parity 的任务入口。

