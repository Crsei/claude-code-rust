# ink-terminal 前端架构文档

> 版本: 0.1.0 | 创建日期: 2026-04-07

## 概述

cc-rust 采用 **IPC 分层架构**，将 UI 与核心逻辑完全解耦：

- **Rust 后端** (`src/ipc/`) — headless 模式，通过 stdin/stdout JSONL 通信
- **ink-terminal 前端** (`ui/`) — React 19 终端 UI，负责所有渲染和用户交互

这种架构替代了原有的 ratatui + crossterm 单体 TUI（`src/ui/`，~4,300 行），新前端代码量约 ~1,700 行。

## 架构图

```
┌──────────────────────────────────────────────────────┐
│  ink-terminal Frontend (TypeScript/React)             │
│                                                       │
│  ui/src/main.tsx ─→ spawn Rust --headless             │
│       ↓                                               │
│  RustBackend (ipc/client.ts)                          │
│       ↓  JSON lines over stdin/stdout                 │
│  BackendProvider (ipc/context.tsx)                     │
│       ↓                                               │
│  AppStateProvider (store/app-store.tsx)                │
│       ↓                                               │
│  <App> ─→ <AlternateScreen>                           │
│    ├── <Header>            模型名 + Session ID        │
│    ├── <MessageList>       ScrollBox + VirtualList    │
│    │    └── <MessageBubble>                           │
│    │         ├── <Markdown streaming>  助手消息       │
│    │         ├── <ToolUseBlock>        工具调用       │
│    │         ├── <ToolResultBlock>     工具结果       │
│    │         └── <ThinkingBlock>       思考块         │
│    ├── <Suggestions>       提示建议                   │
│    ├── <InputPrompt>       输入框 + Vim 模式          │
│    ├── <StatusBar>         状态栏 + Token 用量        │
│    └── <PermissionDialog>  权限覆盖层 (absolute)      │
└──────────────────────────────────────────────────────┘
               │ stdin/stdout (JSONL)
┌──────────────▼──────────────────────────────────────┐
│  Rust Backend (--headless mode)                      │
│                                                      │
│  ipc/headless.rs ─→ tokio event loop                │
│       ↓                                              │
│  FrontendMessage (stdin) ──→ QueryEngine             │
│       ↓                                              │
│  SdkMessage ──→ BackendMessage (stdout)              │
│                                                      │
│  核心模块不变:                                        │
│  engine/ query/ tools/ api/ auth/ permissions/       │
│  commands/ skills/ session/ config/ compact/          │
└──────────────────────────────────────────────────────┘
```

## IPC 协议

### 通信方式

- **传输层**: stdin/stdout，每行一个 JSON 对象 (JSONL)
- **方向**: 双向 — Frontend 写 stdin，读 stdout；Backend 读 stdin，写 stdout
- **编码**: UTF-8 JSON，`#[serde(tag = "type", rename_all = "snake_case")]`

### Frontend → Backend (`FrontendMessage`)

| type | 字段 | 说明 |
|------|------|------|
| `submit_prompt` | `text`, `id` | 提交用户输入 |
| `abort_query` | — | 中断当前查询 |
| `permission_response` | `tool_use_id`, `decision` | 权限决策 (allow/deny/always_allow) |
| `slash_command` | `raw` | 斜杠命令 |
| `resize` | `cols`, `rows` | 终端尺寸变化 |
| `quit` | — | 退出 |

### Backend → Frontend (`BackendMessage`)

| type | 字段 | 说明 |
|------|------|------|
| `ready` | `session_id`, `model`, `cwd` | 后端就绪 |
| `stream_start` | `message_id` | 流式开始 |
| `stream_delta` | `message_id`, `text` | 流式文本增量 |
| `stream_end` | `message_id` | 流式结束 |
| `assistant_message` | `id`, `content`, `cost_usd` | 最终助手消息 |
| `tool_use` | `id`, `name`, `input` | 工具调用 |
| `tool_result` | `tool_use_id`, `output`, `is_error` | 工具结果 |
| `permission_request` | `tool_use_id`, `tool`, `command`, `options` | 请求权限 |
| `system_info` | `text`, `level` | 系统信息 (info/warning/error) |
| `usage_update` | `input_tokens`, `output_tokens`, `cost_usd` | Token 用量 |
| `suggestions` | `items` | 提示建议 |
| `error` | `message`, `recoverable` | 错误 |

### SdkMessage → BackendMessage 映射

```
SdkMessage::StreamEvent(ContentBlockStart)  → stream_start
SdkMessage::StreamEvent(ContentBlockDelta)  → stream_delta (提取 text)
SdkMessage::StreamEvent(MessageStop)        → stream_end
SdkMessage::Assistant                       → assistant_message
SdkMessage::Result                          → usage_update + 可能的 error
```

## 前端目录结构

```
ui/
├── package.json              项目配置
├── tsconfig.json             TypeScript 配置
├── run.sh                    启动脚本
├── src/
│   ├── main.tsx              入口: spawn 后端 + render React
│   ├── theme.ts              颜色常量
│   ├── utils.ts              工具函数 (uid, formatCost, formatTokens)
│   ├── ipc/
│   │   ├── protocol.ts       IPC 类型定义 (与 Rust 端一致)
│   │   ├── client.ts         RustBackend 类 (spawn + JSONL 读写)
│   │   └── context.tsx       React Context: useBackend()
│   ├── store/
│   │   └── app-store.tsx     状态管理 (useReducer + Context)
│   ├── vim/
│   │   ├── types.ts          Vim 类型定义
│   │   ├── motions.ts        文本导航 (w/b/e/0/$/^)
│   │   ├── state-machine.ts  Vim 状态机 (Normal/Insert/Visual)
│   │   └── index.ts          导出
│   └── components/
│       ├── App.tsx            顶层布局 + 消息分发
│       ├── Header.tsx         顶部栏
│       ├── MessageList.tsx    消息列表 (ScrollBox + VirtualList)
│       ├── MessageBubble.tsx  消息渲染分发
│       ├── InputPrompt.tsx    输入框 (Vim 集成)
│       ├── StatusBar.tsx      底部状态栏
│       ├── WelcomeScreen.tsx  欢迎页
│       ├── Spinner.tsx        加载动画
│       ├── PermissionDialog.tsx  权限弹窗 (absolute 覆盖)
│       ├── Suggestions.tsx    提示建议
│       ├── ToolUseBlock.tsx   工具调用展示
│       ├── ToolResultBlock.tsx 工具结果展示
│       ├── ThinkingBlock.tsx  思考块 (可折叠)
│       └── DiffView.tsx       Diff 色彩渲染
```

## 状态管理

使用 React `useReducer` + Context，单一状态树：

```typescript
interface AppState {
  messages: UIMessage[]          // 消息列表
  streamingText: string          // 当前流式文本
  streamingMessageId: string | null
  isStreaming: boolean           // 是否正在流式输出
  model: string                  // 模型名
  sessionId: string              // 会话 ID
  cwd: string                    // 工作目录
  usage: Usage                   // Token 用量
  permissionRequest: PermissionRequest | null
  suggestions: string[]          // 提示建议
  inputHistory: string[]         // 输入历史
  historyIndex: number           // 历史导航位置
  vimEnabled: boolean            // Vim 模式开关
  vimMode: string                // NORMAL/INSERT/VISUAL
}
```

数据流：
```
BackendMessage → App.tsx useEffect handler → dispatch(AppAction) → appReducer → new state → re-render
```

## Vim 模式

从 Rust `ui/vim.rs` (847 行) 完整迁移为 TypeScript (~400 行)。

### 支持的操作

| 类别 | 命令 |
|------|------|
| 模式切换 | `i`, `a`, `I`, `A`, `v`, `Esc` |
| 导航 | `h/l`, `w/b/e`, `0/$`, `^` |
| 操作符 | `d`, `y`, `c` + motion (`dw`, `cw`, `yy`, `dd`, `cc`) |
| 单键 | `x`, `X`, `p`, `u`, `D`, `C` |
| 重复 | `3w`, `2dd` 等数字前缀 |
| Visual | `v` 进入，`d/y/c/x` 操作选区 |

切换方式: `Ctrl+G` 开关 Vim 模式

## ink-terminal 关键依赖

| 组件 | 用途 | 替代的 Rust 代码 |
|------|------|----------------|
| `<ScrollBox stickyScroll>` | 自动滚动容器 | — |
| `<VirtualList>` | 虚拟列表 | `virtual_scroll.rs` (139 行) |
| `<Markdown streaming>` | 流式 Markdown 渲染 | `markdown.rs` (271 行) |
| `useAnimationFrame` | 动画帧 | `spinner.rs` tick 逻辑 |
| `useInput` | 键盘事件 | crossterm event loop |
| `<AlternateScreen>` | 全屏模式 | crossterm AlternateScreen |
| `<Button>` | 交互按钮 | — |

## 与原有 ratatui UI 的关系

| 方面 | ratatui UI (`src/ui/`) | ink-terminal UI (`ui/`) |
|------|----------------------|----------------------|
| 状态 | **保留** (作为 fallback) | **新增** (主 UI) |
| 启动方式 | `cargo run` (默认) | `bun run ui/src/main.tsx` 或 `ui/run.sh` |
| 后端模式 | 直接调用 QueryEngine | `--headless` + IPC |
| 代码量 | ~4,300 行 (14 文件) | ~1,700 行 (25 文件) |
| 布局 | 手动 Rect 计算 | Flexbox (Yoga) |
| 渲染 | 命令式 draw() | 声明式 JSX |

未来可在确认新 UI 稳定后移除 `src/ui/` 和 ratatui/crossterm 依赖。
