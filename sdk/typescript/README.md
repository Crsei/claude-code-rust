# claude-code-rs TypeScript SDK

> 版本: 0.1.0 | 最后更新: 2026-04-06

`claude-code-rs` CLI 的 TypeScript 封装 SDK，提供类型安全的流式接口，支持通过编程方式与 Claude Code Rust 代理交互。

架构参考: [OpenAI Codex TypeScript SDK](../../codex/sdk/typescript/)

---

## 目录

1. [架构概述](#架构概述)
2. [前置要求](#前置要求)
3. [构建与安装](#构建与安装)
4. [快速开始](#快速开始)
5. [API 参考](#api-参考)
6. [事件类型](#事件类型)
7. [项目结构](#项目结构)
8. [Rust 侧变更说明](#rust-侧变更说明)
9. [JSONL 协议](#jsonl-协议)
10. [测试](#测试)
11. [设计决策](#设计决策)

---

## 架构概述

```
┌─────────────────────────────────────────────────┐
│  TypeScript SDK (本包)                           │
│                                                  │
│  ClaudeCode ──→ Session ──→ ClaudeCodeExec      │
│   (客户端)      (会话)       (进程管理)           │
│                    │                              │
│                    │  async *run()                │
│                    ▼                              │
│   spawn("claude-code-rs --output-format json")   │
│         │                           ▲             │
│   stdin │ (prompt)       stdout     │ (JSONL)     │
│         ▼                           │             │
│  ┌──────────────────────────────────┘             │
│  │  readline → JSON.parse → transformRawEvent()  │
│  │                    │                           │
│  │                    ▼                           │
│  │           yield SessionEvent                  │
│  └───────────────────────────────────────────────│
└─────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────┐
│  claude-code-rs (Rust CLI 二进制)                │
│                                                  │
│  --output-format json 模式:                      │
│  QueryEngine::submit_message()                   │
│       │                                          │
│       ▼                                          │
│  SdkMessage stream → serde_json::to_string()    │
│       │                                          │
│       ▼                                          │
│  println!("{}", json)  (每行一个 JSON 对象)       │
└─────────────────────────────────────────────────┘
```

**核心思路**: SDK 不直接调用 API，而是封装 CLI 二进制的子进程。通过 stdin 传入 prompt，通过 stdout 逐行读取 JSONL 事件。这与 OpenAI Codex SDK 的架构完全一致。

---

## 前置要求

- **Node.js** >= 18
- **claude-code-rs** 二进制 (已构建)

### 构建 Rust 二进制

```bash
cd rust/
cargo build --release
```

构建产物: `rust/target/release/claude-code-rs` (Windows: `claude-code-rs.exe`)

---

## 构建与安装

```bash
cd rust/sdk/typescript/

# 安装依赖
npm install

# 构建
npm run build

# 类型检查
npx tsc --noEmit
```

构建产物输出到 `dist/`:
- `dist/index.js` — ESM 模块入口
- `dist/index.d.ts` — TypeScript 类型声明

---

## 快速开始

### 非流式 (简单调用)

```typescript
import { ClaudeCode } from "claude-code-rs-sdk";

const client = new ClaudeCode();
const session = client.startSession({
  permissionMode: "auto",
  model: "claude-sonnet-4-20250514",
});

const turn = await session.run("当前目录有哪些文件？");

console.log(turn.finalResponse);
console.log("Token 用量:", turn.usage);
```

### 流式 (逐事件处理)

```typescript
import { ClaudeCode } from "claude-code-rs-sdk";

const client = new ClaudeCode();
const session = client.startSession({ permissionMode: "auto" });

const { events } = await session.runStreamed("帮我分析 src/main.rs 的架构");

for await (const event of events) {
  switch (event.type) {
    case "session.started":
      console.log(`会话 ${event.session_id} 已启动`);
      break;
    case "stream.delta":
      // 实时输出文本增量
      if (event.event_type === "content_block_delta") {
        const delta = event.delta as { text?: string };
        if (delta.text) process.stdout.write(delta.text);
      }
      break;
    case "item.completed":
      if (event.item.type === "tool_use_summary") {
        console.log(`\n[工具] ${event.item.summary}`);
      }
      break;
    case "turn.completed":
      console.log(`\n完成: $${event.usage.total_cost_usd.toFixed(4)}`);
      break;
    case "turn.failed":
      console.error(`错误: ${event.error.message}`);
      break;
  }
}
```

### 恢复会话

```typescript
const session = client.resumeSession("之前的session-id");
const turn = await session.run("继续上次的工作");
```

### 指定二进制路径

```typescript
const client = new ClaudeCode({
  executablePath: "/path/to/claude-code-rs",
  apiKey: "sk-ant-...",
});
```

SDK 按以下顺序查找二进制:
1. `CLAUDE_CODE_RS_PATH` 环境变量
2. 系统 `PATH` 中的 `claude-code-rs`
3. 相对路径 `../../target/release/claude-code-rs`

---

## API 参考

### `ClaudeCode` — 客户端

```typescript
class ClaudeCode {
  constructor(options?: ClaudeCodeOptions)
  startSession(options?: SessionOptions): Session
  resumeSession(sessionId: string, options?: SessionOptions): Session
}
```

| ClaudeCodeOptions | 类型 | 说明 |
|---|---|---|
| `executablePath` | `string?` | 二进制路径 (自动检测) |
| `apiKey` | `string?` | API 密钥 (设为 `ANTHROPIC_API_KEY` 环境变量) |
| `env` | `Record<string, string>?` | 传递给子进程的环境变量 |

### `Session` — 会话

```typescript
class Session {
  get sessionId(): string | null

  // 缓冲模式: 等待整个 turn 完成后返回
  async run(input: string, turnOptions?: TurnOptions): Promise<Turn>

  // 流式模式: 返回 AsyncGenerator
  async runStreamed(input: string, turnOptions?: TurnOptions): Promise<StreamedTurn>
}
```

| SessionOptions | 类型 | 对应 CLI 参数 |
|---|---|---|
| `model` | `string?` | `--model` |
| `workingDirectory` | `string?` | `--cwd` |
| `permissionMode` | `PermissionMode?` | `--permission-mode` |
| `maxTurns` | `number?` | `--max-turns` |
| `maxBudget` | `number?` | `--max-budget` |
| `systemPrompt` | `string?` | `--system-prompt` |
| `appendSystemPrompt` | `string?` | `--append-system-prompt` |
| `verbose` | `boolean?` | `--verbose` |
| `continueSession` | `string?` | `--continue` |

| TurnOptions | 类型 | 说明 |
|---|---|---|
| `signal` | `AbortSignal?` | 取消信号 |

### 返回类型

```typescript
type Turn = {
  items: SessionItem[];      // 本次 turn 的所有项
  finalResponse: string;     // 最后一条 agent_message 的文本
  usage: Usage | null;       // Token 用量和费用
}

type StreamedTurn = {
  events: AsyncGenerator<SessionEvent>;  // 事件流
}
```

---

## 事件类型

SDK 将 Rust CLI 的原始 JSONL 事件转换为规范化的 `SessionEvent` 联合类型:

| SDK 事件 | 触发时机 | Rust SdkMessage 来源 |
|---|---|---|
| `session.started` | 会话初始化完成 | `SystemInit` |
| `turn.started` | (预留) | — |
| `turn.completed` | Turn 成功结束 | `Result` (is_error=false) |
| `turn.failed` | Turn 失败 | `Result` (is_error=true) |
| `item.completed` | 内容项完成 | `Assistant` / `ToolUseSummary` / `CompactBoundary` / `UserReplay` |
| `stream.delta` | 实时流式增量 | `StreamEvent` |
| `error` | 可重试错误 (如限流) | `ApiRetry` |

### Item 类型

| 类型 | 说明 | 关键字段 |
|---|---|---|
| `agent_message` | 助手回复 | `text`, `content_blocks`, `usage`, `cost_usd` |
| `tool_use_summary` | 工具执行摘要 | `summary`, `preceding_tool_use_ids` |
| `compact_boundary` | 上下文压缩边界 | `pre_compact_token_count`, `post_compact_token_count` |
| `user_replay` | 用户消息重放 | `content`, `is_replay`, `is_synthetic` |
| `error` | 错误 | `message` |

### ContentBlock 类型

与 Anthropic API 一致:

| 类型 | 说明 |
|---|---|
| `text` | 文本内容 |
| `tool_use` | 工具调用 (id, name, input) |
| `tool_result` | 工具结果 |
| `thinking` | 思维链 (Extended Thinking) |
| `redacted_thinking` | 已编辑的思维链 |
| `image` | 图片 (base64) |

---

## 项目结构

```
rust/sdk/typescript/
├── package.json              包配置 (ESM, Node 18+)
├── tsconfig.json             TypeScript 严格模式配置
├── tsup.config.ts            构建配置 (ESM-only, dts, sourcemap)
├── jest.config.cjs           Jest 测试配置
│
���── src/
│   ├── index.ts              公共 API 导出
│   ├── claudeCode.ts         ClaudeCode 客户端类
│   ├── session.ts            Session 会话类 (run / runStreamed)
│   ├── exec.ts               ClaudeCodeExec 进程管理 (spawn + readline)
│   ├── transform.ts          原始 JSONL → SessionEvent 转换层
│   ├── events.ts             事件类型定义 (9 种事件)
│   ├── items.ts              内容项类型 (5 种 + ContentBlock)
│   ├── claudeCodeOptions.ts  客户端选项
│   ├── sessionOptions.ts     会话选项 (映射 CLI 参数)
│   └── turnOptions.ts        Turn 选项 (AbortSignal)
│
├── tests/
│   ├── helpers.ts            测试辅助 (二进制路径)
│   ├── mockProcess.ts        模拟子进程 + 9 个样本 JSONL 载荷
│   └── transform.test.ts     转换层单元测试 (10 个用例)
│
└── samples/
    ├── simple_run.ts         非流式单轮示例
    └── basic_streaming.ts    交互式流式示例
```

### 层次架构

```
Public API     │  ClaudeCode, Session, 所有类型导出
───────────────┼──────────────────────────────────
Execution      │  ClaudeCodeExec (spawn, readline, signal)
───────────────┼──────────────────────────────────
Transform      │  transformRawEvent() — 原始 JSONL → 规范事件
───────────────┼──────────────────────────────────
Types          │  events.ts, items.ts, *Options.ts
```

---

## Rust 侧变更说明

本次更新同时修改了 Rust 代码以支持 JSONL 输出。

### 修改的文件 (5 个)

| 文件 | 变更内容 |
|------|---------|
| `rust/Cargo.toml` | uuid 添加 `serde` feature |
| `rust/src/engine/sdk_types.rs` | `SdkMessage` 枚举及全部 8 个内部结构体添加 `Serialize` derive + `#[serde(tag = "type", rename_all = "snake_case")]` |
| `rust/src/types/message.rs` | `AssistantMessage`, `StreamEvent`, `CompactMetadata` 添加 `Serialize` |
| `rust/src/engine/lifecycle.rs` | `UsageTracking`, `PermissionDenial` 添加 `Serialize` |
| `rust/src/main.rs` | 新增 `run_json_mode()` 函数，接入 `--output-format json` 标志 |

### serde 序列化策略

```rust
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SdkMessage {
    SystemInit(SystemInitMessage),   // → {"type": "system_init", ...}
    Assistant(SdkAssistantMessage),  // → {"type": "assistant", ...}
    StreamEvent(SdkStreamEvent),     // → {"type": "stream_event", ...}
    Result(SdkResult),               // → {"type": "result", ...}
    // ...
}
```

使用 serde 内部标签 (`#[serde(tag = "type")]`)，将枚举变体名序列化为 `type` 字段。内部结构体的所有字段被展平到同一层级的 JSON 对象中。

### JSON 输出模式

```rust
// rust/src/main.rs
async fn run_json_mode(engine: &QueryEngine, prompt: &str) -> anyhow::Result<ExitCode> {
    let stream = engine.submit_message(prompt, QuerySource::Sdk);
    while let Some(msg) = stream.next().await {
        let json = serde_json::to_string(&msg)?;
        println!("{}", json);  // 每行一个 JSON 对象 (JSONL)
    }
    Ok(exit_code)
}
```

触发方式:
```bash
# 通过参数传入 prompt
claude-code-rs --output-format json -p "你好"

# 通过 stdin 传入 prompt (SDK 使用此方式)
echo "你好" | claude-code-rs --output-format json -p
```

---

## JSONL 协议

每行是一个 JSON 对象，包含 `type` 字段标识消息类型。一个完整的 turn 输出示例:

```jsonl
{"type":"system_init","tools":["Bash","Read","Write"],"model":"claude-sonnet-4-20250514","permission_mode":"default","session_id":"abc-123","uuid":"..."}
{"type":"stream_event","event":{"type":"message_start","usage":{"input_tokens":100,"output_tokens":0,...}},"session_id":"abc-123","uuid":"..."}
{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},...}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}},...}
{"type":"stream_event","event":{"type":"content_block_stop","index":0},...}
{"type":"stream_event","event":{"type":"message_stop"},...}
{"type":"assistant","message":{"uuid":"...","content":[{"type":"text","text":"Hello! ..."}],"usage":{...},"cost_usd":0.001},"session_id":"abc-123"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":5000,"num_turns":1,"result":"Hello! ...","usage":{...},"total_cost_usd":0.001,...}
```

每个 `submit_message()` 调用 **始终以一个 `result` 消息结束**。

---

## 测试

```bash
# 运行所有测试
npm test

# 监听模式
npm run test:watch

# 覆盖率
npm run coverage
```

### 测试结构

- **`transform.test.ts`** — 10 个用例，覆盖所有 9 种 SdkMessage 变体的转换:
  - `system_init` → `session.started`
  - `assistant` → `item.completed` (agent_message)
  - `stream_event` → `stream.delta`
  - `tool_use_summary` → `item.completed`
  - `api_retry` → `error` (retryable)
  - `result` (success) → `turn.completed`
  - `result` (error) → `turn.failed`
  - `compact_boundary` → `item.completed`
  - `user_replay` → `item.completed`
  - unknown type → empty array

---

## 设计决策

### 为什么封装 CLI 而不是直接调用 API？

与 Codex SDK 一致的设计: CLI 已实现完整的工具执行、权限管理、会话持久化、上下文压缩等逻辑。SDK 只需关注进程通信和类型安全。

### 为什么用 JSONL 而不是其他协议？

- 每行一个 JSON 对象，解析简单 (`readline` + `JSON.parse`)
- 天然支持流式 (逐行 yield)
- 与 Codex SDK 的 `--experimental-json` 模式一致
- 不需要额外的 IPC 机制

### 事件规范化

SDK **不直接暴露** Rust 的 `SdkMessage` 结构，而是通过 `transform.ts` 转换为语义化的事件模型 (`session.started`, `turn.completed` 等)。这样:
- 消费者不需要了解 Rust 内部类型
- 事件名称自解释
- 未来可以在不改变公共 API 的情况下调整 Rust 序列化

### 二进制发现

SDK 不打包 Rust 二进制 (不同于 Codex SDK 通过 npm optional dependencies 分发)。用户需要自行构建或提供二进制路径。未来可以添加 npm 平台包分发。
