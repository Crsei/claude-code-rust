# Subagent 测试与可视化仪表盘设计

日期: 2026-04-13
状态: 已批准

## 概述

通过三个互补手段全面测试和观测 cc-rust 的 subagent (Agent tool) 运行情况：

1. **PTY 自动化测试** — headless JSONL 协议驱动，验证 subagent 端到端正确性
2. **Chrome DevTools 仪表盘** — 浏览器实时展示 subagent 事件流，伴随 cc-rust 启动
3. **cc-rust 日志消费** — 读取 `F:\temp\.logs` 中的 tracing 输出作为事件源

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    cc-rust 进程                          │
│                                                         │
│  QueryEngine (parent)                                   │
│    ├─ Agent Tool → spawn child QueryEngine (subagent)   │
│    │    └─ tracing::debug!/info! → .logs/cc-rust.log    │
│    └─ tracing::debug!/info! → .logs/cc-rust.log         │
│                                                         │
│  启动时: spawn companion → open browser                  │
│  退出时: kill companion                                  │
└────────────────┬────────────────────────────────────────┘
                 │ fs.watch()
                 ▼
┌─────────────────────────────────────────────────────────┐
│          Bun Companion (ui/subagent-dashboard/)          │
│                                                         │
│  log-watcher.ts                                         │
│    └─ fs.watch(F:\temp\.logs\cc-rust.log)               │
│    └─ 逐行解析 tracing 输出 → 过滤 subagent 事件         │
│                                                         │
│  server.ts (Bun.serve :19838)                           │
│    ├─ GET /           → dashboard.html                  │
│    ├─ GET /events     → SSE stream (subagent 事件)       │
│    └─ GET /api/state  → 当前 subagent 快照 (JSON)        │
│                                                         │
│  dashboard.html (单页)                                   │
│    ├─ EventSource(/events) 实时渲染                      │
│    ├─ 时间线视图: subagent 生命周期                       │
│    ├─ 事件流视图: 滚动日志                                │
│    └─ 状态面板: 活跃 subagent 列表                        │
└─────────────────────────────────────────────────────────┘
                 │
                 ▼ Chrome DevTools MCP
┌─────────────────────────────────────────────────────────┐
│          自动化验证 (e2e 测试阶段)                        │
│                                                         │
│  take_screenshot → 仪表盘渲染正确                        │
│  evaluate_script → 检查 DOM 状态 / 事件计数              │
│  wait_for → 等待特定 subagent 事件出现                    │
└─────────────────────────────────────────────────────────┘
```

### 关键设计决策

- 日志目录可配：默认 `F:\temp\.logs`，通过 `--log-dir` 或 `CC_RUST_LOG_DIR` 环境变量覆盖
- 端口 19838（紧跟 daemon 的 19836、team-memory 的 19837）
- Companion 仅在 `FEATURE_SUBAGENT_DASHBOARD=1` 时启动，`--headless-only` 时跳过

## 2. PTY 自动化测试

### 测试文件

`tests/e2e_subagent.rs`（独立文件）

### 测试方式

通过 `--headless` 模式 + JSONL 协议驱动。subagent 行为是后端逻辑，JSONL 断言比 ANSI 解析稳定。PTY 渲染测试留给手动探索阶段。

### API Key

从项目根目录 (`F:\AIclassmanager\cc\rust\.env`) 加载，`HeadlessHarness::spawn()` 解析 `.env` 文件并注入为子进程环境变量（使用 `dotenvy` crate）。如果 `.env` 中无有效 `ANTHROPIC_API_KEY`，测试标记 `#[ignore]` 并输出 "skipped: no API key in .env"。

### 测试用例矩阵

| # | 用例 | 输入 | 断言 |
|---|------|------|------|
| 1 | 同步 subagent 成功 | `SubmitPrompt` 含 Agent tool call | 收到 `ToolUse{name:"Agent"}` → `ToolResult{is_error:false}` |
| 2 | 同步 subagent 返回结果 | 简单任务 prompt | `ToolResult.output` 含预期内容 |
| 3 | 背景 subagent 完成通知 | `run_in_background: true` | 收到 `BackgroundAgentComplete{had_error:false}` |
| 4 | 递归深度限制 | 嵌套 agent 超过 5 层 | `ToolResult{is_error:true}` 含 "maximum depth" |
| 5 | subagent abort | 发送 prompt → 立即 `AbortQuery` | 不 hang，收到 `StreamEnd` 或 `Error` |
| 6 | worktree 隔离 | `isolation: "worktree"` | `ToolResult.output` 含 worktree 路径信息 |

### 测试 Harness

```rust
struct HeadlessHarness {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl HeadlessHarness {
    fn spawn(cwd: &str) -> Self;
    fn send(&mut self, msg: &FrontendMessage);
    fn recv(&mut self) -> BackendMessage;
    fn recv_until(&mut self, pred: fn(&BackendMessage) -> bool,
                  timeout: Duration) -> Vec<BackendMessage>;
    fn wait_ready(&mut self) -> BackendMessage;
}
```

### 日志验证

每个测试结束后读取 `.logs/cc-rust.log`，grep 确认关键 tracing 事件存在（如 `agent_id=xxx`）。

## 3. 日志解析与事件提取

### 日志来源

`F:\temp\.logs\cc-rust.log` — tracing daily rolling，debug 级别，含 target + line number。

### 解析策略 (`log-watcher.ts`)

```typescript
const LOG_PATTERN = /^(\S+)\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+(\S+):(\d+)\s+(.+)$/;
const KV_PATTERN  = /(\w+)=([\w\-."]+)/g;

const SUBAGENT_MARKERS = [
  "agent_id=",
  "subagent",
  "child QueryEngine",
  "background_agent",
  "worktree",
  "agent::dispatch",
  "agent::tool_impl",
];
```

### 提取事件类型

```typescript
type SubagentEvent = {
  timestamp: string;
  level: "DEBUG" | "INFO" | "WARN" | "ERROR";
  source: string;
  line: number;
  agent_id?: string;
  event_type: "spawn" | "stream" | "tool_call" | "complete" | "error" | "other";
  raw: string;
  fields: Record<string, string>;
};
```

### 文件监听

- `fs.watch()` 监听文件变化，维护读取 offset
- 增量读取新追加内容（`fs.read()` from last offset）
- 日志 rotation 处理：检测文件 size 缩小时重置 offset
- 启动时回溯最近 200 行作为初始状态

## 4. 仪表盘 UI

### 三栏布局

```
┌──────────────────────────────────────────────────────────┐
│  cc-rust Subagent Dashboard           ● Connected  00:42 │
├──────────────┬───────────────────────────────────────────┤
│              │                                           │
│  Active      │  Timeline                                 │
│  Agents      │                                           │
│              │  agent-a1b2 ████████████░░░ 3.8s          │
│  ● a1b2c3   │    ├─ spawn (sonnet)                      │
│    sonnet    │    ├─ Bash: ls -la                        │
│    3.8s      │    ├─ Read: src/main.rs                   │
│              │    └─ ✓ complete                           │
│  ◐ d4e5f6   │                                           │
│    opus      │  agent-d4e5 ████░░░░░░░░░░ running...     │
│    running   │    ├─ spawn (opus, background)            │
│              │    └─ Glob: **/*.rs                        │
│              │                                           │
├──────────────┴───────────────────────────────────────────┤
│  Event Stream (newest first)                        ▼ ◉  │
│                                                          │
│  22:35:43.000 INFO  agent-a1b2 complete duration=3800ms  │
│  22:35:41.500 DEBUG agent-a1b2 stream delta tokens=42    │
│  22:35:39.200 INFO  agent-a1b2 spawn model=sonnet bg=no  │
│  22:35:39.100 INFO  agent-d4e5 spawn model=opus bg=yes   │
└──────────────────────────────────────────────────────────┘
```

| 面板 | 内容 | 更新方式 |
|------|------|----------|
| Active Agents（左侧） | 活跃 subagent 列表，ID 前缀、model、耗时 | SSE 驱动，spawn 新增，complete 标记 |
| Timeline（右上） | 甘特条 + 子事件节点 | spawn 创建，事件追加，complete 封闭 |
| Event Stream（底部） | 全量日志流，最新在上 | SSE 逐条追加，`◉` 切换自动滚动 |

### 技术实现

- 纯 HTML + CSS + vanilla JS，无框架
- SSE：`new EventSource("/events")`
- 状态：`Map<agent_id, AgentState>`
- 暗色主题，甘特条用 CSS `linear-gradient` + `animation`

### SSE 事件格式

```
event: subagent
data: {"timestamp":"...","agent_id":"a1b2","event_type":"spawn","fields":{"model":"sonnet"}}

event: ping
data: {}
```

## 5. Chrome DevTools MCP 自动化验证

### 测试流程

```
1. 启动 companion dashboard (Bun, :19838)
2. 启动 cc-rust --headless，发送触发 subagent 的 prompt
3. Chrome DevTools MCP:
   ├─ new_page → navigate_page(localhost:19838)
   ├─ wait_for(".agent-card")
   ├─ evaluate_script → 断言 DOM 状态
   ├─ take_screenshot → logs/screenshots/
   └─ close_page
4. 停止 cc-rust + companion
```

### 验证点矩阵

| # | 验证内容 | DevTools 操作 | 断言 |
|---|----------|---------------|------|
| 1 | SSE 连接建立 | `evaluate_script` 查询 `.status` 文本 | 含 "Connected" |
| 2 | Agent 卡片渲染 | `wait_for(".agent-card")` + 计数 | >= 1 |
| 3 | 时间线条目 | `evaluate_script` 查询 `.timeline-bar` | >= 1 |
| 4 | 事件流行数 | `evaluate_script` 查询 `.event-row` | >= 3 |
| 5 | 完成状态标记 | `wait_for(".agent-card.completed")` | 元素存在 |
| 6 | 截图存档 | `take_screenshot` | 文件写入 |

### 执行方式

独立脚本 `tests/dashboard_verify.ts`（Bun），不嵌入 Rust 测试。Chrome DevTools MCP 是会话级工具，手动探索阶段由 Claude Code 驱动，后续可固化为 CI。

## 6. 生命周期管理

### Companion 进程管理

新增 `src/dashboard.rs`（~80 行）：

```rust
pub struct DashboardCompanion {
    child: std::process::Child,
}

impl DashboardCompanion {
    pub fn spawn(config: DashboardConfig) -> Result<Self>;
    pub fn kill(&mut self);
}

impl Drop for DashboardCompanion {
    fn drop(&mut self) { self.kill(); }
}
```

### 启动条件

- `FEATURE_SUBAGENT_DASHBOARD=1` 环境变量（默认关闭）
- `--headless-only` 时不启动
- `bun` 不在 PATH 时 warn 跳过，不阻塞 cc-rust

### 关闭保障

- `Drop` trait 兜底
- `shutdown::register_cleanup()` 在 Ctrl+C / panic 时触发
- Companion server 自身检测父进程退出后自行退出

### 新增文件结构

```
rust/
├── src/dashboard.rs                    ← Rust 端生命周期管理 (~80行)
└── ui/subagent-dashboard/             ← Bun companion
    ├── server.ts                       ← HTTP + SSE (~120行)
    ├── log-watcher.ts                  ← 日志监听 + 解析 (~100行)
    ├── dashboard.html                  ← 单页仪表盘 (~200行)
    └── package.json                    ← name + type: module
```

## 测试执行顺序

```
Phase 1: PTY 自动化测试 (Rust)
  cargo test --test e2e_subagent
  → 验证 subagent 端到端正确性
  → 日志文件生成到 .logs/

Phase 2: 仪表盘开发 + 手动探索
  启动 cc-rust (FEATURE_SUBAGENT_DASHBOARD=1)
  → companion 自动启动，浏览器打开仪表盘
  → 在 Claude Code 会话中触发 subagent
  → 观察仪表盘实时渲染

Phase 3: Chrome DevTools 自动化验证
  bun run tests/dashboard_verify.ts
  → 截图 + DOM 断言
  → 发现问题 → 回到 Phase 1/2 调整
```
