# bootstrap/ 架构文档

## 概述

`bootstrap/` 是 cc-rust 的**进程级全局单例层**，对应 TypeScript 原版的 `src/bootstrap/`。位于 import DAG 的叶节点——所有模块可依赖它，但它不依赖任何应用层代码。

**迁移状态：Phase 1-5 全部完成。** SessionId 品牌类型、ProcessState 全局单例、CWD 收编、计费/耗时统计、初始化流程适配均已集成到生产代码路径中。

## TypeScript 原版结构

```
bootstrap/
├── state.ts                          # 核心：进程全局状态单例
└── src/
    ├── types/
    │   ├── hooks.ts                  # HookEvent, HookCallbackMatcher
    │   └── ids.ts                    # SessionId 品牌类型
    └── utils/
        ├── crypto.ts                 # randomUUID 封装
        ├── signal.ts                 # 响应式信号原语
        ├── model/
        │   ├── model.ts              # ModelSetting 类型
        │   └── modelStrings.ts       # ModelStrings 类型
        └── settings/
            ├── constants.ts          # SettingSource 枚举
            ├── settingsCache.ts      # settings 缓存重置
            └── types.ts              # PluginHookMatcher 类型
```

### state.ts 全局状态分类

| 类别 | 字段 |
|---|---|
| 路径/会话 | `originalCwd`, `projectRoot`, `cwd`, `sessionId`, `parentSessionId` |
| 计费/性能 | `totalCostUSD`, `totalAPIDuration`, `totalToolDuration`, `modelUsage` |
| 模型配置 | `mainLoopModelOverride`, `initialMainLoopModel`, `modelStrings` |
| Telemetry | `meter`, `meterProvider`, `tracerProvider`, `loggerProvider`, `eventLogger` |
| 认证/token | `sessionIngressToken`, `oauthTokenFromFd`, `apiKeyFromFd` |
| 会话标志位 | `isInteractive`, `kairosActive`, `sessionBypassPermissionsMode`, `hasExitedPlanMode` |
| Hooks 注册 | `registeredHooks` — SDK callbacks + plugin native hooks |
| Cron/任务 | `sessionCronTasks`, `scheduledTasksEnabled`, `sessionCreatedTeams` |
| 调试/诊断 | `inMemoryErrorLog`, `slowOperations`, `lastAPIRequest`, `lastAPIRequestMessages` |
| Skill 追踪 | `invokedSkills` — 跨 compaction 保留已调用的 skill |

### 核心设计约束

```
// DO NOT ADD MORE STATE HERE - BE JUDICIOUS WITH GLOBAL STATE
```

- 任何模块都可以 `import { ... } from 'src/bootstrap/state.js'`
- `bootstrap/` **不能反向依赖**应用层模块
- 所有工具类 (crypto, signal 等) 放在 bootstrap 内部，保持隔离

---

## Rust 实现结构

```
src/bootstrap/
├── mod.rs              # 模块入口 + re-export
├── state.rs            # ProcessState 全局单例 + init() + 便利读取函数
├── ids.rs              # SessionId 品牌类型
├── signal.rs           # 响应式信号原语 (基于 tokio::watch)
├── model.rs            # ModelSetting, ModelTier, ModelStrings
├── diagnostics.rs      # ErrorLog, SlowOperationTracker
└── timing.rs           # DurationTracker (API/Tool 耗时)
```

### 依赖约束

```
bootstrap/  <──  engine/       (SessionId, PROCESS_STATE)
            <──  tools/        (PROCESS_STATE.tool_duration)
            <──  commands/     (SessionId)
            <──  query/
            <──  ui/
            <──  config/
            <──  session/
            <──  services/

bootstrap/  ──>  (仅 std + tokio::sync + uuid + serde)
```

**绝不**依赖 `engine`, `query`, `tools`, `api`, `ui`, `config`, `session` 等应用层模块。

---

## 实现对照表

### TS → Rust 映射

| TS bootstrap 状态 | Rust 位置 | 状态 |
|---|---|---|
| `originalCwd` | `ProcessState.original_cwd` | 已集成 — `main.rs:374` 写入 |
| `projectRoot` | `ProcessState.project_root` | 已集成 — `main.rs:375-376` via `find_git_root` |
| `sessionId` | `ProcessState.session_id: SessionId` | 已集成 — 品牌类型，全链路使用 |
| `parentSessionId` | `ProcessState.parent_session_id` | 已定义，预留 |
| `totalCostUSD` | `ProcessState.total_cost_usd` | 已集成 — `engine/lifecycle.rs:83` 同步写入 |
| `totalAPIDuration` | `ProcessState.api_duration: DurationTracker` | 已集成 — `engine/lifecycle.rs:808` 记录 |
| `totalToolDuration` | `ProcessState.tool_duration: DurationTracker` | 已集成 — `tools/execution.rs:286` 记录 |
| `mainLoopModelOverride` | `ProcessState.main_loop_model_override` | 已定义 |
| `initialMainLoopModel` | `ProcessState.initial_main_loop_model` | 已集成 — `main.rs:382` 写入 |
| `modelStrings` | `ProcessState.model_strings: Option<ModelStrings>` | 已定义 |
| `isInteractive` | `ProcessState.is_interactive` | 已集成 — `main.rs:381` 写入 |
| `inMemoryErrorLog` | `ProcessState.error_log: ErrorLog` | 已定义 |
| `slowOperations` | `ProcessState.slow_operations: SlowOperationTracker` | 已定义 |
| `invokedSkills` | `ProcessState.invoked_skills: HashMap<String, bool>` | 已定义 |
| `cwd` (动态) | `utils/cwd.rs` — `static CWD: Mutex<Option<PathBuf>>` | 保留在 utils，职责清晰 |

### 不纳入实现的部分

| TS 功能 | 原因 |
|---|---|
| OTel Telemetry (`meter`, `tracerProvider`, ...) | Lite 版已移除 analytics，`tracing` crate 足够 |
| Hooks 注册中心 (`registeredHooks`) | 无 plugin/SDK agent 系统，当前 `tools/hooks.rs` 够用 |
| Cron/任务 (`sessionCronTasks`, ...) | Lite 版不需要 |
| Agent 颜色管理 (`agentColorManager`) | 无 Agent tool |
| `SettingSource` 追踪 | 只有两级来源 (global + project)，复杂度不值得 |
| `PluginHookMatcher` 类型 | 无 plugin 系统 |
| `oauthTokenFromFd` / `apiKeyFromFd` | Lite 版不支持 fd 传递认证 |
| `lastAPIRequestMessages` | `/share` 功能不在 Lite 范围内 |

---

## 核心类型说明

### `ids.rs` — SessionId 品牌类型

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);
```

- `SessionId::new()` — UUID v4
- `SessionId::from_string(impl Into<String>)` — session resume 场景
- 实现 `Default`, `Display`, `AsRef<str>` — 与 `&str` 互操作无缝
- 所有需要 session ID 的函数签名均使用此类型（`engine/lifecycle.rs`, `commands/mod.rs`, `session/` 等）

### `signal.rs` — 响应式信号

```rust
pub struct Signal<T: Clone + Send + Sync + 'static> { tx, rx }
```

- `get()` / `set()` / `subscribe()` — 基本读写和异步订阅
- `set_if_changed()` — 仅值变更时通知（需 `PartialEq`）
- 与 `utils/abort.rs` 的 `AbortController` 区别：Signal 承载任意类型，AbortController 仅 bool 信号

### `model.rs` — 模型配置类型

- `ModelTier` — `Opus | Sonnet | Haiku | Unknown`，实现 `Display`
- `ModelSetting::from_model_id()` — 大小写不敏感推断 tier 和 display name
- `ModelStrings::from_setting()` — 从 ModelSetting 快速构造 UI 显示字符串

### `diagnostics.rs` — 诊断信息

- `ErrorLog` — 固定容量环形缓冲，默认 100 条，不写磁盘
- `SlowOperationTracker` — 默认阈值 500ms / 最多 50 条，含 `timestamp: Instant`
- 两者均实现 `Default`，`ProcessState::default()` 直接使用

### `timing.rs` — 无锁耗时追踪

```rust
pub struct DurationTracker { total_ms: AtomicU64, count: AtomicU64 }
```

- `record(duration_ms)` — 原子累加，多线程安全，无需持锁
- `total_ms()` / `count()` / `avg_ms()` — 只读查询
- `const fn new()` — 可在编译时构造

### `state.rs` — ProcessState 全局单例

```rust
pub static PROCESS_STATE: LazyLock<RwLock<ProcessState>> = ...;
```

**字段分组：**

| 类别 | 字段 | 可变性 |
|---|---|---|
| 路径/身份 | `original_cwd`, `project_root`, `session_id`, `parent_session_id` | 启动时写入，之后不变 |
| 计费统计 | `total_cost_usd`, `api_duration`, `tool_duration` | 只增不减 |
| 模型配置 | `main_loop_model_override`, `initial_main_loop_model`, `model_strings` | 运行中可更新 |
| 会话标志 | `is_interactive` | 启动时写入 |
| 诊断 | `error_log`, `slow_operations` | 运行中追加 |
| Skill 追踪 | `invoked_skills` | 运行中追加 |

**便利方法：**

- `effective_model()` — override 优先于 initial
- `log_error()` / `record_operation()` — 封装诊断写入
- `mark_skill_invoked()` / `is_skill_invoked()` — skill 状态查询

**免锁读取函数：**

```rust
pub fn session_id() -> SessionId { ... }
pub fn original_cwd() -> PathBuf { ... }
pub fn project_root() -> PathBuf { ... }
pub fn total_cost_usd() -> f64 { ... }
```

**初始化函数：**

```rust
pub fn init(cwd, project_root, session_id, is_interactive, initial_model) { ... }
```

在 `main.rs` Phase B.7.1 调用，写入不可变身份字段。

---

## 集成点

### main.rs — 初始化

```
Phase B.7:   QueryEngine::new()          → session_id: SessionId::new()
Phase B.7.1: init_process_state()        → 写入 original_cwd, project_root, session_id,
                                            is_interactive, initial_main_loop_model
```

### engine/lifecycle.rs — 计费同步

```
UsageTracking::add_usage()  → PROCESS_STATE.total_cost_usd += cost_usd
submit_message() yield      → PROCESS_STATE.api_duration.record(api_duration_ms)
```

### tools/execution.rs — 工具耗时

```
tool 执行完成后  → PROCESS_STATE.tool_duration.record(duration_ms)
```

### commands/ — SessionId 消费

所有 28 个命令通过 `CommandContext.session_id: SessionId` 接收，不再使用裸 `String`。

---

## 与现有模块的关系

```
                    ┌─────────────────────────────────────────┐
                    │              main.rs                     │
                    │  Phase A: parse CLI                      │
                    │  Phase B: init ProcessState → tools →    │
                    │           AppState → QueryEngine → TUI   │
                    │  Phase I: graceful_shutdown               │
                    └───────┬──────────────┬──────────────┬────┘
                            │              │              │
                    ┌───────▼──┐   ┌───────▼──┐   ┌──────▼───┐
                    │ engine/  │   │  tools/  │   │   ui/    │
                    │ query/   │   │  skills/ │   │          │
                    └───────┬──┘   └───────┬──┘   └──────┬───┘
                            │              │              │
                            ▼              ▼              ▼
                    ┌─────────────────────────────────────────┐
                    │           bootstrap/                     │
                    │  PROCESS_STATE (全局单例)                 │
                    │  SessionId, Signal<T>, ModelSetting      │
                    │  ErrorLog, SlowOperationTracker          │
                    │  DurationTracker                         │
                    └─────────────────────────────────────────┘
                            │
                            ▼
                    (std, tokio::sync, uuid, serde)
```

### 与 AppState 的分工

| | `ProcessState` (bootstrap) | `AppState` (types/app_state.rs) |
|---|---|---|
| 生命周期 | 进程级，启动到退出 | 会话级，可跨 QueryEngine 共享 |
| 可变性 | 身份信息不可变，统计只增不减 | UI/配置状态随时变更 |
| 访问方式 | `PROCESS_STATE` 全局静态 | `Arc<RwLock<AppState>>` 实例传递 |
| 内容 | 路径、session ID、计费、诊断 | settings、model、permission context |
| 依赖方向 | 叶节点，不依赖任何应用层 | 依赖 `bootstrap::SessionId` 等 |

### 与 QueryEngine 的分工

| | `ProcessState` | `QueryEngine` |
|---|---|---|
| 统计 | 全局累计 (跨多次 submit) | 单次 submit 的中间状态 |
| Session ID | 权威来源 | 从 `ProcessState` 读取 |
| Usage | `total_cost_usd` (全局) | `UsageTracking` (详细明细) |
| 生命周期 | 进程级 | 单个会话 |

---

## 测试覆盖

每个子模块均含单元测试：

| 文件 | 测试点 |
|---|---|
| `ids.rs` | unique ID generation, from_string roundtrip, Display, serde roundtrip |
| `signal.rs` | initial value, set/get, subscriber notification, set_if_changed, async await |
| `model.rs` | tier detection (sonnet/opus/haiku/unknown), ModelStrings construction |
| `diagnostics.rs` | push/read, ring-buffer eviction, clear, threshold filtering |
| `timing.rs` | initial state, accumulation, avg_ms, concurrent thread safety |
| `state.rs` | default session_id validity, effective_model priority, skill tracking, diagnostics integration |
