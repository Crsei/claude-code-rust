# cc-rust Streaming Architecture

记录日期：2026-05-05

本页按 `F:\AIclassmanager\cc\claude-code-bun\docs\conversation\streaming.mdx` 描述的 streaming 模型，整理当前 Rust 项目的 streaming 链路、已实现能力和未实现差距。

## 参考基线

Bun 版文档把 streaming 视为一个统一的内容块事件流：

```text
message_start
  content_block_start
    content_block_delta*
  content_block_stop
  ...
  message_delta
message_stop
```

核心语义：

- `content_block_*` 是 streaming 的最小增量边界；一个 API 响应可包含多个内容块。
- `text_delta` 追加文本；`thinking_delta` 追加思考；`signature_delta` 补齐 thinking 签名。
- `input_json_delta` 追加工具输入 JSON 片段，适用于 `tool_use` / `server_tool_use`。
- `connector_text_delta` 追加 connector 输出文本。
- 每个 `content_block_stop` 都会形成一个可交付的 `AssistantMessage`，`message_delta.stop_reason` 再回填最终停止原因。
- 错误恢复包括流停滞检测、主动 idle watchdog、非 streaming fallback、重试退避、prompt-too-long/context-window 恢复和 max-output-token 续写。
- Bash 等长任务通过 `onProgress` 把工具执行进度推给前端。

## 当前端到端链路

```text
Provider HTTP/SSE 或 synthesized response
  -> StreamProvider::stream()
  -> cc_types::StreamEvent
  -> QueryEngineDeps::call_model_streaming()
  -> query::loop_impl::query()
       - 逐个转发 QueryYield::Stream(event)
       - StreamAccumulator 累积最终 AssistantMessage
       - stream 结束后统一执行 tool_use
  -> engine::lifecycle::submit_message()
       - SdkMessage::StreamEvent
       - SdkMessage::Assistant
       - SdkMessage::Result
  -> TUI / headless JSONL IPC / Web SSE / daemon SSE
```

主要代码入口：

- 事件类型：`crates/cc-types/src/message.rs`
- Anthropic SSE 解析与累积：`crates/claude-code-rs/src/api/streaming.rs`
- SSE 字节流解析：`crates/claude-code-rs/src/api/client/stream.rs`
- Provider 统一接口：`crates/claude-code-rs/src/api/stream_provider.rs`
- Query 主循环：`crates/claude-code-rs/src/query/loop_impl.rs`
- SDK 转换：`crates/claude-code-rs/src/engine/lifecycle/submit_message.rs`
- TUI 增量渲染：`crates/claude-code-rs/src/ui/tui/engine_events.rs`
- Headless IPC 映射：`crates/claude-code-rs/src/ipc/sdk_mapper.rs`
- Web SSE：`crates/claude-code-rs/src/web/sse.rs`
- Daemon SSE：`crates/claude-code-rs/src/daemon/routes.rs`

## 事件状态机对照

| Bun streaming 事件 | Rust 当前处理 | 状态 |
| --- | --- | --- |
| `message_start` | 解析为 `StreamEvent::MessageStart`，记录初始 usage 并转发给 SDK/前端。 | 已实现 |
| `content_block_start` | 解析为 `ContentBlockStart`，`StreamAccumulator` 按 index 保存初始 block。 | 已实现 |
| `content_block_delta.text_delta` | 追加到 `ContentBlock::Text.text`；TUI/headless 也能实时显示文本 delta。 | 已实现 |
| `content_block_delta.thinking_delta` | 追加到 `ContentBlock::Thinking.thinking`；TUI/headless 能实时显示 thinking delta。 | 部分实现 |
| `content_block_delta.signature_delta` | 当前没有写入 `ContentBlock::Thinking.signature`。 | 未实现 |
| `content_block_delta.input_json_delta` | 当前 accumulator/TUI 不拼接 `partial_json`；Anthropic/Vertex/Bedrock 风格的 streaming tool input 可能丢失。 | 未实现 |
| `content_block_delta.connector_text_delta` | 没有对应内容块和 delta 累积逻辑。 | 未实现 |
| `content_block_stop` | 事件会转发；Rust 现阶段有意保留“stream event 实时输出 + message stream 结束后产出一个最终 `AssistantMessage`”的交付语义。 | Intentional / 暂不改 |
| `message_delta` | 更新 `stop_reason` 和 usage；`max_tokens` 会触发后续恢复逻辑。 | 已实现 |
| `message_stop` | 事件会转发；TUI/headless 用它结束当前流。 | 已实现 |
| stream `error` / `ping` | `ping` 忽略；`error` 当前未转成可恢复事件。 | 部分实现 |

## Provider 覆盖

| Provider 类别 | 当前 streaming 方式 | 已实现 | 主要差距 |
| --- | --- | --- | --- |
| Anthropic native / 当前 `ApiProvider::Azure` 路由 | `/v1/messages` SSE，按 Anthropic 事件解析。 | 文本、thinking、usage、stop_reason、tool block 起点。 | `input_json_delta`、`signature_delta` 未累积；流错误恢复不足；Azure 命名/能力矩阵与实际路由需再确认。 |
| Vertex Anthropic | `streamRawPredict`，复用 Anthropic SSE 解析。 | native streaming 主链路。 | 认证仍偏环境/gcloud fallback；同样缺 `input_json_delta`/`signature_delta`。 |
| OpenAI-compatible / Codex / DeepSeek / Qwen 等 | OpenAI chat/completions SSE 转换为统一 `StreamEvent`。 | 文本 delta、usage、finish_reason 映射；DeepSeek `reasoning_content` 映射 thinking；tool calls 在 finish 时转为 `ToolUse` block。 | tool calls 不是实时 `input_json_delta`；thinking 仅覆盖特定兼容字段；代码注释提到 Azure OpenAI，但当前 `ApiProvider::Azure` 不走该分支。 |
| Google Gemini | `streamGenerateContent?alt=sse`，按累计文本 diff 产出 `text_delta`。 | 文本 streaming。 | 工具调用、thinking、prompt cache 未支持。 |
| Bedrock | 当前用非 streaming `/invoke`，再合成 `StreamEvent`。 | 可复用统一下游链路。 | 未接 AWS EventStream；合成 tool_use 依赖 `input_json_delta`，但 accumulator 不拼接，工具输入可能丢失。 |
| Foundry | capability 标为 unsupported。 | 无。 | 未实现 provider。 |

## 消费者层行为

| 消费者 | 当前行为 |
| --- | --- |
| TUI | `SdkMessage::StreamEvent` 维护 partial assistant，只实时追加 text/thinking；收到最终 `SdkMessage::Assistant` 后替换 partial。未看到 TUI 安装 tool progress callback。 |
| Headless JSONL IPC | 把 stream 映射为 `StreamStart`、`StreamDelta`、`ThinkingDelta`、`StreamEnd`；最终 assistant 中的 `ToolUse` 会单独发 `ToolUse`，Bash 进度通过 `ToolProgress` 推送。 |
| Web SSE | 直接把 `SdkMessage` 序列化为 SSE，事件名包括 `stream_event`、`assistant`、`api_retry`、`tool_use_summary`、`result`。 |
| Daemon SSE | 广播简化事件：`stream_start`、`stream_delta`、`assistant_message`、`stream_end` 等；当前跳过 `ApiRetry`、`CompactBoundary`、`ToolUseSummary`。 |

## 已实现

- 统一 `StreamEvent` 类型已覆盖 message/content block/message delta/message stop 的主状态机。
- Anthropic 风格 SSE 字节流解析已实现，支持多行 `data:`、事件尾部 flush。
- Provider 适配层已把 Anthropic、OpenAI-compatible、Google、Vertex、Bedrock 合成响应接入同一流式接口。
- Query 主循环会边接收边转发 `QueryYield::Stream(event)`，并用 `StreamAccumulator` 生成最终 `AssistantMessage`。
- SDK 层会把 stream event、最终 assistant、最终 result 统一输出给 TUI/headless/Web/daemon。
- TUI 支持 text/thinking partial 渲染，最终 assistant 到达后替换 partial。
- Headless IPC 支持 text delta、thinking delta、stream start/end、tool_use、tool_result、tool_progress。
- Bash 工具执行支持约 1 秒间隔的进度 callback，headless 端可收到 `ToolProgress`。
- `message_delta.stop_reason` 和 usage 聚合已实现。
- `max_tokens` 停止原因有恢复路径：先提升 max output tokens，再注入 continuation 消息，最多 3 次。
- prompt-too-long 有 reactive compact retry 路径。
- `input_json_delta.partial_json` 会累积到最终 `ToolUse.input`，`signature_delta` 会写入 thinking signature。
- stream 建立前的 429、5xx、529 / overloaded / high-demand / capacity 和网络发送错误会按 `ApiClientConfig.max_retries` 退避重试；prompt-too-long、auth、invalid request 等不可恢复错误立即返回给上层恢复或 terminal 路径。
- Query 主循环会在 stream 消费阶段执行主动 idle watchdog 和 passive stall 检测，默认 idle 120s / stall 60s，可通过 `CC_RUST_STREAM_IDLE_TIMEOUT_MS`、`CC_RUST_STREAM_STALL_TIMEOUT_MS` 调整。
- stream 中途 capacity 失败触发 fallback 时，主 loop 会 tombstone 已累积 partial assistant；fallback retry 前会移除旧模型的 thinking / redacted-thinking signature blocks。
- 工具执行已支持 stream 结束后的安全工具并发批处理和非安全工具串行执行。

## 未实现 / 未对齐

| 优先级 | 差距 | 影响 |
| --- | --- | --- |
| P0 | `StreamingToolExecutor` 存在但未接入 query 主循环。 | 当前只能在完整 assistant 结束后执行工具，无法像 Bun 版那样按内容块流式启动安全工具。 |
| P1 | mid-stream error 分类、`ApiRetry` 用户可见事件和非 streaming fallback 尚未完整对齐。 | stream 建立前已 retry/backoff；但已开始输出后的错误仍主要由 query fallback/tombstone 路径处理，retry 可见性和 exhausted 后释放策略还需在 3.6 / 7.6 收敛。 |
| P1 | `server_tool_use`、`connector_text` 未建模。 | Web search/server tool/connector 类内容无法按参考协议完整还原。 |
| P1 | `content_block_stop` 不产出 per-block `AssistantMessage`。 | 这是当前有意保留的边界：SDK/session/TUI 仍以最终单 assistant 替换 partial stream；per-block assistant 需要和 `StreamingToolExecutor`、session tombstone/fallback 语义一起重新设计。 |
| P1 | prompt-too-long 只有 reactive compact retry，没有 collapse drain。 | 极端长上下文恢复能力弱于参考设计。 |
| P2 | TUI 未观察到 tool progress callback 安装。 | TUI 可能只能看到工具最终结果，不能显示 Bash 长任务实时进度。 |
| P2 | Daemon SSE 跳过部分 SDK 事件，permission endpoint 仍是 stub。 | daemon/Web 客户端能力不完整。 |
| P2 | Google tool use、Bedrock AWS EventStream、Vertex service-account JWT exchange 等 provider 能力仍未补齐。 | 多 provider 行为还不是 full-build 对齐状态。 |
| P2 | Azure provider 的命名、能力矩阵和 streaming 路由存在不一致。 | 可能导致 Azure OpenAI 与 Anthropic-compatible Azure endpoint 的预期混淆。 |
| P2 | 针对 streaming tool input、mid-stream error、retry、stall/idle 的回归测试不足。 | 后续补齐协议时容易回归。 |

## 建议补齐顺序

1. 接入 `StreamingToolExecutor` 前，继续保留最终单 `AssistantMessage` 交付语义；真正改成 per-block assistant 时，需要同步设计工具 block 完成边界、SDK/session 持久化、fallback tombstone 和 UI partial replacement。
2. 在 3.6 明确 request 建立失败、stream 中途失败、assistant stop reason 的 exhausted/withheld/release 策略。
3. 补 prompt-too-long 的 collapse drain retry，避免只依赖 reactive compact。
4. 补 `ApiRetry` / `CompactBoundary` / `ToolUseSummary` 等事件在 daemon SSE、TUI、headless 中的可见性策略。
5. 再扩展 provider：Bedrock EventStream、Google tool use、server tool/connector content。

## 文档一致性提醒

`docs/architecture/conversation/streaming.mdx` 中已有 streaming 说明，但部分描述与当前代码不完全一致，尤其是 `StreamingToolExecutor` 是否已在主循环中实时执行工具。后续若以本页作为 full-build 对齐清单，应同步更新该旧文档，避免两个 architecture 入口给出不同结论。
