# Headless IPC Protocol

`--headless` 模式下 Rust 后端通过 stdin/stdout 以 JSONL（每行一个 JSON 对象）与外部 UI 进程通信。

## 启动流程

```
run.ps1 / run.sh
  → bun run src/main.tsx @args
    → RustBackend(binaryPath, ['--headless', ...extraArgs])
      → spawn(binary, ['--headless', ...], stdio: [pipe, pipe, inherit])
```

- stdin: pipe (前端 → 后端)
- stdout: pipe (后端 → 前端, JSONL)
- stderr: inherit (tracing 日志直接输出到终端)

## 协议类型

### Frontend → Backend (`FrontendMessage`)

定义: `src/ipc/protocol.rs`, `ui/src/ipc/protocol.ts`

| type | 字段 | 说明 |
|------|------|------|
| `submit_prompt` | `text`, `id` | 用户提交提示词 |
| `abort_query` | — | 中断当前流式响应 |
| `permission_response` | `tool_use_id`, `decision` | 用户权限决策 (`allow`/`deny`/`always_allow`) |
| `slash_command` | `raw` | 斜杠命令（如 `/help`） |
| `resize` | `cols`, `rows` | 终端尺寸变更 |
| `quit` | — | 退出 |

### Backend → Frontend (`BackendMessage`)

定义: `src/ipc/protocol.rs`, `ui/src/ipc/protocol.ts`

| type | 字段 | 说明 |
|------|------|------|
| `ready` | `session_id`, `model`, `cwd` | 后端初始化完成 |
| `stream_start` | `message_id` | 开始流式输出 |
| `stream_delta` | `message_id`, `text` | 流式文本增量 |
| `stream_end` | `message_id` | 流式输出结束 |
| `assistant_message` | `id`, `content`, `cost_usd` | 完整助手消息（含 tool_use blocks） |
| `tool_use` | `id`, `name`, `input` | 单个工具调用（从 assistant content 提取） |
| `tool_result` | `tool_use_id`, `output`, `is_error` | 工具执行结果 |
| `permission_request` | `tool_use_id`, `tool`, `command`, `options` | 权限请求对话框 |
| `system_info` | `text`, `level` | 系统信息 (`info`/`warning`/`error`) |
| `usage_update` | `input_tokens`, `output_tokens`, `cost_usd` | token 用量更新 |
| `suggestions` | `items` | 提示建议（预留，未启用） |
| `error` | `message`, `recoverable` | 错误 |

## SdkMessage → BackendMessage 映射

`submit_message()` 产生的 `SdkMessage` 在 `headless.rs::handle_sdk_message()` 中映射：

| SdkMessage | BackendMessage | 说明 |
|------------|----------------|------|
| `SystemInit` | `system_info` (info) | 会话初始化：model、permission_mode、tools 数量 |
| `StreamEvent::MessageStart` | `stream_start` | 流式开始 |
| `StreamEvent::ContentBlockDelta` | `stream_delta` | 文本增量 |
| `StreamEvent::MessageStop` | `stream_end` | 流式结束 |
| `Assistant` | `tool_use` × N + `assistant_message` | 先发每个 ToolUse block，再发完整消息 |
| `UserReplay` | _(logged)_ | 工具结果回放，当前仅记录日志 |
| `CompactBoundary` | `system_info` (info) | 上下文压缩：压缩前后 token 数 |
| `ApiRetry` | `error` (recoverable) | API 重试：次数、错误、延迟 |
| `ToolUseSummary` | `system_info` (info) | 工具使用摘要文本 |
| `Result` | `stream_end` + `usage_update` [+ `error`] | 每轮结束，附带 token 统计 |

## 权限流程

```
Model 请求工具 → deps.execute_tool()
  → tool.check_permissions()
    ├─ Allow → 直接执行
    ├─ Deny  → 返回错误（不经过前端）
    └─ Ask   → permission_callback 存在？
                ├─ 是 → send PermissionRequest → await oneshot
                │       → 前端 PermissionDialog 渲染
                │       → 用户按 y/n/a
                │       → send PermissionResponse → headless 主循环
                │       → oneshot 回传 decision → callback 返回
                └─ 否 → 拒绝（非交互模式兜底）
```

实现位置：
- 回调类型: `src/types/tool.rs` — `PermissionCallback`
- 回调注册: `src/engine/lifecycle/mod.rs` — `set_permission_callback()`
- 权限检查: `src/engine/lifecycle/deps.rs` — `execute_tool()` Stage: Permission check
- IPC 桥接: `src/ipc/headless.rs` — `PendingPermissions` + oneshot channel
- 前端组件: `ui/src/components/PermissionDialog.tsx`

## 典型消息序列

### 纯文本问答

```jsonl
→ {"type":"submit_prompt","text":"What is 2+2?","id":"001"}
← {"type":"system_info","text":"Session ... initialized","level":"info"}
← {"type":"stream_start","message_id":"001"}
← {"type":"stream_delta","message_id":"001","text":"4"}
← {"type":"stream_end","message_id":"001"}
← {"type":"assistant_message","id":"uuid","content":[{"type":"text","text":"4"}],"cost_usd":0.001}
← {"type":"stream_end","message_id":"001"}
← {"type":"usage_update","input_tokens":100,"output_tokens":5,"cost_usd":0.001}
```

### 工具调用（bypass 模式）

```jsonl
→ {"type":"submit_prompt","text":"Read test.txt","id":"002"}
← {"type":"stream_start","message_id":"002"}
← {"type":"stream_delta","message_id":"002","text":"..."}
← {"type":"stream_end","message_id":"002"}
← {"type":"tool_use","id":"tu_1","name":"Read","input":{"file_path":"test.txt"}}
← {"type":"assistant_message","id":"uuid","content":[...],"cost_usd":0.001}
← {"type":"system_info","text":"[tool summary] Read test.txt ...","level":"info"}
... (next turn with tool result → assistant response)
← {"type":"stream_end","message_id":"002"}
← {"type":"usage_update","input_tokens":500,"output_tokens":50,"cost_usd":0.005}
```

### 工具调用（default 模式，需要权限）

```jsonl
→ {"type":"submit_prompt","text":"Run echo hello","id":"003"}
← {"type":"tool_use","id":"tu_2","name":"Bash","input":{"command":"echo hello"}}
← {"type":"permission_request","tool_use_id":"tu_2","tool":"Bash","command":"Bash: ...","options":["Allow","Deny","Always Allow"]}
→ {"type":"permission_response","tool_use_id":"tu_2","decision":"allow"}
... (tool executes, response continues)
← {"type":"usage_update","input_tokens":600,"output_tokens":30,"cost_usd":0.004}
```

## 大文件截断

当工具结果超过 100,000 字符时，`compact/tool_result_budget.rs` 自动截断：

- 保留前 500 字符 + 后 200 字符
- 完整内容保存到 `$TEMP/claude-code-rs/tool-results/{tool_use_id}.txt`
- 截断标记: `[... {N} characters omitted. Full output saved to: {path} ...]`

## 相关文件

| 文件 | 说明 |
|------|------|
| `src/ipc/protocol.rs` | Rust 端协议类型 (`FrontendMessage`, `BackendMessage`) |
| `src/ipc/headless.rs` | Headless 事件循环 + SdkMessage 映射 + 权限桥接 |
| `ui/src/ipc/protocol.ts` | TypeScript 端协议类型 |
| `ui/src/ipc/client.ts` | `RustBackend` 类：spawn binary + JSONL 解析 |
| `ui/src/components/App.tsx` | 消息分发到 store |
| `ui/src/components/PermissionDialog.tsx` | 权限对话框 UI |
| `ui/src/store/app-store.tsx` | 状态管理 (permissionRequest) |
| `tests/e2e_terminal/` | 42 个 E2E 测试 (26 offline + 16 live) |
