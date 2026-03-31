# Claude Code Rust 重写计划

## 原始项目概况

- **语言**: TypeScript/React (Ink 终端渲染)
- **运行时**: Bun
- **文件数**: ~1896 个 .ts/.tsx 文件
- **代码行数**: ~91,000 行
- **核心架构**: Generator-based 流式查询状态机 + 工具系统 + 终端 UI

---

## 核心架构分析: 主查询状态机

### 请求流 (Request Flow)

```
cli.tsx (入口)
  → main.tsx (初始化: tools, commands, settings, auth, MCP)
    → QueryEngine.ts (会话生命周期管理器)
      → query.ts (Generator 流式循环状态机)
        → services/api/claude.ts (API 调用 + 流式解析)
        → services/tools/toolOrchestration.ts (工具并发执行)
        → services/compact/ (上下文压缩)
```

### QueryEngine (会话管理器)

**文件**: `src/QueryEngine.ts` (~1295 行)

**职责**:
- 一个 QueryEngine 实例 = 一个会话 (conversation)
- 每次 `submitMessage()` = 一个新的对话轮次 (turn)
- 跨轮次维护: messages, file cache, usage tracking, permission denials

**关键方法**: `submitMessage(prompt) -> AsyncGenerator<SDKMessage>`
1. 配置 processUserInputContext (设置、权限、工具)
2. 调用 processUserInput 处理用户输入 (斜杠命令、附件)
3. 构建 systemPrompt (默认 + 自定义 + memory)
4. 委托给 query() 生成器循环
5. 处理 query() 产出的每个消息:
   - assistant → 记录 + 转发 SDK
   - user (tool_result) → 记录
   - system (compact_boundary) → 释放旧消息 GC
   - stream_event → 累计 usage + 可选转发
   - attachment → 处理结构化输出 / max_turns 等
6. 预算检查 (maxBudgetUsd)
7. 生成最终 result 消息

### query() 生成器状态机

**文件**: `src/query.ts` (~1729 行)

这是**整个系统的核心**。一个 `while(true)` 循环，每次迭代 = 一个 API 调用 + 工具执行周期。

#### 状态定义 (State struct)

```
State {
  messages: Message[]              // 当前对话历史
  toolUseContext: ToolUseContext    // 工具执行上下文
  autoCompactTracking              // 自动压缩跟踪
  maxOutputTokensRecoveryCount     // 输出 token 超限恢复计数
  hasAttemptedReactiveCompact      // 是否已尝试响应式压缩
  maxOutputTokensOverride          // 输出 token 上限覆盖
  pendingToolUseSummary            // 待处理的工具摘要
  stopHookActive                   // 停止钩子是否激活
  turnCount                        // 轮次计数
  transition: Continue | undefined // 上一次迭代的 continue 原因
}
```

#### 状态转换 (Transitions)

每次循环迭代的流程:

```
┌─────────────────────────────────────────────────────────────┐
│ 1. SETUP (每次迭代入口)                                       │
│    - 解构 state                                               │
│    - 技能发现预取 (skill discovery prefetch)                    │
│    - 查询链跟踪 (queryTracking) 递增                           │
│    - 获取 compactBoundary 之后的消息                           │
├─────────────────────────────────────────────────────────────┤
│ 2. CONTEXT MANAGEMENT (上下文管理)                             │
│    - applyToolResultBudget (工具结果大小预算)                   │
│    - snipCompact (历史裁剪)                                    │
│    - microcompact (微压缩)                                     │
│    - contextCollapse (上下文折叠)                               │
│    - autoCompact (自动压缩: 完整摘要)                           │
├─────────────────────────────────────────────────────────────┤
│ 3. API CALL (模型调用)                                        │
│    - calculateTokenWarningState → blocking limit 检查           │
│    - queryModelWithStreaming (流式调用 API)                     │
│    - 流式处理: 收集 assistantMessages + toolUseBlocks           │
│    - StreamingToolExecutor: 并行执行已完成的工具调用             │
│    - 错误扣留: prompt_too_long / max_output_tokens / media_size │
│    - 模型降级重试 (FallbackTriggeredError)                     │
├─────────────────────────────────────────────────────────────┤
│ 4. POST-STREAMING (流结束后处理)                               │
│    - abort 检查 → 产出中断消息                                  │
│    - 处理 pendingToolUseSummary                                │
├─────────────────────────────────────────────────────────────┤
│ 5. TERMINAL CHECK (终止检查, needsFollowUp == false)           │
│    - prompt_too_long 恢复:                                     │
│      a) collapse_drain_retry (上下文折叠排空)                   │
│      b) reactive_compact_retry (响应式压缩)                     │
│      c) 不可恢复 → 返回错误                                    │
│    - max_output_tokens 恢复:                                   │
│      a) max_output_tokens_escalate (8k→64k)                    │
│      b) max_output_tokens_recovery (注入续写消息, 最多3次)       │
│      c) 恢复耗尽 → 返回错误                                    │
│    - stop hooks 评估                                           │
│    - token budget 检查                                         │
│    - → return Terminal { reason }                              │
├─────────────────────────────────────────────────────────────┤
│ 6. TOOL EXECUTION (工具执行, needsFollowUp == true)            │
│    - runTools / StreamingToolExecutor.getRemainingResults()     │
│    - 工具分区: concurrency-safe → 并行, 其余 → 串行             │
│    - hook_stopped_continuation 检查                            │
│    - abort 检查 (工具执行期间)                                  │
├─────────────────────────────────────────────────────────────┤
│ 7. ATTACHMENTS (附件注入)                                     │
│    - getAttachmentMessages (文件变更、排队命令)                  │
│    - memory prefetch 消费                                      │
│    - skill discovery prefetch 消费                             │
│    - 排队命令消费                                              │
├─────────────────────────────────────────────────────────────┤
│ 8. CONTINUE (循环继续)                                        │
│    - 刷新工具列表 (MCP 热加载)                                  │
│    - maxTurns 检查                                             │
│    - task summary 生成                                         │
│    - state = next { reason: 'next_turn' }                      │
│    → continue (回到步骤 1)                                     │
└─────────────────────────────────────────────────────────────┘
```

#### 所有 Continue 转换原因

| reason | 触发条件 | 说明 |
|--------|---------|------|
| `next_turn` | 工具执行完毕 | 正常循环: 带工具结果回模型 |
| `collapse_drain_retry` | prompt_too_long + context_collapse 有效 | 排空折叠队列后重试 |
| `reactive_compact_retry` | prompt_too_long/media_error + 压缩成功 | 响应式压缩后重试 |
| `max_output_tokens_escalate` | max_output_tokens + 未覆盖 + 特性开关 | 从 8k 升级到 64k 重试 |
| `max_output_tokens_recovery` | max_output_tokens + 恢复计数 < 3 | 注入续写指令后重试 |
| `stop_hook_blocking` | stop hook 返回阻塞错误 | 带 hook 错误回模型 |
| `token_budget_continuation` | token 预算未达 90% | 注入续写指令继续 |

#### 所有 Terminal (终止) 原因

| reason | 说明 |
|--------|------|
| `completed` | 正常完成 (无工具调用) |
| `aborted_streaming` | 流式阶段用户中断 |
| `aborted_tools` | 工具执行阶段用户中断 |
| `blocking_limit` | 达到 token 硬上限 |
| `prompt_too_long` | prompt 过长且不可恢复 |
| `image_error` | 图片大小错误 |
| `model_error` | API 调用异常 |
| `hook_stopped` | hook 阻止继续 |
| `stop_hook_prevented` | stop hook 阻止 |
| `max_turns` | 达到最大轮次 |

### 工具系统 (Tool System)

**Tool trait 定义** (`src/Tool.ts`):

```
Tool {
  name: string
  description(input) -> string
  inputSchema: ZodSchema
  call(args, context, canUseTool, parentMessage, onProgress) -> ToolResult
  checkPermissions(input, context) -> PermissionResult
  isEnabled() -> bool
  isConcurrencySafe(input) -> bool
  isReadOnly(input) -> bool
  isDestructive?(input) -> bool
  validateInput?(input, context) -> ValidationResult
  prompt(options) -> string  // 工具的系统提示词
  userFacingName(input) -> string
  maxResultSizeChars: number
  // ... 大量 UI 渲染方法
}
```

**工具编排** (`services/tools/toolOrchestration.ts`):
- `partitionToolCalls`: 将工具调用分为并发安全批次 / 串行批次
- 并发安全工具 → 最多 10 并发执行
- 非并发安全工具 → 严格串行

**ToolUseContext** — 工具执行时的环境上下文:
- options (commands, tools, model, MCP clients, ...)
- abortController
- readFileState (文件内容缓存)
- getAppState / setAppState
- messages (当前对话历史)
- permission context
- 各种状态更新回调

### 消息类型系统

```
Message = UserMessage | AssistantMessage | SystemMessage
        | ProgressMessage | AttachmentMessage

UserMessage {
  type: 'user'
  message: { role: 'user', content: string | ContentBlock[] }
  uuid, timestamp
  isMeta?, toolUseResult?, sourceToolAssistantUUID?
}

AssistantMessage {
  type: 'assistant'
  message: { role: 'assistant', content: ContentBlock[], usage, stop_reason }
  uuid, timestamp
  isApiErrorMessage?, apiError?
  costUSD
}

StreamEvent { type: 'stream_event', event: MessageStreamEvent }
TombstoneMessage { type: 'tombstone', message: AssistantMessage }
ToolUseSummaryMessage { type: 'tool_use_summary', summary, precedingToolUseIds }
```

### 依赖注入 (QueryDeps)

```
QueryDeps {
  callModel: queryModelWithStreaming   // API 调用
  microcompact: microcompactMessages   // 微压缩
  autocompact: autoCompactIfNeeded     // 自动压缩
  uuid: () -> string                   // UUID 生成
}
```

---

## Rust 重写优先级

### 原则
1. **离线优先**: 所有需要登录、联网、API 调用的功能放到最后
2. **核心先行**: 先建立类型系统和状态机骨架
3. **可测试**: 每个阶段都要有可编译运行的代码
4. **增量替换**: 不是一次性全部重写，而是模块化替换

---

### Phase 0: 类型基础 (无网络依赖)
**目标**: 建立 Rust 类型系统，对应 TypeScript 的核心类型

- [x] **P0.1** `types/message.rs` — Message 枚举 (UserMessage, AssistantMessage, SystemMessage, etc.)
- [x] **P0.2** `types/tool.rs` — Tool trait 定义 + ToolUseContext + ToolResult + PermissionResult
- [x] **P0.3** `types/state.rs` — State 结构体 (query 循环状态)
- [x] **P0.4** `types/config.rs` — QueryConfig, QueryParams
- [x] **P0.5** `types/app_state.rs` — AppState (简化版, 无 React)
- [x] **P0.6** `types/transitions.rs` — Terminal / Continue 枚举

### Phase 1: 状态机骨架 (无网络依赖)
**目标**: 实现 query loop 的控制流，使用 mock API
**详细文档**: [`QUERY_ENGINE_SESSION_LIFECYCLE.md`](QUERY_ENGINE_SESSION_LIFECYCLE.md) — QueryEngine Session 完整生命周期、消息分发、持久化管线、预算检查、Result 生成、abort 控制、Rust 重构路线图

- [x] **P1.1** `query/mod.rs` — query() async generator (用 async Stream)
- [x] **P1.2** `query/state.rs` — State 转换逻辑
- [x] **P1.3** `query/deps.rs` — QueryDeps trait (可 mock)
- [x] **P1.4** `query/token_budget.rs` — BudgetTracker + checkTokenBudget
- [x] **P1.5** `engine.rs` — QueryEngine struct + submitMessage()
- [x] **P1.6** `query/stop_hooks.rs` — handleStopHooks 骨架

### Phase 2: 本地工具系统 (无网络依赖)
**目标**: 实现纯本地的工具

- [x] **P2.1** `tools/mod.rs` — Tool trait + 注册表 + 分区逻辑 (partitionToolCalls)
- [x] **P2.2** `tools/orchestration.rs` — runTools (并发/串行编排)
- [x] **P2.3** `tools/bash.rs` — BashTool (进程执行)
- [x] **P2.4** `tools/file_read.rs` — FileReadTool
- [x] **P2.5** `tools/file_write.rs` — FileWriteTool
- [x] **P2.6** `tools/file_edit.rs` — FileEditTool (diff-based 编辑)
- [x] **P2.7** `tools/glob.rs` — GlobTool (文件搜索)
- [x] **P2.8** `tools/grep.rs` — GrepTool (内容搜索)
- [x] **P2.9** `tools/notebook_edit.rs` — NotebookEditTool

### Phase 3: 权限与配置 (无网络依赖)
**目标**: 本地权限系统和配置管理
**详细文档**: [`TOOL_EXECUTION_STATE_MACHINE.md`](TOOL_EXECUTION_STATE_MACHINE.md) — 工具执行全流程状态机、权限检查管线、并发编排、Denial Tracking、Rust 类型映射

- [x] **P3.1** `permissions/mod.rs` — PermissionMode + PermissionResult
- [x] **P3.2** `permissions/rules.rs` — 允许/拒绝规则匹配
- [x] **P3.3** `permissions/dangerous.rs` — 危险命令检测
- [x] **P3.4** `config/settings.rs` — 设置加载 (本地 JSON)
- [x] **P3.5** `config/claude_md.rs` — CLAUDE.md 解析注入

### Phase 4: 上下文管理 (无网络依赖)
**目标**: 消息历史管理和压缩 (本地计算部分)
**详细文档**: [`COMPACTION_RETRY_STATE_MACHINE.md`](COMPACTION_RETRY_STATE_MACHINE.md) — 6 层压缩架构、阈值体系、AutoCompact 电路断路器、PTL 重试、Microcompact 三路分支、Recovery 状态机、Rust 类型映射

- [x] **P4.1** `compact/messages.rs` — normalizeMessagesForAPI
- [x] **P4.2** `compact/microcompact.rs` — microcompactMessages
- [x] **P4.3** `compact/snip.rs` — snipCompact (历史裁剪)
- [x] **P4.4** `compact/tool_result_budget.rs` — applyToolResultBudget
- [x] **P4.5** `utils/file_state_cache.rs` — FileStateCache (LRU)
- [x] **P4.6** `utils/tokens.rs` — token 估算

### Phase 5: 终端 UI (无网络依赖)
**目标**: 基于 ratatui 的终端渲染

- [x] **P5.1** `ui/app.rs` — 主 App 框架 (ratatui)
- [x] **P5.2** `ui/messages.rs` — 消息渲染
- [x] **P5.3** `ui/prompt_input.rs` — 用户输入框
- [x] **P5.4** `ui/spinner.rs` — 加载动画
- [x] **P5.5** `ui/permissions.rs` — 权限确认对话框
- [x] **P5.6** `ui/diff.rs` — Diff 渲染

### Phase 6: 会话持久化 (无网络依赖)
**目标**: 本地会话存储和恢复

- [x] **P6.1** `session/storage.rs` — 会话 JSON 存储
- [x] **P6.2** `session/transcript.rs` — 对话记录
- [x] **P6.3** `session/resume.rs` — /resume 恢复

### Phase 7: 命令系统 (无网络依赖)
**目标**: 斜杠命令

- [x] **P7.1** `commands/mod.rs` — 命令注册表
- [x] **P7.2** `commands/compact.rs` — /compact
- [x] **P7.3** `commands/clear.rs` — /clear
- [x] **P7.4** `commands/help.rs` — /help
- [x] **P7.5** `commands/config.rs` — /config
- [x] **P7.6** `commands/diff.rs` — /diff

### Phase 8: 高级本地工具 (无网络依赖)
**目标**: 复杂本地工具

- [x] **P8.1** `tools/agent.rs` — AgentTool (子代理)
- [x] **P8.2** `tools/task_*.rs` — TaskCreate/Get/Update/List/Stop
- [x] **P8.3** `tools/todo_write.rs` — TodoWriteTool
- [x] **P8.4** `tools/plan_mode.rs` — EnterPlanMode/ExitPlanMode
- [x] **P8.5** `tools/worktree.rs` — EnterWorktree/ExitWorktree
- [x] **P8.6** `tools/lsp.rs` — LSPTool
- [x] **P8.7** `tools/skill.rs` — SkillTool

---

### Phase 9: API 客户端 (需要网络) ⚠️ 低优先级
**目标**: Anthropic API 集成

- [x] **P9.1** `api/client.rs` — API 客户端基础 (reqwest + SSE)
- [x] **P9.2** `api/streaming.rs` — 流式响应解析
- [x] **P9.3** `api/retry.rs` — 重试逻辑 + 降级
- [x] **P9.4** `api/providers/anthropic.rs` — 直连 Anthropic
- [x] **P9.5** `api/providers/bedrock.rs` — AWS Bedrock
- [x] **P9.6** `api/providers/vertex.rs` — GCP Vertex AI

### Phase 10: 认证系统 (需要网络) ⚠️ 低优先级
**目标**: OAuth 和密钥管理

- [x] **P10.1** `auth/api_key.rs` — API Key 存储
- [x] **P10.2** `auth/keychain.rs` — 系统钥匙串
- [x] **P10.3** `auth/oauth.rs` — OAuth 流程
- [x] **P10.4** `auth/token.rs` — Token 刷新

### Phase 11: MCP 协议 (需要网络) ⚠️ 低优先级
**目标**: Model Context Protocol

- [x] **P11.1** `mcp/client.rs` — MCP 客户端
- [x] **P11.2** `mcp/discovery.rs` — 服务器发现
- [x] **P11.3** `mcp/tools.rs` — MCP 工具集成
- [x] **P11.4** `mcp/permissions.rs` — MCP 权限

### Phase 12: 网络工具 (需要网络) ⚠️ 低优先级
**目标**: 需联网的工具

- [x] **P12.1** `tools/web_fetch.rs` — WebFetchTool
- [x] **P12.2** `tools/web_search.rs` — WebSearchTool

### Phase 13: 远程/遥测 (需要网络) ⚠️ 低优先级
- [x] **P13.1** `analytics/mod.rs` — 遥测
- [x] **P13.2** `remote/session.rs` — 远程会话
- [x] **P13.3** `remote/trigger.rs` — 远程触发器

---

## Rust 与 TypeScript 的关键映射

| TypeScript 概念 | Rust 对应 |
|----------------|-----------|
| `AsyncGenerator<T>` | `impl Stream<Item = T>` (futures/tokio-stream) |
| `interface Tool` | `trait Tool` |
| `type Message = A \| B \| C` | `enum Message { A(...), B(...), C(...) }` |
| `ToolUseContext` (大对象) | `struct ToolUseContext` (部分用 Arc 共享) |
| `DeepImmutable<AppState>` | `Arc<AppState>` (不可变共享) |
| `AbortController` | `tokio::sync::watch` / `CancellationToken` |
| feature() gates | Cargo features |
| `require()` 懒加载 | Cargo features + `#[cfg(feature)]` |
| `z.infer<Schema>` (Zod) | `#[derive(Deserialize)]` struct |
| React/Ink (UI) | ratatui + crossterm |
| processUserInput | 独立的 input processor 模块 |

## 开发注意事项

1. **Generator → Stream**: TypeScript 的 `async function*` 映射为 Rust 的 `async_stream::stream!` 或手写 `Poll`
2. **State 对象**: query loop 的 State 是一个 mutable struct，在 Rust 中直接 `&mut State`
3. **工具并发**: `runToolsConcurrently` 用 `tokio::JoinSet` 或 `FuturesUnordered`
4. **权限系统**: `CanUseToolFn` 是闭包回调 → Rust 用 `Box<dyn Fn(...) -> ...>` 或 trait object
5. **消息不可变性**: 原始 API 消息不能 mutate (cache 对齐) → Rust 中用 `Arc<Message>` + clone-on-write
