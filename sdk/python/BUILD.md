# claude-code-rs Python SDK — 构建说明

> 版本: 0.1.0 | 最后更新: 2026-04-06

`claude-code-rs` CLI 的 Python 封装 SDK，提供类型安全的流式接口，支持通过编程方式与 Claude Code Rust 代理交互。

架构参考: [OpenAI Codex Python SDK](../../../codex/sdk/python/) / [TypeScript SDK](../typescript/)

---

## 目录

1. [架构概述](#架构概述)
2. [前置要求](#前置要求)
3. [构建与安装](#构建与安装)
4. [快速开始](#快速开始)
5. [API 参考](#api-参考)
6. [事件类型](#事件类型)
7. [项目结构](#项目结构)
8. [JSONL 协议](#jsonl-协议)
9. [测试](#测试)
10. [与 TypeScript SDK 的对照](#与-typescript-sdk-的对照)
11. [设计决策](#设计决策)
12. [故障排查](#故障排查)

---

## 架构概述

```
┌─────────────────────────────────────────────────┐
│  Python SDK (本包)                               │
│                                                  │
│  ClaudeCode ──→ Session ──→ ClaudeCodeExec      │
│   (客户端)      (会话)       (进程管理)           │
│                    │                              │
│                    │  run() / run_streamed()      │
│                    ▼                              │
│   Popen("claude-code-rs --output-format json")   │
│         │                           ▲             │
│   stdin │ (prompt)       stdout     │ (JSONL)     │
│         ▼                           │             │
│  ┌──────────────────────────────────┘             │
│  │  readline → json.loads → transform_raw_event()│
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

**核心思路**: SDK 不直接调用 API，而是封装 CLI 二进制的子进程。通过 stdin 传入 prompt，通过 stdout 逐行读取 JSONL 事件。这与 OpenAI Codex SDK 和本项目 TypeScript SDK 的架构完全一致。

---

## 前置要求

### 运行时

- **Python** >= 3.10（使用了 `match` 语句和 `X | Y` 联合类型语法）
- **claude-code-rs** 二进制（已构建）
- **零运行时依赖** — 仅使用 Python 标准库

### 构建 Rust 二进制

```bash
cd rust/
cargo build --release
```

构建产物: `rust/target/release/claude-code-rs`（Windows: `claude-code-rs.exe`）

### 开发依赖（可选）

```bash
pip install -e "sdk/python[dev]"
```

安装:
- `pytest` >= 7.0 — 测试框架
- `pytest-asyncio` >= 0.21 — 异步测试支持
- `mypy` >= 1.0 — 静态类型检查

---

## 构建与安装

### 方式一: 开发安装（推荐）

```bash
cd rust/sdk/python/

# 可编辑安装 (源码直接引用，修改即时生效)
pip install -e .

# 含开发依赖
pip install -e ".[dev]"
```

### 方式二: 直接安装

```bash
cd rust/
pip install sdk/python/
```

### 方式三: 免安装使用

```bash
cd rust/sdk/python/
PYTHONPATH=src python your_script.py
```

Windows PowerShell:
```powershell
$env:PYTHONPATH = "src"
python your_script.py
```

### 验证安装

```bash
python -c "import claude_code_rs; print(f'SDK v{claude_code_rs.__version__} OK, {len(claude_code_rs.__all__)} exports')"
```

预期输出: `SDK v0.1.0 OK, 39 exports`

### 构建 wheel 包

```bash
cd rust/sdk/python/
pip install build
python -m build
```

产物: `dist/claude_code_rs-0.1.0-py3-none-any.whl`

---

## 快速开始

### 非流式（简单调用）

```python
from claude_code_rs import ClaudeCode, SessionOptions

client = ClaudeCode()
session = client.start_session(SessionOptions(
    permission_mode="auto",
    model="claude-sonnet-4-20250514",
))

turn = session.run("当前目录有哪些文件？")

print(turn.final_response)
print("Token 用量:", turn.usage)
```

### 流式（逐事件处理）

```python
from claude_code_rs import (
    ClaudeCode, SessionOptions,
    SessionStartedEvent, StreamDeltaEvent,
    ItemCompletedEvent, TurnCompletedEvent, TurnFailedEvent,
)

client = ClaudeCode()
session = client.start_session(SessionOptions(permission_mode="auto"))

streamed = session.run_streamed("帮我分析 src/main.rs 的架构")

for event in streamed.events:
    if isinstance(event, SessionStartedEvent):
        print(f"会话 {event.session_id} 已启动")

    elif isinstance(event, StreamDeltaEvent):
        # 实时输出文本增量
        if event.event_type == "content_block_delta":
            delta = event.delta
            if isinstance(delta, dict) and "text" in delta:
                print(delta["text"], end="", flush=True)

    elif isinstance(event, ItemCompletedEvent):
        if event.item and event.item.type == "tool_use_summary":
            print(f"\n[工具] {event.item.summary}")

    elif isinstance(event, TurnCompletedEvent):
        print(f"\n完成: ${event.usage.total_cost_usd:.4f}" if event.usage else "")

    elif isinstance(event, TurnFailedEvent):
        print(f"错误: {event.error.message}" if event.error else "")
```

### 恢复会话

```python
client = ClaudeCode()
session = client.start_session()
turn1 = session.run("读取 src/main.rs")

# 稍后 — 使用 session ID 恢复
resumed = client.resume_session(session.session_id)
turn2 = resumed.run("继续上次的工作")
```

### 指定二进制路径

```python
from claude_code_rs import ClaudeCode, ClientOptions

client = ClaudeCode(ClientOptions(
    executable_path="/path/to/claude-code-rs",
    api_key="sk-ant-...",
))
```

SDK 按以下顺序查找二进制:
1. `ClientOptions.executable_path` 显式指定
2. `CLAUDE_CODE_RS_PATH` 环境变量
3. 系统 `PATH` 中的 `claude-code-rs`（通过 `shutil.which`）
4. 相对路径 `../../target/release/claude-code-rs`（开发模式）
5. 相对路径 `../../target/debug/claude-code-rs`（开发模式）

---

## API 参考

### `ClaudeCode` — 客户端

```python
class ClaudeCode:
    def __init__(self, options: ClientOptions | None = None) -> None
    def start_session(self, options: SessionOptions | None = None) -> Session
    def resume_session(self, session_id: str, options: SessionOptions | None = None) -> Session
```

| ClientOptions | 类型 | 说明 |
|---|---|---|
| `executable_path` | `str \| None` | 二进制路径（自动检测） |
| `api_key` | `str \| None` | API 密钥（设为 `ANTHROPIC_API_KEY` 环境变量） |
| `env` | `dict[str, str] \| None` | 传递给子进程的环境变量 |

### `Session` — 会话

```python
class Session:
    @property
    def session_id(self) -> str | None

    # 缓冲模式: 等待整个 turn 完成后返回
    def run(self, input: str) -> Turn

    # 流式模式: 返回事件迭代器
    def run_streamed(self, input: str) -> StreamedTurn
```

| SessionOptions | 类型 | 对应 CLI 参数 |
|---|---|---|
| `model` | `str \| None` | `--model` |
| `working_directory` | `str \| None` | `--cwd` |
| `permission_mode` | `PermissionMode \| None` | `--permission-mode` |
| `max_turns` | `int \| None` | `--max-turns` |
| `max_budget` | `float \| None` | `--max-budget` |
| `system_prompt` | `str \| None` | `--system-prompt` |
| `append_system_prompt` | `str \| None` | `--append-system-prompt` |
| `verbose` | `bool` | `--verbose` |
| `continue_session` | `str \| None` | `--continue` |

`PermissionMode = Literal["default", "auto", "bypass", "plan"]`

### 返回类型

```python
@dataclass
class Turn:
    items: list[SessionItem]       # 本次 turn 的所有项
    final_response: str            # 最后一条 agent_message 的文本
    usage: Usage | None            # Token 用量和费用

@dataclass
class StreamedTurn:
    events: Iterator[SessionEvent]  # 同步事件迭代器
```

### 异常类型

```python
CcRustError                    # SDK 基础异常
├── BinaryNotFoundError        # 找不到二进制 (.searched: list[str])
├── ProcessError               # 子进程非零退出 (.exit_code, .stderr)
├── TurnExecutionError         # Turn 执行失败 (.subtype, .errors)
└── ParseError                 # JSONL 解析失败 (.line, .cause)
```

---

## 事件类型

SDK 将 Rust CLI 的原始 JSONL 事件转换为规范化的 `SessionEvent` 联合类型:

| SDK 事件 | 触发时机 | Rust SdkMessage 来源 |
|---|---|---|
| `SessionStartedEvent` | 会话初始化完成 | `SystemInit` |
| `TurnStartedEvent` | （预留） | — |
| `TurnCompletedEvent` | Turn 成功结束 | `Result` (is_error=false) |
| `TurnFailedEvent` | Turn 失败 | `Result` (is_error=true) |
| `ItemCompletedEvent` | 内容项完成 | `Assistant` / `ToolUseSummary` / `CompactBoundary` / `UserReplay` |
| `StreamDeltaEvent` | 实时流式增量 | `StreamEvent` |
| `SessionErrorEvent` | 可重试错误（如限流） | `ApiRetry` |

### Usage 类型

```python
@dataclass
class Usage:
    input_tokens: int         # 输入 token 数
    cached_input_tokens: int  # 缓存命中 token 数
    output_tokens: int        # 输出 token 数
    total_cost_usd: float     # 总费用 (USD)
```

### Item 类型

| 类型 | 说明 | 关键字段 |
|---|---|---|
| `AgentMessageItem` | 助手回复 | `text`, `content_blocks`, `usage`, `cost_usd` |
| `ToolUseSummaryItem` | 工具执行摘要 | `summary`, `preceding_tool_use_ids` |
| `CompactBoundaryItem` | 上下文压缩边界 | `pre_compact_token_count`, `post_compact_token_count` |
| `UserReplayItem` | 用户消息重放 | `content`, `is_replay`, `is_synthetic` |
| `ErrorItem` | 错误 | `message` |

### ContentBlock 类型

与 Anthropic API 一致:

| 类型 | 说明 |
|---|---|
| `TextBlock` | 文本内容 |
| `ToolUseBlock` | 工具调用 (id, name, input) |
| `ToolResultBlock` | 工具结果 |
| `ThinkingBlock` | 思维链 (Extended Thinking) |
| `RedactedThinkingBlock` | 已编辑的思维链 |
| `ImageBlock` | 图片 (base64) |

---

## 项目结构

```
rust/sdk/python/
├── pyproject.toml                包配置 (hatchling, 零运行时依赖)
├── BUILD.md                      构建说明 (本文件)
├── README.md                     快速使用文档
│
├── src/
│   └── claude_code_rs/           Python 包
│       ├── __init__.py           公共 API 导出 (39 个符号)
│       ├── claude_code.py        ClaudeCode 客户端类
│       ├── session.py            Session 会话类 (run / run_streamed)
│       ├── exec.py               ClaudeCodeExec 进程管理 (Popen + readline)
│       ├── transform.py          原始 JSONL → SessionEvent 转换层
│       ├── events.py             事件类型定义 (9 种事件)
│       ├── items.py              内容项类型 (5 种 + ContentBlock)
│       ├── config.py             配置选项 (ClientOptions + SessionOptions)
│       ├── errors.py             异常层级 (5 种异常)
│       └── py.typed              PEP 561 类型标记
│
└── tests/                        (Phase 2 添加)
    ├── conftest.py               样本 JSONL 载荷
    ├── test_transform.py         转换层单元测试
    ├── test_exec.py              二进制查找测试
    └── test_session.py           会话集成测试
```

### 层次架构

```
Public API     │  ClaudeCode, Session, 所有类型导出
───────────────┼──────────────────────────────────────
Execution      │  ClaudeCodeExec (Popen, readline, cleanup)
───────────────┼──────────────────────────────────────
Transform      │  transform_raw_event() — 原始 JSONL → 规范事件
───────────────┼──────────────────────────────────────
Types          │  events.py, items.py, config.py, errors.py
```

### 模块依赖图

```
errors.py          (0 deps)  ─┐
config.py          (0 deps)  ─┤
items.py           (0 deps)  ─┤
                              ├──→ events.py (← items)
                              ├──→ transform.py (← items + events)
                              ├──→ exec.py (← config + errors)
                              │         │
                              │         ▼
                              ├──→ session.py (← exec + events + items + transform)
                              │         │
                              │         ▼
                              ├──→ claude_code.py (← session + config + exec)
                              │         │
                              │         ▼
                              └──→ __init__.py (← all)
```

---

## JSONL 协议

每行是一个 JSON 对象，包含 `type` 字段标识消息类型。一个完整的 turn 输出示例:

```jsonl
{"type":"system_init","tools":["Bash","Read","Write"],"model":"claude-sonnet-4-20250514","permission_mode":"default","session_id":"abc-123","uuid":"..."}
{"type":"stream_event","event":{"type":"message_start","usage":{"input_tokens":100,...}},"session_id":"abc-123","uuid":"..."}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}},...}
{"type":"assistant","message":{"uuid":"...","content":[{"type":"text","text":"Hello! ..."}],"usage":{...},"cost_usd":0.001},"session_id":"abc-123"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":5000,"num_turns":1,"result":"Hello! ...","usage":{...},"total_cost_usd":0.001,...}
```

### 8 种消息类型

| 类型 | 字段 | Python SDK 映射 |
|---|---|---|
| `system_init` | tools, model, permission_mode, session_id, uuid | `SessionStartedEvent` |
| `assistant` | message (uuid, content, usage, cost_usd, ...), session_id | `ItemCompletedEvent(AgentMessageItem)` |
| `user_replay` | content, uuid, is_replay, is_synthetic | `ItemCompletedEvent(UserReplayItem)` |
| `stream_event` | event (type, index, delta, usage, content_block) | `StreamDeltaEvent` |
| `compact_boundary` | uuid, compact_metadata | `ItemCompletedEvent(CompactBoundaryItem)` |
| `api_retry` | attempt, max_retries, retry_delay_ms, error | `SessionErrorEvent(retryable=True)` |
| `tool_use_summary` | summary, preceding_tool_use_ids | `ItemCompletedEvent(ToolUseSummaryItem)` |
| `result` | subtype, is_error, duration_ms, result, usage, ... | `TurnCompletedEvent` / `TurnFailedEvent` |

每个 `submit_message()` 调用 **始终以一个 `result` 消息结束**。

---

## 测试

### 运行测试

```bash
cd rust/sdk/python/

# 运行所有测试
PYTHONPATH=src pytest

# 或安装后运行
pip install -e ".[dev]"
pytest

# 详细输出
pytest -v

# 覆盖率
pytest --cov=claude_code_rs --cov-report=term-missing
```

### 类型检查

```bash
cd rust/sdk/python/

# 严格模式
PYTHONPATH=src mypy src/claude_code_rs/ --strict

# 或安装后
pip install -e ".[dev]"
mypy src/claude_code_rs/ --strict
```

### 快速验证（无需安装）

```bash
cd rust/sdk/python/

# 验证导入
PYTHONPATH=src python -c "import claude_code_rs; print('OK')"

# 验证转换层
PYTHONPATH=src python -c "
from claude_code_rs.transform import transform_raw_event
from claude_code_rs.events import SessionStartedEvent

events = transform_raw_event({
    'type': 'system_init',
    'tools': ['Bash', 'Read'],
    'model': 'claude-sonnet-4-20250514',
    'permission_mode': 'default',
    'session_id': 'test',
    'uuid': '00000000-0000-0000-0000-000000000001',
})
assert len(events) == 1
assert isinstance(events[0], SessionStartedEvent)
assert events[0].model == 'claude-sonnet-4-20250514'
print('Transform OK')
"
```

### 端到端集成测试

前置: 已构建 `claude-code-rs` 并在 PATH 中，已配置 API Key。

```bash
cd rust/sdk/python/
PYTHONPATH=src python -c "
from claude_code_rs import ClaudeCode, SessionOptions

client = ClaudeCode()
session = client.start_session(SessionOptions(
    permission_mode='bypass',
    max_turns=1,
))
turn = session.run('echo hello world')
print(f'Response: {turn.final_response[:100]}')
print(f'Usage: {turn.usage}')
print('E2E OK')
"
```

---

## 与 TypeScript SDK 的对照

Python SDK 与 TypeScript SDK 是功能对等的镜像实现:

### 文件对照

| TypeScript | Python | 说明 |
|---|---|---|
| `claudeCode.ts` | `claude_code.py` | 顶层客户端 |
| `session.ts` | `session.py` | 会话管理 (run / run_streamed) |
| `exec.ts` | `exec.py` | 子进程管理 |
| `transform.ts` | `transform.py` | JSONL → 事件转换 |
| `events.ts` | `events.py` | 事件类型 |
| `items.ts` | `items.py` | 内容项类型 |
| `claudeCodeOptions.ts` | `config.py` (ClientOptions) | 客户端选项 |
| `sessionOptions.ts` | `config.py` (SessionOptions) | 会话选项 |
| `turnOptions.ts` | （内联在 session.py） | Turn 选项 |
| `index.ts` | `__init__.py` | 公共导出 |

### API 对照

| TypeScript | Python |
|---|---|
| `new ClaudeCode(options?)` | `ClaudeCode(options?)` |
| `client.startSession(options?)` | `client.start_session(options?)` |
| `client.resumeSession(id, options?)` | `client.resume_session(id, options?)` |
| `await session.run(input)` | `session.run(input)` |
| `await session.runStreamed(input)` | `session.run_streamed(input)` |
| `for await (const event of events)` | `for event in streamed.events:` |
| `turn.finalResponse` | `turn.final_response` |
| `turn.items` | `turn.items` |
| `turn.usage` | `turn.usage` |

### 关键差异

| 维度 | TypeScript | Python |
|---|---|---|
| 异步模型 | `async/await` + `AsyncGenerator` | 同步 `Iterator`（Phase 2 添加 async） |
| 类型系统 | TypeScript union types | `@dataclass` + `Union[...]` |
| 依赖 | `devDependencies` only | 零运行时依赖 |
| 包管理 | npm/package.json | pip/pyproject.toml (hatchling) |
| 取消机制 | `AbortSignal` | （Phase 2 添加 threading.Event） |
| 命名风格 | camelCase | snake_case (PEP 8) |

---

## 设计决策

### 为什么封装 CLI 而不是直接调用 API？

与 Codex SDK 一致的设计: CLI 已实现完整的工具执行、权限管理、会话持久化、上下文压缩等逻辑。SDK 只需关注进程通信和类型安全。

### 为什么零运行时依赖？

- 与 TypeScript SDK 保持一致（只依赖 Node.js 标准库）
- 避免 Pydantic 等大型依赖的版本冲突
- `dataclass` + `typing` 足以覆盖类型安全需求
- 降低安装门槛

### 为什么不用 Pydantic？

早期设计文档 (`docs/PYTHON_SDK_PLAN.md`) 建议使用 Pydantic `BaseModel` 做数据模型。最终改为 `dataclass`，原因:

1. **零依赖约束**: Pydantic 是运行时依赖
2. **TS SDK 先例**: TypeScript SDK 使用纯类型（raw dict + 类型断言），不做 JSON Schema 验证
3. **简单性**: 8 种消息类型在 `transform.py` 中直接从 dict 提取，无需中间反序列化层
4. **性能**: `dataclass` 实例化比 `BaseModel` 快 5-10x

### 为什么用 `list[SessionEvent]` 返回值？

`transform_raw_event()` 返回列表而非 `Optional[SessionEvent]`:
- 与 TypeScript SDK 的 `SessionEvent[]` 一致
- 未知消息类型返回空列表 `[]`（而非 `None`）
- 面向未来: 单条原始消息可能产生多个事件

### 为什么同步优先？

Python 的 `subprocess.Popen` 本质上是同步的（逐行 readline 阻塞），异步包装需要线程桥接。同步 API 作为 MVP:
- 覆盖最常见用例
- 实现简单，行为可预测
- 异步 API 在 Phase 2 通过 `asyncio.to_thread` + `asyncio.Queue` 桥接添加

---

## 故障排查

### `BinaryNotFoundError`

```
claude-code-rs not found. Searched: PATH, .../target/release/claude-code-rs
```

**解决**:
```bash
# 方式 1: 构建 Rust 二进制
cd rust/ && cargo build --release

# 方式 2: 设置环境变量
export CLAUDE_CODE_RS_PATH=/path/to/claude-code-rs

# 方式 3: 传入路径
client = ClaudeCode(ClientOptions(executable_path="/path/to/claude-code-rs"))
```

### `ProcessError` (exit code 非零)

```
claude-code-rs exited with code 1: ...
```

**可能原因**:
- API Key 未配置 → 设置 `ANTHROPIC_API_KEY` 或通过 `ClientOptions(api_key=...)` 传入
- 模型不可用 → 检查 `SessionOptions(model=...)` 拼写
- 网络问题 → 检查代理设置

### `ParseError` (JSONL 解析失败)

```
Failed to parse JSONL: Expecting value: line 1 column 1
```

**可能原因**:
- `claude-code-rs` 版本不匹配 — 重新构建: `cargo build --release`
- stderr 混入 stdout — 确认使用 `--output-format json` 模式

### `TurnExecutionError`

```
[error_max_turns] Maximum turns exceeded
```

**解决**: 增加 `SessionOptions(max_turns=20)` 或 `SessionOptions(max_budget=1.0)`

### Windows 特殊注意

- 二进制名自动添加 `.exe` 后缀（在相对路径查找时）
- `shutil.which("claude-code-rs")` 会自动处理 `.exe`
- 子进程使用 `subprocess.Popen`，兼容 Windows
- 环境变量通过 `os.environ` 继承，包含 Windows 系统路径

### 开发模式调试

```python
# 启用详细输出
session = client.start_session(SessionOptions(verbose=True))

# 使用自定义环境变量
client = ClaudeCode(ClientOptions(
    env={**os.environ, "RUST_LOG": "debug"},
))
```
