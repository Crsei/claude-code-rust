# Agent Loop / Streaming 补齐路线图

记录日期：2026-05-05

来源：

- `architecture/agent-loop.md`
- `architecture/streaming.md`
- `architecture/the-loop.md`

目标：把当前 `cc-rust` 的 agent loop、streaming、工具执行和恢复路径补齐到 Bun / TypeScript 参考实现的完整行为。本文只整理待办任务和执行顺序，不描述已经完整对齐的基础能力。

## 排序原则

补齐顺序按依赖关系而不是按文档出现顺序排列：

1. 先补会破坏语义正确性的底层协议问题。
2. 再统一工具执行入口，避免后续接入 streaming tool execution 时复制权限、hook、validation、progress、abort 逻辑。
3. 在工具仍是 post-stream 执行时先补模型调用恢复，因为此时 fallback / tombstone 的状态空间更小。
4. 等 stream event、工具执行入口、fallback 语义稳定后，再接入 `StreamingToolExecutor`。
5. context collapse、attachment、slash command、provider 扩展放在主 loop 语义稳定后补齐。
6. 每个阶段先补测试或 fixture，再改行为。

## 总体补齐顺序

| 阶段 | 主题 | 优先级 | 依赖 | 完成后解锁 |
| --- | --- | --- | --- | --- |
| 0 | 基线测试与任务边界 | P0 | 无 | 后续行为改动有回归保护。 |
| 1 | Streaming content-block 协议正确性 | P0 | 阶段 0 | tool_use 参数完整、thinking 签名完整、后续 stream-time 工具识别可用。 |
| 2 | 工具执行 canonical path | P0 | 阶段 0 | 权限、hooks、validation、result-size、progress 只有一套规则。 |
| 3 | 模型调用恢复契约 | P0/P1 | 阶段 1 | fallback、tombstone、retry、stream error、idle/stall 有统一状态语义。 |
| 4 | StreamingToolExecutor 接入主 loop | P0 | 阶段 1、2、3 | 工具可在模型流式输出期间启动执行。 |
| 5 | Context pipeline 与 prompt-too-long 完整恢复 | P1 | 阶段 3 | collapse drain + reactive compact + terminal 的恢复链完整。 |
| 6 | Tool input observability 与 prompt-cache identity | P1 | 阶段 1、2、4 | 可观察 input 回填不破坏 prompt cache byte identity。 |
| 7 | Loop 表面能力收敛 | P2 | 阶段 2、4、5 | attachment、slash command、query gates、SDK/IPC/TUI 行为一致。 |
| 8 | Provider parity 扩展 | P2 | 阶段 1、3 | Bedrock、Google、Vertex、Azure 等 provider 差异可逐步收敛。 |
| 9 | 文档与旧入口收敛 | P2 | 所有阶段 | `architecture/` 与 `docs/architecture/` 不再给出冲突结论。 |

## 执行进度

| 任务 | 状态 | 完成日期 | 证据 | 备注 |
| --- | --- | --- | --- | --- |
| 0.1 streaming fixture | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs api::streaming::tests`，3 passed。 | 在 `api/streaming.rs` 内新增 mixed Anthropic stream fixture，覆盖 `message_start`、text、thinking、tool_use、`message_delta`、`message_stop`。 |
| 0.2 post-stream tool fixture | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs test_tool_use_then_text_response`，1 passed。 | `query::loop_tests::MockDeps` 现在记录 `message_stop` 是否已被消费，并断言当前工具执行不会早于模型流结束启动；后续接入 stream-time tool execution 时需同步调整该期望。 |
| 0.3 recovery fixture | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs query::loop_impl::loop_tests`，10 passed。 | 新增 prompt-too-long reactive compact retry、`max_tokens` 输出上限升级、Stop hook 续写、token budget nudge 四条 query-loop fixture；collapse drain retry 已在 5.4 补齐。 |
| 0.4 ownership 梳理 | 已完成 | 2026-05-05 | 文档：本页“阶段 1-4 ownership”。 | 明确主 loop、工具执行入口、stream accumulator、mapper/provider 的写入边界；阶段 2/4 的核心实现必须 leader 串行集成，subagent 只做只读调研或独立测试。 |
| 1.1 `input_json_delta` | 已完成 | 2026-05-05 | `api::streaming::tests::accumulates_tool_input_json_delta` 通过。 | `StreamAccumulator` 现在累积 `partial_json`，在 `content_block_stop` 和最终 `build()` 时解析为 `ToolUse.input`。 |
| 1.2 `signature_delta` | 已完成 | 2026-05-05 | `api::streaming::tests::accumulates_thinking_signature_delta` 通过。 | `StreamAccumulator` 现在将 `signature_delta.signature` 追加到 `ContentBlock::Thinking.signature`。 |
| 1.3 unsupported delta 策略 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs api::streaming::tests`，5 passed；`cargo test -p claude-code-rs unsupported_text_like_delta`，3 passed。 | Accumulator、headless、TUI、agent event forwarding 都按 delta `type` 处理 text/thinking/input；未知或暂不支持的 text-like delta 不再被误当作 assistant text，同时保留无 `type` legacy delta 兼容。 |
| 1.4 TUI/headless 映射 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs headless_stream_event_mapping`；`cargo test -p claude-code-rs tui_ignores_tool_input_delta_until_final_assistant`。 | Headless 只把 text/thinking delta 映射为可见流；TUI 忽略 tool input delta，并等待最终 assistant 替换为完整 tool_use。 |
| 1.5 per-block assistant 语义 | 已完成 | 2026-05-05 | 文档决策：`architecture/streaming.md` 已标注 Intentional / 暂不改。 | 现阶段保留“stream event 实时输出 + 最终单 `AssistantMessage`”；per-block assistant 需等 `StreamingToolExecutor`、SDK/session、fallback tombstone 语义一起设计。 |
| 2.1 canonical path 决策 | 已完成 | 2026-05-05 | 文档：本页“2.1 canonical path 决策”；代码注释：`query/deps.rs`、`loop_helpers.rs`、`tools/execution/pipeline.rs`、`tools/execution/coordinator.rs`。 | 主 loop canonical 工具执行边界确定为 `QueryDeps::execute_tool()` / `QueryEngineDeps::execute_tool()`；`run_tool_use()` 暂作为参考/待折叠管线，缺失的 validation、security、result-size 阶段在 2.2 合入 canonical 边界。 |
| 2.2 行为合并 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs engine::lifecycle::deps`，8 passed。 | `QueryEngineDeps::execute_tool()` 现在复用 `tools::execution` 的 lookup alias、validation、`_simulatedSedEdit` sanitization、security validation、result-size enforcement，同时保留既有 allow / deny / ask、hook、progress id、abort、audit / Langfuse 和结构化 `ToolResult` 保真。 |
| 2.3 concurrency-safe 语义 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs execute_tool_calls_batches_consecutive_safe_tools_only`，1 passed。 | `loop_helpers::execute_tool_calls()` 已有连续 safe 批量并发、unsafe 串行屏障、结果顺序稳定的回归测试；阶段 4 接入 stream-time scheduler 时必须保留该语义。 |
| 2.4 tool result 标准化 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs tool_result_user_message`，3 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，10 passed。 | 新增 `make_tool_result_user_message()` 作为 `ToolExecResult` 进入下一轮 `tool_result` user message 的统一生成点，保留 error、`model_content`、`display_preview` 和 `new_messages` 语义；`loop_tests` 改为显式导入消息类型，不再依赖 `loop_impl` 私有 import。 |
| 3.1 fallback wrapper | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs test_fallback_model_retries_stream_start_capacity_error`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，11 passed。 | `QueryParams::fallback_model` 现在会在 stream 建立前的 529 / overloaded / high-demand / capacity 失败上触发一次 fallback retry；`prompt-too-long` 不走 fallback wrapper，而是由 5.4 的 collapse drain / reactive compact 恢复链处理。 |
| 3.2 tombstone 生命周期 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs tombstone`，2 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，12 passed。 | stream 中途 capacity 失败触发 fallback 时，主 loop 会 tombstone 已累积 partial assistant 并重试；`SdkMessage::Tombstone` 现在透传到 lifecycle、TUI、headless IPC、daemon/Web SSE，TUI 会移除 partial streaming assistant。 |
| 3.3 signature 清理 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs fallback_signature_stripping`，1 passed；`cargo test -p claude-code-rs fallback_strips_signature`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，13 passed。 | fallback retry 前会从 request history 副本中移除 `Thinking` / `RedactedThinking` blocks，保留 text / tool_use 等可回放上下文；不修改 `state.messages`，避免旧模型签名进入 fallback 请求。 |
| 3.4 retry/backoff | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs stream_start_error`，4 passed；`cargo test -p claude-code-rs messages_stream_`，2 passed。 | `ApiClient::messages_stream()` 现在按 `ApiClientConfig.max_retries` 对 stream 建立前的 429、5xx、529/overloaded/high-demand/capacity 和网络发送错误执行指数退避重试；prompt-too-long、auth、invalid request、未知不可恢复错误立即返回给 query 恢复/terminal 路径。 |
| 3.5 idle/stall watchdog | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs stream_idle_watchdog`，1 passed；`cargo test -p claude-code-rs stream_stall_detection`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，15 passed。 | query stream 消费层现在对 `event_stream.next()` 使用主动 idle timeout，并对 MessageStart 后长期没有内容进展的 passive stall 产出 API error；生产默认 idle 120s / stall 60s，可用 `CC_RUST_STREAM_IDLE_TIMEOUT_MS`、`CC_RUST_STREAM_STALL_TIMEOUT_MS` 调整。 |
| 3.6 failure 分类 / withheld | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs model_call_failure_classifier`，1 passed；`cargo test -p claude-code-rs fallback_exhaustion`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，16 passed。 | 新增 request-start / stream-interrupted failure stage 分类：request-start 可走 prompt-too-long 或 fallback，stream-interrupted 只走 fallback/tombstone 或 terminal；可恢复 primary 错误在 fallback 成功时 withheld，fallback 耗尽后释放最终 API error；assistant `stop_reason` 仍在正常 assistant 分支处理。 |
| 4.1 stream-time `tool_use` block 识别 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs exposes_completed_tool_use_after_block_stop`，1 passed；`cargo test -p claude-code-rs api::streaming::tests`，6 passed。 | `StreamAccumulator::completed_tool_use()` 只在 `content_block_stop` 后返回完整 id/name/input；主 loop 在 stream event 消费阶段识别该边界但暂不启动工具，保留旧 post-stream 执行语义。 |
| 4.2 提交完整 `tool_use` 给 stream-time executor | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs streaming_tool_execution_gate_starts_safe_tools_before_message_stop`，1 passed。 | gate on 时，完整 safe `tool_use` 在 `content_block_stop` 后立即进入 `StreamingToolExecutor` 并可早于 `message_stop` 启动。 |
| 4.3 canonical execution path 复用 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs streaming_tool_execution_gate_starts_safe_tools_before_message_stop`，1 passed。 | stream-time executor 通过 `QueryDeps::execute_tool()` / `ToolExecRequest` 调度，不直接调用 `run_tool_use()`，权限、hooks、progress、abort、result-size 继续共用阶段 2 边界。 |
| 4.4 remaining results 等待 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs streaming_tool_execution_gate_starts_safe_tools_before_message_stop`，1 passed。 | stream 结束后先 await 已启动 safe tools，再只对未启动工具调用 post-stream 批处理，并按原始 `tool_use` 顺序合并结果，避免重复执行。 |
| 4.5 `QueryGates.streaming_tool_execution` | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs test_tool_use_then_text_response`，1 passed；`cargo test -p claude-code-rs streaming_tool_execution_gate_starts_safe_tools_before_message_stop`，1 passed。 | `QueryParams.gates` 默认关闭新行为，可用 `CC_RUST_STREAMING_TOOL_EXECUTION=1` 打开；gate off 保留旧 post-stream 语义，gate on 启用 stream-time safe tool 调度。 |
| 4.6 fallback 与已启动工具隔离 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs streaming_tool_execution_aborts_started_tools_on_stream_fallback`，1 passed。 | stream 中途 fallback/tombstone 会 abort 当前 attempt 已启动的 stream-time tool task；旧 assistant 的工具结果不会写入 fallback transcript。 |
| 5.1 context collapse | 已完成 | 2026-05-05 | `cargo test -p cc-compact context_collapse`，3 passed；`cargo test -p cc-compact pipeline`，4 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，18 passed。 | `cc-compact` 新增 context collapse 阶段：超过 turn/token 阈值时把旧上下文折叠为 `CompactBoundary` system message，并在 autocompact 前运行。 |
| 5.2 tool pair 完整性 | 已完成 | 2026-05-05 | `context_collapse::tests::context_collapse_preserves_tool_use_result_pairs_in_tail` 通过。 | collapse 以 user turn 边界切分并保留最近 turns，避免在保留尾部留下孤立 `tool_result` 或丢失对应 `tool_use`。 |
| 5.3 freed-token accounting | 已完成 | 2026-05-05 | `cargo test -p cc-compact pipeline`，5 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，18 passed。 | `PipelineResult` 现在暴露 snip / microcompact / context collapse 分段释放 token、总释放 token 和 autocompact adjusted token；autocompact 阈值判断会先扣除本轮 local compaction 已释放的 token。 |
| 5.4 collapse drain retry | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs prompt_too_long`，3 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，20 passed。 | prompt-too-long 现在先尝试 `QueryDeps::collapse_drain()`，成功则以 `Continue::CollapseDrainRetry` 重试；无可折叠内容或失败时再尝试 reactive compact，两者都失败后 terminal。 |
| 5.5 max_tokens/context 交互 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs prompt_too_long`，3 passed；`cargo test -p claude-code-rs test_max_tokens_recovery_escalates_next_request_limit`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，20 passed。 | prompt-too-long 的 collapse/reactive retry 不会设置 max-output-token override；`max_tokens` stop reason 的升级/续写路径不会调用 collapse drain 或 reactive compact。 |
| 6.1 observable input 工具清单 | 已完成 | 2026-05-05 | 文档：本页“6.1 observable input 回填清单”。 | 参考实现中只有 FileRead / FileWrite / FileEdit / SendMessage 有 tool-level `backfillObservableInput()`；hooks / permission `updatedInput` 属于执行边界替换，不再二次回填。 |
| 6.2 only-added-fields clone 策略 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs observable_input_backfill`，4 passed；`cargo test -p cc-engine`，12 passed。 | `Tool` trait 新增 observer-only `backfill_observable_input()`；`backfill_observable_tool_inputs()` 只在 tool backfill 新增字段时 clone assistant message，覆盖已有字段时保留原消息 byte identity。 |
| 6.3 streaming / post-stream 双路径接入 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs query::loop_impl::loop_tests`，22 passed。 | 主 loop yield assistant 前生成 observer-facing clone，但 `state.messages` 和下一轮 model request 继续保留原始 assistant；gate off 和 gate on 的 observable tool input 一致。 |
| 6.4 prompt-cache identity 回归测试 | 已完成 | 2026-05-05 | `observable_input_backfill_keeps_byte_identity_when_only_overwriting_fields`、`observable_input_backfill_clones_yield_without_changing_next_request_gate_off`、`observable_input_backfill_matches_with_streaming_tool_gate_on` 通过。 | 测试断言 file-path 类覆盖字段不触发 clone，SendMessage 类新增字段触发 observer clone，且下一轮请求不携带 observer-only 字段。 |
| 7.1 Attachment stage 完整路径 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs test_hook_stopped_tool_execution_yields_attachment_and_stops`，1 passed；`cargo test -p claude-code-rs test_call_returns_backward_compatible_result_with_safe_write_diagnostics`，1 passed；`cargo test -p claude-code-rs full_read_registers_state_and_edit_refreshes_it`，1 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，25 passed。 | `Write` / `Edit` 成功后产出 `EditedTextFile` attachment；post-tool hook stop 通过 `ToolExecResult.hook_stopped_continuation` 汇聚为 `HookStoppedContinuation` attachment 并停止下一轮；工具 `new_messages` 继续作为 attachment/message 统一从 loop yield。 |
| 7.2 Attachment 上下文边界 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs attachment_policy_sends_only_model_context_attachments_to_api`，1 passed。 | `QueuedCommand` 与 `NestedMemory` 在 API request 构建时转为 meta user 上下文；`EditedTextFile`、`StructuredOutput`、`SkillDiscovery`、`HookStoppedContinuation`、`MaxTurnsReached` 只保留 session / SDK / UI 可观察语义，不进入模型请求。 |
| 7.3 slash command dispatcher 真实执行 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs engine::input_processing::tests`，6 passed；`cargo test -p claude-code-rs test_submit`，4 passed。 | `process_user_input()` 只解析已知 slash command，生命周期层执行 async handler；`Output` 直接返回、`Query` 注入消息继续模型调用、`Clear` 清空状态并轮换 session，错误命令以 typed error result 返回。 |
| 7.4 QueryGates 行为收敛 | 已完成 | 2026-05-05 | `cargo test -p cc-engine query_gates`，3 passed；`cargo test -p claude-code-rs tool_use_summary_gate`，2 passed；`cargo test -p claude-code-rs query::loop_impl::loop_tests`，24 passed。 | `QueryGates` 默认全部关闭，`fast_mode_enabled` 由调用方传入，`CC_RUST_STREAMING_TOOL_EXECUTION` 和 `CC_RUST_EMIT_TOOL_USE_SUMMARIES` 分别打开 streaming tool execution 与 typed `ToolUseSummary` 输出；summary gate 关闭时 query loop 不产生 summary 事件。 |
| 7.5 TUI tool progress callback | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs tui_tool_progress`，1 passed；`cargo test -p claude-code-rs tui::tests`，10 passed。 | Headless/IPC 已有 `ToolProgress -> BackendMessage::ToolProgress`；直接 Rust TUI 现在也安装 tool progress callback，把 Bash 等长任务进度转成 spinner 文本和 `ProgressMessage`，同一 `tool_use_id` 的进度会替换上一条进度消息。 |
| 7.6 daemon SSE 事件覆盖 | 已完成 | 2026-05-05 | `cargo test -p claude-code-rs daemon::routes::tests`，3 passed。 | daemon SSE 不再过滤 `ApiRetry`、`CompactBoundary`、`ToolUseSummary`；事件名分别为 `api_retry`、`compact_boundary`、`tool_use_summary`，payload 保留 `message_id`、`session_id` 和对应关键字段。 |

## Subagent 并行拆分规则

可以交给 subagent 并行完成的任务，应满足以下条件：

- 写入范围清晰，最好限定在测试、provider adapter、文档、单个 mapper 或单个工具执行阶段。
- 不需要先决定全局语义，例如 session tombstone 如何持久化、canonical tool execution path 选哪条。
- 可以用独立 fixture 验证，不依赖另一个未合并实现。
- 不会同时改 `loop_impl.rs`、`loop_helpers.rs`、`deps.rs` 这类主状态机核心文件。

不适合并行交给多个 subagent 的任务：

- 需要一次性确定跨层契约的任务，例如 fallback/tombstone 的 SDK、session、UI 可观察语义。
- 会重排主 loop 状态机的任务，例如 `StreamingToolExecutor` 接入、prompt-too-long 恢复链重写。
- 会改变工具执行 canonical path 的任务，因为它影响权限、hooks、validation、progress、abort、result-size。
- 最终集成和验证，因为需要统一读完整 diff、跑测试并判断行为是否真的闭环。

阶段级并行性：

| 阶段 | 可否让 subagent 并行 | 建议拆法 | 不应并行的部分 |
| --- | --- | --- | --- |
| 0：基线测试 | 可并行 | `test-engineer` 分别补 streaming fixture、query loop fixture、recovery fixture。 | 测试命名、fixture 目录结构、哪些测试先标 pending 由 leader 统一决定。 |
| 1：stream 协议正确性 | 部分可并行 | 一个 `executor` 补 `input_json_delta`，另一个补 `signature_delta`，`test-engineer` 同步补 fixture。 | `content_block_stop` 是否产出 per-block assistant message 必须 leader 决策后再改。 |
| 2：工具执行 canonical path | 不建议并行实现 | 可让 `explore`/`architect` 做只读对比，列出两条执行路径差异。 | canonical path 选择、主 loop 迁移、权限/hook/result-size 合并必须串行完成。 |
| 3：模型调用恢复契约 | 部分可并行 | `test-engineer` 先写 fallback/retry/stall fixture；`executor` 可独立补 retry/backoff helper。 | tombstone 生命周期、fallback attempt wrapper、SDK/session/UI 语义必须由 leader 串行集成。 |
| 4：StreamingToolExecutor 接入 | 不建议并行实现 | 可让 `explore` 提前列出 event 边界和现有 coordinator API 缺口。 | 主 loop 接入、gate 行为、已启动工具与 fallback/abort 交互必须单线实现。 |
| 5：Context pipeline | 部分可并行 | `executor` 可独立实现 freed-token 统计；`test-engineer` 补 prompt-too-long fixture。 | context collapse 语义、collapse drain retry 与 autocompact 的最终集成必须串行。 |
| 6：Observable input | 部分可并行 | `explore` 梳理需要回填的工具；`test-engineer` 补 byte identity 测试。 | 回填插入点和 streaming/post-stream 双路径集成必须串行。 |
| 7：Loop 表面能力 | 可并行 | attachment、slash command、query gates、TUI progress、daemon SSE 可按模块分给不同 subagent。 | SDK/IPC/TUI/Web 的最终事件契约需要 leader 统一复核。 |
| 8：Provider parity | 可并行 | Bedrock、Google、Vertex、Azure 可按 provider 独立分配。 | provider capability matrix 和统一错误/stream 语义要集中复核。 |
| 9：文档收敛 | 可并行 | `writer` 可同步更新旧 docs 和 gap/archive 清单。 | intentional 差异的最终措辞和范围必须和代码实际行为一致。 |

任务级并行标注：

| 任务 | 并行性 | 适合的 subagent 输出 |
| --- | --- | --- |
| 0.1 streaming fixture | 可并行 | 新增测试文件或测试用例，列出当前 pending gap。 |
| 0.2 post-stream tool fixture | 可并行 | 锁定旧行为的 e2e / unit fixture。 |
| 0.3 recovery fixture | 可并行 | prompt-too-long、max_tokens、stop hook、token budget 测试矩阵。 |
| 0.4 ownership 梳理 | 不并行 | leader 产出最终 ownership，避免多个子任务写同一核心文件。 |
| 1.1 `input_json_delta` | 可并行，但需独占 `streaming.rs` 相关写入 | 实现 + fixture。 |
| 1.2 `signature_delta` | 可并行，但需和 1.1 协调同文件冲突 | 实现 + fixture。 |
| 1.3 unsupported delta 策略 | 可并行做设计，不建议独立落实现 | 设计建议和影响面。 |
| 1.4 TUI/headless 映射 | 可并行 | mapper/UI 层补齐或验证不渲染 tool input delta。 |
| 1.5 per-block assistant 语义 | 不并行 | leader 决策；实现前需定 SDK/session 语义。 |
| 2.1 canonical path 决策 | 不并行 | leader 决策，可接收只读调研。 |
| 2.2 行为合并 | 不并行 | 主线串行改动，避免权限/hook 规则分叉。 |
| 2.3 concurrency-safe 语义 | 可并行补测试，不建议并行改实现 | 测试矩阵。 |
| 2.4 tool result 标准化 | 部分可并行 | 可先调研和测试，最终接入串行。 |
| 3.1 fallback wrapper | 不并行 | 主线串行实现。 |
| 3.2 tombstone 生命周期 | 不并行 | 主线串行实现。 |
| 3.3 signature 清理 | 部分可并行 | 可独立补 helper / test，最终接入 fallback 串行。 |
| 3.4 retry/backoff | 可并行 | 独立 helper + unit tests。 |
| 3.5 idle/stall watchdog | 可并行做 helper，不建议独立接主 loop | timeout helper + tests。 |
| 3.6 withheld stream error | 不并行 | 依赖 fallback/tombstone 语义。 |
| 4.1-4.6 StreamingToolExecutor 接入 | 不并行实现 | 只适合并行做只读分析、测试准备、review。 |
| 5.1 context collapse | 部分可并行 | 可独立做 collapse primitive；最终 pipeline 接入串行。 |
| 5.2 tool pair 完整性 | 可并行测试 | 构造 collapse 后 tool_use/tool_result 配对测试。 |
| 5.3 freed-token 统计 | 可并行 | pipeline result 扩展 + tests，写入范围清楚时可独立。 |
| 5.4 collapse drain retry | 不并行 | 主 loop 恢复链串行实现。 |
| 5.5 max_tokens 交互 | 可并行测试 | 边界 fixture。 |
| 6.1 工具清单 | 可并行 | 只读梳理表。 |
| 6.2 clone 策略 | 部分可并行 | helper + byte identity tests。 |
| 6.3 双路径接入 | 不并行 | 依赖阶段 4，主线串行。 |
| 6.4 prompt-cache 测试 | 可并行 | 回归测试。 |
| 7.1-7.6 表面能力 | 可并行 | 按 attachment、slash command、gates、TUI、daemon 拆独立写入范围。 |
| 8.1-8.5 provider parity | 可并行 | 每个 provider 一个独立子任务，避免共享文件冲突时由 leader 集成 capability matrix。 |
| 9.1-9.3 文档收敛 | 可并行 | 文档 PR / patch，最终由 leader 对照代码复核。 |

## 阶段 0：基线测试与任务边界

目的：先把当前行为和目标行为固定下来，避免后续大改时无法判断是修复还是回归。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 0.1 | 为 `StreamAccumulator` 建 Anthropic-style fixture：text、thinking、tool_use、message_delta、message_stop。 | `streaming.md` | 当前已支持的 text/thinking/usage 通过；tool input delta 可先作为 pending/failing case。 |
| 0.2 | 为 query loop 建最小 post-stream tool execution fixture。 | `agent-loop.md`、`the-loop.md` | 能证明当前是 stream 结束后执行工具，后续改成 stream-time 时测试会同步改期望。 |
| 0.3 | 为 prompt-too-long、max_tokens、stop hook、token budget 建恢复路径 fixture。 | `agent-loop.md`、`the-loop.md` | 已实现路径被锁定，未实现路径以 TODO 测试或文档化 test gap 记录。 |
| 0.4 | 梳理主 loop 入口和工具执行入口的 ownership。 | `agent-loop.md` | 明确哪些文件是阶段 1-4 的主要改动面。 |

建议主要文件：

- `crates/claude-code-rs/src/api/streaming.rs`
- `crates/claude-code-rs/src/query/loop_impl.rs`
- `crates/claude-code-rs/src/query/loop_helpers.rs`
- `crates/claude-code-rs/src/tools/execution/`

## 阶段 1-4 ownership

0.4 决策：阶段 1-4 的核心状态机和工具执行边界由 leader 串行维护。subagent 可以并行做只读对比、测试夹具、provider adapter 或 mapper 层补丁，但不能同时改同一条主 loop / tool execution contract。

| 范围 | 主要文件 | ownership | 可并行工作 | 串行要求 |
| --- | --- | --- | --- | --- |
| Stream 协议累积 | `crates/claude-code-rs/src/api/streaming.rs` | 阶段 1 的协议 owner。 | 可以并行补 provider fixture、TUI/headless 不渲染测试。 | `ContentBlock` / `StreamEvent` 解释规则必须单一 owner 合并，避免不同 delta 类型被重复解释。 |
| Stream 可见面映射 | `crates/claude-code-rs/src/ipc/sdk_mapper.rs`、`crates/claude-code-rs/src/ui/tui/engine_events.rs`、`crates/claude-code-rs/src/engine/agent/mod.rs` | mapper owner，在 accumulator 契约稳定后跟进。 | 可按 IPC、TUI、agent event 独立拆给 subagent。 | 不得自行改变最终 assistant / per-block assistant 交付语义；该语义由阶段 1.5 / 阶段 4 统一决策。 |
| 主 query loop 状态机 | `crates/claude-code-rs/src/query/loop_impl.rs` | leader 独占 owner。 | 只允许并行做只读分析和不改状态机的测试准备。 | fallback/tombstone、stream-time tool scheduling、collapse drain、terminal / continue transition 必须串行改。 |
| Loop helper 与恢复 helper | `crates/claude-code-rs/src/query/loop_helpers.rs` | 跟随 `loop_impl.rs` 的同一 owner。 | 可并行补 helper unit tests。 | helper 改动若影响 `QueryLoopState`、`Continue`、工具批处理顺序，必须和 `loop_impl.rs` 同批串行集成。 |
| QueryDeps 边界 | `crates/claude-code-rs/src/query/deps.rs`、`crates/claude-code-rs/src/engine/lifecycle/deps.rs` | 阶段 2 canonical path owner。 | 可并行调研当前 `QueryEngineDeps::execute_tool()` 与 `run_tool_use()` 差异。 | `ToolExecRequest`、progress callback、permission/hook/abort/result-size 语义不能由多个任务分叉修改。 |
| 通用工具执行管线 | `crates/claude-code-rs/src/tools/execution/pipeline.rs`、`crates/claude-code-rs/src/tools/execution/coordinator.rs` | 阶段 2/4 tool execution owner。 | coordinator API 缺口、pipeline 单元测试可并行准备。 | canonical path 决策前不迁移主 loop；`StreamingToolExecutor` 接入必须复用阶段 2 的唯一规则入口。 |
| 工具与权限支撑面 | `crates/claude-code-rs/src/tools/**`、`crates/claude-code-rs/src/permissions/**` | 由 canonical path owner 牵头，具体工具可模块 owner 跟进。 | 单个工具 validation / permission fixture 可以并行。 | 不得把阶段 2 缺失能力临时复制进单个工具，除非文档标记 intentional。 |

阶段 1-4 的提交顺序也按 ownership 收敛：

1. `streaming.rs` 协议规则先稳定。
2. mapper / TUI / headless 只同步协议可见面，不改主 loop 语义。
3. 阶段 2 先决定 canonical tool execution path，再改 `QueryDeps` / `QueryEngineDeps` / `tools/execution`。
4. 阶段 3 的 fallback / tombstone wrapper 只由主 loop owner 串行接入。
5. 阶段 4 最后改 `loop_impl.rs` 接入 `StreamingToolExecutor`，并复用阶段 2 的工具执行入口。

## 阶段 1：Streaming content-block 协议正确性

这是最高优先级，因为不补 `input_json_delta` 就无法可靠识别 streaming tool_use 的完整参数。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 1.1 | 在 `StreamAccumulator` 中累积 `input_json_delta.partial_json`。 | `streaming.md` | 多个 partial JSON delta 在 `content_block_stop` 或最终 build 时解析成完整 `ToolUse.input`。 |
| 1.2 | 在 `StreamAccumulator` 中处理 `signature_delta`。 | `streaming.md` | `ContentBlock::Thinking.signature` 最终包含 provider 返回的签名。 |
| 1.3 | 定义未知 / 暂不支持 delta 的处理策略。 | `streaming.md` | `server_tool_use`、`connector_text` 不再静默误判；至少保留可观察 warning 或明确 unsupported。 |
| 1.4 | 同步 TUI/headless 映射边界。 | `streaming.md` | text/thinking 继续实时渲染；tool input delta 不乱渲染，但最终 assistant/tool_use 参数完整。 |
| 1.5 | 明确 `content_block_stop` 交付语义。 | `streaming.md`、`the-loop.md` | 决定是否实现 per-block `AssistantMessage`。如果保留最终单 assistant，文档标注为 intentional。 |

验收测试：

- Anthropic-style tool_use：`content_block_start` 初始 input `{}`，多个 `input_json_delta` 后最终 input 完整。
- Thinking：`thinking_delta` + `signature_delta` 后最终 thinking block 完整。
- Bedrock synthesized tool_use 不再因为 `input_json_delta` 未累积而丢参数。

## 阶段 2：工具执行 canonical path

当前主 loop 使用 `loop_helpers::execute_tool_calls()` / `QueryEngineDeps::execute_tool()`，同时存在 `tools/execution/pipeline.rs::run_tool_use()`。在接入 `StreamingToolExecutor` 前必须确定唯一规则入口。

### 2.1 canonical path 决策

决策：主 query loop 的 canonical 工具执行边界是 `QueryDeps::execute_tool()`，生产实现是 `QueryEngineDeps::execute_tool()`。

理由：

- `QueryEngineDeps::execute_tool()` 已经持有 lifecycle state、`permission_callback`、`ask_user_callback`、`tool_progress_callback`、audit context、Langfuse trace、file state cache、background agent channel、command dispatcher。
- 当前主 loop 的 post-stream 批处理已经通过 `loop_helpers::execute_tool_calls()` 进入 `QueryDeps::execute_tool()`，0.2 fixture 已锁定这个边界。
- `QueryEngineDeps::execute_tool()` 会保留 `ToolResult.model_content`、`display_preview` 和 `new_messages`，这对 screenshot / computer-use 等多模态工具结果进入下一轮很关键。
- `tools/execution/pipeline.rs::run_tool_use()` 覆盖了 `validate_input`、输入 sanitization、`security_validate()`、`enforce_result_size()` 等阶段，但当前不拥有交互式 permission callback、progress tool_use_id 补全、audit / Langfuse span、完整 `ToolResult` 保真，也不是主 loop 路径。

后续要求：

- 2.2 不迁移主 loop 直接调用 `run_tool_use()`；而是把 `run_tool_use()` 中缺失的 validation、sanitization、security、result-size、hook-stopped-continuation 语义合入 `QueryEngineDeps::execute_tool()` 或它调用的共享 helper。
- 2.3 的 concurrency-safe 批处理继续由 `loop_helpers::execute_tool_calls()` 控制，直到阶段 4 引入 stream-time scheduler。
- 2.4 统一 `ToolExecResult` 到 `tool_result` user message 的标准化，避免 `run_tool_use()` 和 `QueryEngineDeps::execute_tool()` 各自组装不同结果。
- 阶段 4 接入 `StreamingToolExecutor` 时，executor 必须通过 `QueryDeps::execute_tool()` / `ToolExecRequest` 调度工具，不能直接复用当前 `run_tool_use()` 绕过 canonical 边界。
- `run_tool_use()` 暂保留为参考实现和测试覆盖来源；当阶段 2 合并完成后，再决定删除、降级为 helper，或让它内部委托给 canonical 边界。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 2.1 | 决定 canonical path：迁移主 loop 到 `run_tool_use()`，或把缺失阶段合入 `QueryEngineDeps::execute_tool()`。 | `agent-loop.md`、`the-loop.md` | 架构文档和代码注释都指向同一个工具执行入口。 |
| 2.2 | 合并 validation / permission / hooks / result-size / progress / abort 行为。 | `agent-loop.md` | allow、deny、ask、validation failure、oversized result 都走同一套规则。 |
| 2.3 | 保留现有 concurrency-safe 批处理语义。 | `agent-loop.md` | 连续 safe tools 可并发，unsafe tools 串行，行为有测试覆盖。 |
| 2.4 | 统一工具结果标准化和 `tool_result` user message 生成。 | `the-loop.md` | 工具结果进入下一轮前经过一致的 normalize/size budget 处理。 |

验收测试：

- 权限 allow / deny / ask。
- PreToolUse / PostToolUse hook。
- oversized result。
- 并发 safe tools 与 serial unsafe tools 混排。
- abort in tool execution。

## 阶段 3：模型调用恢复契约

先在 post-stream 工具执行模型下补齐 fallback / retry，可以降低与 `StreamingToolExecutor` 的耦合。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 3.1 | 实现 `attemptWithFallback` 等价 wrapper。 | `agent-loop.md`、`the-loop.md` | `QueryParams::fallback_model` 真正参与模型调用重试。 |
| 3.2 | 完成 tombstone retry 生命周期。 | `agent-loop.md`、`streaming.md`、`the-loop.md` | fallback 发生时已收集 assistant 内容被 tombstone，SDK/session/UI 可观察。 |
| 3.3 | 清理跨模型不可安全回放的 thinking signature blocks。 | `the-loop.md` | fallback retry 不把旧模型 signature 当成新模型上下文。 |
| 3.4 | 接入 streaming API retry/backoff。 | `streaming.md` | 可重试网络错误、限流、5xx 按策略重试；不可恢复错误立即 terminal。 |
| 3.5 | 增加主动 idle watchdog 和 passive stall 检测。 | `streaming.md` | 长时间无 delta 的连接可中断并进入恢复或报错路径。 |
| 3.6 | 区分 request 建立失败、stream 中途失败、assistant stop reason。 | `the-loop.md` | recoverable stream error 可以 withheld；恢复耗尽后再释放用户可见错误。 |

验收测试：

- primary model stream 中途失败后 fallback model 重试。
- tombstone 事件不被最终 transcript 当作有效 assistant。
- 5xx / 429 backoff。
- idle/stall timeout。
- prompt-too-long 和 max_tokens 与新 wrapper 不冲突。

## 阶段 4：StreamingToolExecutor 接入主 loop

这是 agent loop 行为对齐的核心阶段。阶段 1 确保 tool input 完整，阶段 2 确保执行入口统一，阶段 3 确保 fallback 状态语义稳定。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 4.1 | 在 stream event 消费阶段识别完整 `tool_use` block。 | `agent-loop.md`、`streaming.md`、`the-loop.md` | `content_block_stop` 后可拿到完整 tool name/input/id。 |
| 4.2 | 将完整 tool_use 提交给 `StreamingToolExecutor`。 | `agent-loop.md`、`the-loop.md` | concurrency-safe 工具可在 assistant stream 尚未结束时开始执行。 |
| 4.3 | `StreamingToolExecutor` 复用阶段 2 的 canonical execution path。 | `agent-loop.md` | 权限、hooks、progress、abort、result-size 不分叉。 |
| 4.4 | stream 结束后只等待 remaining results。 | `the-loop.md` | 不重复执行已启动工具。 |
| 4.5 | 用 `QueryGates.streaming_tool_execution` 控制新行为。 | `agent-loop.md`、`the-loop.md` | gate off 时保留 post-stream 批处理；gate on 时启用 stream-time 调度。 |
| 4.6 | 定义 fallback 与已启动工具的交互。 | `streaming.md`、`the-loop.md` | fallback/tombstone 不会让旧 assistant 的工具结果污染新 attempt。 |

验收测试：

- assistant 继续输出文本时，第一个 safe tool 已经开始执行。
- 多个 safe tools 并发，unsafe tool 串行。
- gate off 行为与旧 post-stream 路径一致。
- stream abort / fallback 时已启动工具被取消或隔离。

## 阶段 5：Context pipeline 与 prompt-too-long 完整恢复

这个阶段补齐参考文档中的 pre-processing pipeline 和 prompt-too-long 恢复链。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 5.1 | 实现 context collapse。 | `agent-loop.md`、`the-loop.md` | `applyCollapsesIfNeeded()` 等价阶段不再是 placeholder。 |
| 5.2 | 保护 tool_use / tool_result 配对完整性。 | `the-loop.md` | collapse 不会留下孤立 tool_result 或丢失工具上下文。 |
| 5.3 | 记录 snip / microcompact / collapse freed tokens。 | `agent-loop.md`、`the-loop.md` | freed token 能传入 autocompact 阈值计算。 |
| 5.4 | 实现 `collapse_drain_retry`。 | `agent-loop.md`、`the-loop.md` | prompt-too-long 先 drain pending collapses，再 reactive compact，最后 terminal。 |
| 5.5 | 固化 max-output-tokens 与 context 恢复交互。 | `agent-loop.md` | max_tokens escalate/recovery 不被 context retry 误触发。 |

验收测试：

- 刚 snip/microcompact 释放空间后不重复 autocompact。
- prompt-too-long：collapse drain 成功。
- prompt-too-long：collapse drain 失败但 reactive compact 成功。
- prompt-too-long：两者都失败后 terminal。
- collapse 后工具调用配对仍合法。

## 阶段 6：Tool input observability 与 prompt-cache identity

`backfillObservableInput()` 是参考实现中容易被忽略但影响 prompt cache 的细节。建议在 stream-time 工具执行稳定后补齐，避免输入结构反复重写。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 6.1 | 梳理需要 observable input 回填的工具。 | `agent-loop.md`、`the-loop.md` | 每个需要回填的工具有明确字段和原因。 |
| 6.2 | 实现“只有新增字段才 clone”的回填策略。 | `the-loop.md` | 无新增字段时消息 JSON byte identity 不变。 |
| 6.3 | 把回填位置接入 streaming / post-stream 两种工具执行路径。 | `the-loop.md` | gate on/off 都得到同样的最终 tool input。 |
| 6.4 | 增加 prompt-cache identity 回归测试。 | `the-loop.md` | 无意义重序列化会被测试捕获。 |

### 6.1 observable input 回填清单

参考实现把 `backfillObservableInput()` 定义在 tool trait 上，只在 observer-facing clone 上补 legacy / derived 字段。原始 assistant `tool_use.input` 继续进入 API-bound transcript；这样 prompt cache 的 JSON byte identity 不会因为可观察字段或路径展开被破坏。

回填规则分两层：

- Tool-level backfill：只适用于显式实现 `backfillObservableInput()` 的工具，且必须幂等。
- Execution-boundary replacement：`PreToolUse` hook 或 permission 返回 `updatedInput` 时，新的 input 自己拥有完整 shape，不再套用 tool-level backfill。

| 工具 / 来源 | 需要回填的字段 | 参考实现行为 | Rust 当前状态 | 并行性 |
| --- | --- | --- | --- | --- |
| `Read` / `FileReadTool` | `file_path` | 对 observer clone 展开为 absolute path，供 hooks / allowlist 使用；因为只是覆盖已有字段，不克隆最终 yield message。 | `Tool` trait 没有 `backfill_observable_input`；`Read` 的 hooks / permission 仍看原始 `file_path`。 | 可让 subagent 独立补 tool method 和 unit test；接入 loop 必须串行。 |
| `Write` / `FileWriteTool` | `file_path` | 同 `Read`，只覆盖已有 `file_path`；不改变 API-bound message，也不改变 tool result 里回显的输入路径。 | 同上。 | 可并行补 tool method/test；接入串行。 |
| `Edit` / `FileEditTool` | `file_path` | 同 `Read` / `Write`，用于 hooks / allowlist 的路径规范化。 | 同上。 | 可并行补 tool method/test；接入串行。 |
| `SendMessage` | string message：`type`、`recipient`、`content`；broadcast 额外 `type=broadcast`；structured message：`type`、`recipient`、`request_id`、`approve`、`content`。 | 当缺少 `type` 且能从 `to` / `message` 推导时添加 legacy 字段；因为是新增字段，observer yield message 需要 clone。 | Rust `SendMessageInput.message` 目前是 `String`，只覆盖 string/broadcast 分支；structured message object 仍属于 Stage 7 team surface gap。 | string/broadcast helper 可并行；structured message 支持和 SDK/IPC 表面必须跟 Stage 7 串行。 |
| `_simulatedSedEdit` sanitization | 删除内部字段 | 参考实现只在 Bash execution boundary 做 defense-in-depth strip；这不是 observable backfill。 | Rust canonical path 当前会从 object input 删除该字段，再执行 hook / permission / tool call。 | 不作为 6.2 backfill helper 实现；若调整范围需随 Bash / sed-edit permission 串行复核。 |
| `PreToolUse.updatedInput` | hook 返回的完整 input | 替换 processed input；不再重新跑 tool-level backfill。 | Rust canonical path 已在 `run_pre_tool_hooks()` 后替换 `effective_input`。 | 不适合拆给多个 subagent；属于 execution boundary contract。 |
| permission `updatedInput` | permission / UI 返回的完整 input | 替换 call input；不再重新跑 tool-level backfill。 | Rust canonical path 已在 tool-local / central permission 后替换 `effective_input`。 | 不适合并行实现；会影响权限、hooks、audit 和 prompt-cache identity。 |

6.2 的实现顺序应先补 trait/helper 和 file tool / SendMessage 的纯函数测试，再由主 loop owner 串行接入 observer clone。6.3 再统一 streaming tool execution gate on/off 的双路径行为，避免 stream-time result 与 post-stream result 对 assistant message 做两套不同回填。

## 阶段 7：Loop 表面能力收敛

这些不是最底层语义问题，但会影响 SDK、IPC、TUI、Web 客户端看到的行为完整度。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 7.1 | 补完整 Attachment stage。 | `agent-loop.md`、`the-loop.md` | edited file、queued command、structured output、nested memory、skill discovery 有统一产出路径。 |
| 7.2 | 区分进入模型上下文的 attachment 和只给 UI/SDK 的 attachment。 | `agent-loop.md` | session transcript、SDK event、UI 显示边界清晰。 |
| 7.3 | 接入 slash command dispatcher 的真实异步执行结果。 | `agent-loop.md`、`the-loop.md` | command 可直接返回、生成消息继续 query、或生成 attachment。 |
| 7.4 | 收敛 `QueryGates` 行为。 | `agent-loop.md`、`the-loop.md` | `streaming_tool_execution`、`emit_tool_use_summaries`、`fast_mode_enabled` 都有明确默认值和测试。 |
| 7.5 | 补 TUI tool progress callback 或标记为 intentional。 | `streaming.md` | TUI 能看到 Bash 长任务实时进度，或文档解释为什么只 headless 支持。 |
| 7.6 | 补 daemon SSE 事件覆盖。 | `streaming.md` | `ApiRetry`、`CompactBoundary`、`ToolUseSummary` 是否广播有明确策略。 |

验收测试：

- SDK / IPC mapper e2e。
- TUI streaming progress。
- known / unknown / async slash command。
- gate fixture 覆盖开关行为。

## 阶段 8：Provider parity 扩展

Provider 扩展应放在统一 streaming 协议和恢复语义之后，避免每个 provider 自己处理一套边界。

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 8.1 | 接 Bedrock AWS EventStream。 | `streaming.md` | 不再依赖非 streaming `/invoke` 合成事件。 |
| 8.2 | 补 Google Gemini tool use / thinking 能力，或明确 unsupported。 | `streaming.md` | capability matrix 与真实行为一致。 |
| 8.3 | 补 Vertex service-account JWT exchange。 | `streaming.md` | Vertex 不只依赖环境 token / gcloud fallback。 |
| 8.4 | 整理 Azure provider 命名、能力矩阵和路由。 | `streaming.md` | Azure OpenAI 与 Anthropic-compatible Azure endpoint 不再混淆。 |
| 8.5 | 建模 `server_tool_use`、`connector_text`。 | `streaming.md` | 对应 content block 至少能 round-trip 或明确 unsupported。 |

## 阶段 9：文档与旧入口收敛

任务：

| ID | 任务 | 来源 | 验收 |
| --- | --- | --- | --- |
| 9.1 | 更新 `docs/architecture/conversation/streaming.mdx` 中过时的 `StreamingToolExecutor` 描述。 | `streaming.md` | 不再声称主 loop 已接入未接入的行为。 |
| 9.2 | 将完成的 gap 从 `docs/IMPLEMENTATION_GAPS.md` 迁移到 archive。 | AGENTS.md 项目规则 | 已补齐条目不再停留在 gap 清单。 |
| 9.3 | 为保留的差异标记 intentional。 | `agent-loop.md`、`streaming.md`、`the-loop.md` | 保留 post-stream、单 assistant message 等差异时有明确理由。 |

## 依赖关系图

```text
阶段 0：测试基线
  -> 阶段 1：stream delta / content block 完整性
  -> 阶段 2：工具执行 canonical path
  -> 阶段 3：fallback / tombstone / retry / watchdog
  -> 阶段 4：StreamingToolExecutor 接入
  -> 阶段 5：context collapse / prompt-too-long 完整恢复
  -> 阶段 6：observable input / prompt cache identity
  -> 阶段 7：attachment / slash command / gates / UI-SDK surfaces
  -> 阶段 8：provider parity
  -> 阶段 9：文档收敛
```

并行机会：

- 阶段 1 和阶段 2 可以并行，但阶段 4 必须等两者完成。
- 阶段 5 的 context collapse 可以和阶段 4 的工具执行细节并行，但 `prompt-too-long` 恢复测试需要等阶段 3 的恢复 wrapper 稳定。
- 阶段 8 的 provider parity 可以分 provider 并行，但必须复用阶段 1 和阶段 3 的统一协议 / 恢复语义。

## 第一批建议落地任务

如果从最小可交付切入，建议第一批只做以下 5 个任务：

1. `StreamAccumulator` 支持 `input_json_delta.partial_json`，补 Anthropic-style tool_use streaming 测试。
2. `StreamAccumulator` 支持 `signature_delta`，补 thinking signature 测试。
3. 确定并文档化 canonical tool execution path。
4. 为当前 post-stream tool execution 建 e2e，锁住权限、hooks、并发、oversized result。
5. 设计 `attemptWithFallback` / tombstone 的 SDK 和 session 可观察语义。

这批完成后，才进入 `StreamingToolExecutor` 接入；否则会把协议、工具执行和恢复三类问题揉在同一个大改里，难以验证。
