# cc-rust Agent Loop

本文按 CCB 文档 [Agentic Loop：AI 自主循环的核心机制](https://ccb.agent-aura.top/docs/conversation/the-loop) 的拆分方式，整理 `cc-rust` 当前的 agent-loop，并标出与参考实现尚未对齐的部分。

记录日期：2026-05-05。

## 参考模型

参考文档把 Claude Code 的核心 loop 归纳为四段：

| 阶段 | 参考职责 |
| --- | --- |
| Phase 1：上下文预处理 | 对消息做工具结果预算截断、历史 snip、microcompact、context collapse、autocompact，然后送入模型。 |
| Phase 2：流式模型调用 | 通过 `deps.callModel()` 流式读取 assistant message，收集 `tool_use`，并在流式过程中启动工具执行。 |
| Phase 3：工具执行 | 对工具结果标准化、追加进消息历史，再进入下一轮。 |
| Phase 4：终止或继续 | 根据 stop reason、工具调用、token budget、stop hooks、恢复路径等决定结束或重试 / 续跑。 |

参考文档强调的关键机制包括：`StreamingToolExecutor` 流式工具执行、`attemptWithFallback` 模型降级、tombstone 标记、prompt-too-long / max-output-tokens 恢复、collapse drain retry、reactive compact retry、stop hook blocking、token budget continuation，以及一个跨轮次维护的 `State` 对象。

## 本项目总览

当前 Rust 端的 agent-loop 分成两层：

1. 外层生命周期：`QueryEngine::submit_message()`。
2. 内层查询循环：`query::loop_impl::query()`。

主要代码位置：

| 模块 | 职责 |
| --- | --- |
| `crates/claude-code-rs/src/engine/lifecycle/submit_message.rs` | 外层提交入口，串起输入处理、系统提示词、query loop、SDK 结果输出。 |
| `crates/claude-code-rs/src/engine/lifecycle/input_processing.rs` | 用户输入和 slash command 预处理。 |
| `crates/claude-code-rs/src/engine/lifecycle/system_prompt.rs` | 构建系统提示词、环境、工具、CLAUDE.md 等动态上下文。 |
| `crates/claude-code-rs/src/query/loop_impl.rs` | 内层 agent loop 主状态机。 |
| `crates/claude-code-rs/src/query/loop_helpers.rs` | prompt-too-long、max-output-tokens、工具执行等辅助逻辑。 |
| `crates/claude-code-rs/src/engine/lifecycle/deps.rs` | `QueryDeps` 实现：模型调用、压缩、工具执行、hooks、权限。 |
| `crates/cc-compact/src/pipeline.rs` | 上下文压缩管线。 |
| `crates/cc-types/src/state.rs` | `QueryLoopState`、`AutoCompactTracking` 等跨轮状态。 |
| `crates/cc-types/src/transitions.rs` | terminal / continue transition 枚举。 |

整体链路：

```text
UI / IPC / caller
  -> QueryEngine::submit_message()
    -> process_user_input()
    -> build_system_prompt()
    -> loop_impl::query(params, deps)
      -> context pipeline / compact
      -> call_model_streaming()
      -> collect assistant + tool_use
      -> execute tools
      -> transition: terminal or continue
    -> SdkMessage::Result
```

## 外层 Submit Loop

`submit_message.rs` 的文件注释把外层生命周期写成 A 到 E 五段：

| 阶段 | 当前实现 |
| --- | --- |
| Phase A：Input Processing | 处理用户输入、attachments、slash command 早返回。 |
| Phase B：System Prompt Build | 构建系统 prompt，合并动态上下文和环境信息。 |
| Phase C：Pre-Query Setup | 组装 `QueryParams`、检查 API provider、准备 `QueryEngineDeps`。 |
| Phase D：Query Loop | 调用 `loop_impl::query(params, deps)` 并转发 streaming / tool / attachment 事件。 |
| Phase E：Result Generation | 生成最终 `SdkMessage::Result`，包含 cost、usage、duration、turn count 等。 |

这层更接近 SDK / session lifecycle，不直接决定 agent 的每轮推理策略；真正的 agent-loop 在内层 `query()`。

## 内层 Query Loop

`loop_impl.rs` 的头部注释把当前 query loop 对齐为八步状态机：

| 步骤 | 当前实现 |
| --- | --- |
| Step 1：Setup | 初始化 `QueryLoopState`、消息历史、可用工具、系统 prompt、token budget。 |
| Step 1b：Background agent results | 拉取后台 agent 结果，并在有更新时注入 user message。 |
| Step 2：Context preparation | 先执行 `deps.microcompact()`，再执行 `deps.autocompact()`。 |
| Step 3：API streaming | 调用 `deps.call_model_streaming()`，转发 streaming delta，并用 `StreamAccumulator` 收集完整 assistant message。 |
| Step 4：Post-streaming | 把 assistant message 追加到状态消息历史。 |
| Step 5：Terminal check | 如果没有 tool call，处理 max-output-tokens、stop hooks、token budget、terminal completed 等。 |
| Step 6：Tool execution | 如果存在 tool call，执行工具，生成 `tool_result` user message，并可输出 tool use summary。 |
| Step 7：Attachments | 当前是 placeholder，只处理部分工具返回的 `new_messages` 和 max-turns attachment。 |
| Step 8：Continue | 刷新工具列表，设置 `Continue::NextTurn`，进入下一轮。 |

这说明本项目已经有清晰的 loop 骨架，并且很多 continuation 枚举已存在，但若按参考文档的精细行为对齐，部分路径仍是占位或未接入。

## State 与 Transition

`crates/cc-types/src/state.rs` 中的 `QueryLoopState` 已覆盖主要跨轮状态：

| 字段 | 用途 |
| --- | --- |
| `messages` | 当前 loop 累积的消息历史。 |
| `auto_compact_tracking` | 自动压缩状态、turn counter、连续失败次数。 |
| `max_output_tokens_recovery_count` | max-output-tokens 恢复次数。 |
| `has_attempted_reactive_compact` | prompt-too-long 后是否已尝试 reactive compact。 |
| `max_output_tokens_override` | 动态提升 max output tokens。 |
| `pending_tool_use_summary` | 延迟输出的工具使用摘要。 |
| `stop_hook_active` | 防止 stop hook 递归触发。 |
| `turn_count` | 当前 loop 轮数。 |
| `transition` | 最近一次 terminal / continue 决策。 |

`crates/cc-types/src/transitions.rs` 已定义多种终止和继续路径：

| 类型 | 已定义路径 |
| --- | --- |
| Terminal | `Completed`、`AbortedStreaming`、`AbortedTools`、`PromptTooLong`、`ModelError`、`HookStopped`、`StopHookPrevented`、`MaxTurns` 等。 |
| Continue | `NextTurn`、`CollapseDrainRetry`、`ReactiveCompactRetry`、`MaxOutputTokensEscalate`、`MaxOutputTokensRecovery`、`StopHookBlocking`、`TokenBudgetContinuation`。 |

这些类型与参考文档的 State / transition 设计基本同构，但并不是所有枚举都已经有完整行为。

## Context Pipeline

本项目的上下文处理分两处：

1. `loop_impl.rs` 每轮先调用 `deps.microcompact()`。
2. 随后 `deps.autocompact()` 调用 `cc-compact::pipeline::run_context_pipeline()`。

`cc-compact` 当前管线顺序为：

```text
apply_tool_result_budget
  -> snip_compact
  -> microcompact
  -> context collapse placeholder
  -> auto compact check
```

已经实现的能力：

| 能力 | 当前状态 |
| --- | --- |
| 工具结果预算 | 已有 `apply_tool_result_budget()`，可将超大工具结果持久化并替换为引用。 |
| Snip | 已有 `snip_compact()`，按最大 turn 数裁剪旧历史。 |
| Microcompact | 已有 `microcompact_messages()`，用于压缩工具结果摘要。 |
| Autocompact | 已有阈值判断和模型摘要 / 本地摘要 fallback。 |
| Reactive compact | 已有 `try_reactive_compact()`，用于 prompt-too-long 后更激进压缩。 |

仍未对齐的点见下方“未实现 / 未对齐清单”。

## Streaming 与 Tool Execution

当前主 loop 的模型调用过程是：

1. `deps.call_model_streaming()` 返回 provider stream。
2. `StreamAccumulator` 消费 stream，期间向外转发 streaming 事件。
3. stream 结束后，从完整 assistant message 中提取 `tool_use`。
4. 如果存在工具调用，再调用 `execute_tool_calls()`。

工具执行当前有两套相关代码：

| 路径 | 状态 |
| --- | --- |
| 主 loop 路径：`loop_helpers::execute_tool_calls()` -> `QueryEngineDeps::execute_tool()` | 当前实际使用。支持连续 concurrency-safe 工具批量并发、serial 工具顺序执行、hooks、权限和交互式授权。 |
| 通用执行管线：`tools/execution/pipeline.rs::run_tool_use()` | 有 lookup、validation、permission、execution、post-hooks、result-size 等阶段，但不是主 query loop 的 canonical 路径。 |
| `tools/execution/coordinator.rs::StreamingToolExecutor` | 类型已存在，但主 loop 未接入；其内部 batch 执行注释也标明目前仍是 sequential。 |

这与参考文档最大的差异是：参考实现在流式读取 `tool_use` 时就启动工具执行，本项目当前是在 assistant stream 完成后再提取和执行工具。

## Termination 与 Continuation

当前已实现或部分实现的继续 / 结束路径：

| 路径 | 当前状态 |
| --- | --- |
| 普通完成 | 无 tool call 且无续跑条件时 `Terminal::Completed`。 |
| max turns | 达到 `max_turns` 后 yield `MaxTurnsReached` attachment 并终止。 |
| max-output-tokens escalate | 首次遇到 `stop_reason == "max_tokens"` 时提升到 64k。 |
| max-output-tokens recovery | 后续注入 continuation user message，最多 3 次。 |
| prompt-too-long reactive compact | 模型调用返回 prompt-too-long 时尝试一次 reactive compact。 |
| stop hooks | 已有 stop hook 检查、blocking continuation 和 prevent stop 路径。 |
| token budget | 已有 `check_token_budget()`，可触发 continuation 或 blocking limit。 |
| tool use summary | gate 配置存在，loop 当前可输出工具摘要事件。 |

## 与参考文档对比

| 参考能力 | cc-rust 当前状态 | 证据 | 差距 |
| --- | --- | --- | --- |
| 四阶段 agent loop | 基本具备 | `submit_message.rs` + `loop_impl.rs` | 外层 / 内层拆分清晰，但部分参考细节未接入。 |
| 预处理管线顺序 | 部分具备 | `cc-compact/src/pipeline.rs` | 有 budget、snip、microcompact、autocompact；context collapse 仍是 placeholder。 |
| Snip / microcompact freed token 传递给 autocompact | 未完整对齐 | `run_context_pipeline()` | 当前 auto compact 阈值判断没有参考实现中的 `snipTokensFreed` 传递语义。 |
| Streaming API call | 已实现 | `deps.call_model_streaming()` + `StreamAccumulator` | 能流式输出，但 tool_use 只在 stream 完整结束后处理。 |
| StreamingToolExecutor 流式工具执行 | 未接入主 loop | `coordinator.rs` 存在，`loop_impl.rs` 未使用 | 需要在 streaming delta 中识别完整 tool_use 并提前启动工具。 |
| Tool result 归一化并进入下一轮 | 已实现 | `execute_tool_calls()` 生成 `tool_result` user message | 需要继续补齐 attachment / observable input 等细节。 |
| attemptWithFallback / model fallback | 未实现主 loop 行为 | `QueryParams.fallback_model` 存在但 `loop_impl.rs` 未使用 | 需要 API call wrapper、fallback retry、tombstone 标记和重放策略。 |
| Tombstone retry | 类型存在但行为很弱 | `QueryYield::Tombstone` 存在；`submit_message.rs` 只 debug log | 需要在 fallback / streaming fallback 时标记旧 assistant messages 并重试。 |
| prompt-too-long recovery | 部分实现 | `handle_prompt_too_long()` | 只有 reactive compact；缺 collapse drain retry。 |
| max-output-tokens recovery | 已较完整 | `handle_max_output_tokens()` | 仍需确认与参考文档的 recovery 文案和 token escalation 完全一致。 |
| Stop hook blocking | 已实现 | `stop_hooks` + `Continue::StopHookBlocking` | 需继续用 e2e 固化边界行为。 |
| Token budget continuation | 已实现 | `token_budget.rs` | 需补测试覆盖不同预算边界。 |
| Attachment stage | 部分 / placeholder | `loop_impl.rs` Step 7 | 尚未成为完整的 edited-file、queued-command、memory、skill discovery 汇聚阶段。 |
| Backfill observable input | 未发现等价实现 | 未见 `backfillObservableInput` 等价路径 | 需要为部分工具的可观察输入字段补回填，并保护 prompt cache byte identity。 |
| 通用工具执行管线 | 未成为主路径 | `run_tool_use()` 与 `QueryEngineDeps::execute_tool()` 并存 | 主 loop 可能绕过通用 validation / security / result-size 阶段，除非单个工具自行处理。 |

## 未实现 / 未对齐清单

### P0：StreamingToolExecutor 未接入主 loop

参考文档的核心性能点是边收模型流边启动工具。本项目虽然有 `StreamingToolExecutor` 类型，但 `loop_impl.rs` 当前通过 `StreamAccumulator` 收完整 assistant message 后才提取 `tool_use`。这会造成工具启动延迟，也无法完全复刻参考实现的 streaming fallback / partial tool-use 处理。

建议：

1. 在 stream event 消费阶段识别完整 `tool_use` block。
2. 接入 `StreamingToolExecutor`，把权限检查、hook、progress 事件与现有 `QueryEngineDeps::execute_tool()` 对齐。
3. stream 结束后只等待剩余工具结果，而不是重新从 assistant message 批量启动。

### P0：模型 fallback、tombstone retry 未实现

`QueryParams` 已有 `fallback_model`，`QueryYield` 已有 `Tombstone`，但主 query loop 没有 `attemptWithFallback` 等价逻辑。`submit_message.rs` 收到 tombstone 也只是 debug log。

建议：

1. 给 `call_model_streaming()` 外层增加 fallback attempt wrapper。
2. streaming fallback 发生时，将已收集 assistant message 标记 tombstone 并清空后重试。
3. 将 tombstone 事件传到 SDK / IPC 可观察层，避免 UI 和 session 误认为旧 assistant 内容有效。

### P1：prompt-too-long 缺 collapse drain retry

`Continue::CollapseDrainRetry` 已定义，但 `handle_prompt_too_long()` 注释中的三段恢复实际上只执行 reactive compact，然后失败即 terminal。参考文档中的 collapse drain retry 尚未落地。

建议：

1. 明确 collapse drain 的 committed-message 策略。
2. 在 reactive compact 前先尝试 drain / collapse retry。
3. 为 prompt-too-long 构造单元测试和集成测试，覆盖 drain 成功、reactive 成功、全部失败三类路径。

### P1：Context collapse 仍是 placeholder

`cc-compact` 管线已经预留 context collapse 步骤，但注释明确写着尚未实现。参考文档中 `applyCollapsesIfNeeded()` 是 autocompact 前的独立阶段。

建议：

1. 定义可折叠段落边界和摘要消息格式。
2. 保留可恢复的原始消息引用，避免压缩破坏 tool result 对应关系。
3. 将 collapse freed token 计入后续 autocompact 决策。

### P1：流式错误恢复不完整

当前模型调用入口能在 `call_model_streaming()` 返回 prompt-too-long 错误时走恢复；但参考文档还包含 streaming 过程中 withheld recoverable errors、fallback retry、max-output-tokens recovery 等更细的错误处理。

建议：

1. 区分模型调用建立失败和 stream 中途失败。
2. 对 recoverable streaming error 先保留，不立即向用户暴露。
3. 与 fallback / tombstone 机制合并实现。

### P1：Backfill observable input 缺失

参考文档有 `backfillObservableInput()`：只在新增可观察字段时 clone，以保护 prompt cache 的 byte identity。本项目未发现等价实现。

建议：

1. 梳理哪些工具的 `tool_use` input 需要后补可观察字段。
2. 实现“只有新增字段才 clone”的消息更新策略。
3. 加 prompt cache 相关回归测试，避免无意义重序列化。

### P1：主 loop 未使用通用工具执行管线

`tools/execution/pipeline.rs::run_tool_use()` 包含 lookup、validation、permission、execution、post-hooks、result-size 等阶段；主 loop 当前走 `QueryEngineDeps::execute_tool()`。后者已有 hooks 和权限，但通用 validation / security / result-size 阶段不是统一入口。

建议：

1. 决定 canonical 工具执行入口：要么主 loop 迁移到 `run_tool_use()`，要么把缺失阶段合入 `QueryEngineDeps::execute_tool()`。
2. 确保所有工具输入校验、权限、结果大小限制只维护一套规则。
3. 用工具执行 e2e 覆盖 allow / deny / ask / validation failure / oversized result。

### P2：Attachment stage 仍不完整

`loop_impl.rs` Step 7 标记为 placeholder。当前能处理工具返回的 `new_messages` 和 max-turns attachment，但 `Attachment` 类型中已有 edited file、queued command、structured output、nested memory、skill discovery 等变体，尚未形成完整汇聚阶段。

建议：

1. 统一工具、命令、memory、skill discovery 的 attachment 产出路径。
2. 明确哪些 attachment 进入模型上下文，哪些只进入 SDK / UI 可观察层。
3. 为 IPC / SDK mapper 加端到端测试。

### P2：Slash command 预处理仍是局部 stub

`process_user_input()` 已能识别 slash command，但当前已知命令路径会返回 `/{cmd} {args}` 文本并 `should_query = false`，注释也说明完整异步命令执行后续再接入。

建议：

1. 将 command dispatcher 的真实执行结果接入 `submit_message()`。
2. 区分“命令直接返回结果”和“命令生成消息继续 query”两类。
3. 覆盖 unknown command、known local command、需要模型继续处理的 command。

### P2：配置 gate 与 loop 行为未完全贯通

`QueryGates` 中已有 `streaming_tool_execution`、`emit_tool_use_summaries`、`fast_mode_enabled` 等字段，但主 loop 对 streaming tool execution 等 gate 的使用还不完整。

建议：

1. 明确每个 gate 的默认值和行为边界。
2. 在 loop 关键分支使用 gate 控制行为。
3. 把 gate 状态写入测试 fixture，避免未来默认值改变导致行为漂移。

## 已实现但需要补测试固化的行为

| 行为 | 风险 |
| --- | --- |
| max-output-tokens escalate / recovery | 逻辑已有，仍需与 provider stop reason、usage 统计做集成验证。 |
| stop hook blocking / prevent stop | 分支复杂，建议用 hook fixture 覆盖递归保护和 blocking continuation。 |
| token budget continuation | 预算边界容易漂移，需要覆盖 90%、95%、diminishing returns 等阈值。 |
| background agent result injection | 会改变消息历史，需要覆盖多 agent result 合并与重复注入。 |
| tool use summary | 与工具结果、streaming UI、SDK 输出相关，建议覆盖开启 / 关闭 gate。 |

## 建议补齐顺序

1. 先确定主工具执行入口，避免 StreamingToolExecutor 接入后继续复制权限和 validation 逻辑。
2. 接入 model fallback + tombstone，因为这会影响 streaming、session 和 UI 可观察语义。
3. 接入 StreamingToolExecutor，让工具执行从 post-stream 批处理变成 stream-time 调度。
4. 补 context collapse + collapse drain retry，完成 prompt-too-long 的完整恢复链。
5. 补 backfill observable input 和 prompt-cache byte identity 测试。
6. 收敛 attachment stage、slash command、query gates，并补 IPC / SDK e2e。

