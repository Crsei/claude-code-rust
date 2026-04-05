# bootstrap/ 架构文档

## 概述

`bootstrap/` 是 Claude Code TypeScript 版本中的**进程级全局单例层**，位于 import DAG 的叶节点——所有模块可依赖它，但它不依赖任何应用层代码。

Rust Lite 版本目前**没有对应的 `bootstrap` 模块**。全局状态分散在 `AppState`、`QueryEngine`、`utils/cwd.rs` 等处。本文档分析差距并给出实现方案。

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

## Rust 现状对照

### 已实现 (散落在各处)

| TS bootstrap 状态 | Rust 当前位置 | 差异 |
|---|---|---|
| `cwd` | `utils/cwd.rs` — `static CWD: Mutex<Option<PathBuf>>` | 功能完整，但属于 utils 而非统一状态层 |
| `sessionId` | `QueryEngine.session_id: String` | 实例字段，非全局；裸 String 无品牌类型 |
| `totalCostUSD` | `QueryEngine.usage: Arc<Mutex<UsageTracking>>` | 实例字段，非全局；缺少 duration 统计 |
| `mainLoopModel` | `AppState.main_loop_model: String` | 有，但无 override/initial 区分 |
| `isInteractive` | 通过 `cli.print` 隐式判断 | 无显式标志位 |
| Abort 信号 | `utils/abort.rs` — `AbortController` | 设计良好，tokio::watch 实现 |

### 完全缺失

| TS bootstrap 状态 | 说明 |
|---|---|
| `originalCwd` | 启动时的原始 CWD，不随 `cd` 或 worktree 改变 |
| `projectRoot` | 项目根目录标识，不随 worktree 更新 |
| `parentSessionId` | 子代理/子会话的父会话标识 |
| `totalAPIDuration` / `totalToolDuration` | 性能计时统计 |
| `mainLoopModelOverride` / `initialMainLoopModel` | 模型覆盖链 |
| `ModelSetting` / `ModelStrings` | 结构化模型信息 (tier, display name) |
| `SessionId` 品牌类型 | 类型安全的会话 ID |
| `Signal<T>` 响应式原语 | 通用状态广播 |
| `SettingSource` | 设置值来源追踪 |
| `inMemoryErrorLog` / `slowOperations` | 调试诊断信息收集 |
| `invokedSkills` | 跨 compaction 的 skill 调用追踪 |
| `registeredHooks` | 统一 hooks 注册中心 |
| OTel Telemetry | meter, tracer, logger providers |

### 根因分析

Rust 版没有一个**统一的全局状态入口点**。导致：

1. **路径语义混淆** — 只有一个 `cwd`，无法区分 "启动目录" vs "项目根" vs "当前逻辑目录"
2. **状态访问路径长** — 需要通过 `engine.usage()` 访问计费数据，无法从任意位置直接获取
3. **类型安全弱** — `session_id` 是裸 `String`，和任意其他字符串无类型级区分
4. **无诊断信息** — 出问题时缺少 error log / slow operation 记录，调试困难

---

## 实现方案

### 目标文件结构

```
src/bootstrap/
├── mod.rs              # 模块入口
├── state.rs            # ProcessState 全局单例
├── ids.rs              # SessionId 品牌类型
├── signal.rs           # 响应式信号原语 (基于 tokio::watch)
├── model.rs            # ModelSetting, ModelTier, ModelStrings
├── diagnostics.rs      # ErrorLog, SlowOperationTracker
└── timing.rs           # DurationTracker (API/Tool 耗时)
```

### 依赖约束

```
bootstrap/  ←──  engine/
            ←──  query/
            ←──  tools/
            ←──  ui/
            ←──  config/
            ←──  session/
            ←──  services/

bootstrap/  ──→  (仅标准库 + tokio::sync + uuid + serde)
```

bootstrap 只依赖：
- `std` (sync, path, collections, time)
- `tokio::sync` (watch, Mutex)
- `uuid` (生成 SessionId)
- `serde` / `serde_json` (序列化)

**绝不**依赖 `engine`, `query`, `tools`, `api`, `ui`, `config`, `session` 等应用层模块。

---

### 核心类型定义

#### `ids.rs` — 品牌类型

```rust
use serde::{Deserialize, Serialize};

/// 会话 ID 品牌类型 — 防止与其他 String 混用
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
```

#### `model.rs` — 模型设置

```rust
/// 模型层级
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
    Unknown,
}

/// 结构化模型配置
#[derive(Debug, Clone)]
pub struct ModelSetting {
    /// 原始模型 ID ("claude-sonnet-4-20250514")
    pub model_id: String,
    /// 显示名称 ("Sonnet 4")
    pub display_name: String,
    /// 层级
    pub tier: ModelTier,
}

/// 模型字符串集合 — 用于 UI 显示和日志
#[derive(Debug, Clone)]
pub struct ModelStrings {
    /// 主模型完整 ID
    pub main_model_id: String,
    /// 主模型显示名
    pub main_display: String,
    /// 快速模式模型显示名
    pub fast_display: Option<String>,
}

impl ModelSetting {
    /// 从原始模型 ID 推断 tier 和显示名
    pub fn from_model_id(model_id: &str) -> Self {
        let (tier, display_name) = if model_id.contains("opus") {
            (ModelTier::Opus, "Opus")
        } else if model_id.contains("haiku") {
            (ModelTier::Haiku, "Haiku")
        } else if model_id.contains("sonnet") {
            (ModelTier::Sonnet, "Sonnet")
        } else {
            (ModelTier::Unknown, model_id)
        };

        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            tier,
        }
    }
}
```

#### `signal.rs` — 响应式信号

```rust
use tokio::sync::watch;

/// 响应式信号原语 — 包装 tokio::watch 提供简洁 API
///
/// 对应 TypeScript: bootstrap/src/utils/signal.ts
///
/// 与 AbortController 的区别:
/// - AbortController 只做 bool 信号 (aborted or not)
/// - Signal<T> 承载任意类型的值变更通知
pub struct Signal<T: Clone + Send + Sync> {
    tx: watch::Sender<T>,
    rx: watch::Receiver<T>,
}

impl<T: Clone + Send + Sync> Signal<T> {
    pub fn new(initial: T) -> Self {
        let (tx, rx) = watch::channel(initial);
        Self { tx, rx }
    }

    /// 读取当前值
    pub fn get(&self) -> T {
        self.rx.borrow().clone()
    }

    /// 设置新值，通知所有订阅者
    pub fn set(&self, value: T) {
        let _ = self.tx.send(value);
    }

    /// 创建一个新的订阅者
    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.rx.clone()
    }
}
```

#### `diagnostics.rs` — 诊断信息

```rust
use std::collections::VecDeque;
use std::time::Instant;

/// 内存中的错误日志 — 不写磁盘，仅在会话内保留
pub struct ErrorLog {
    entries: VecDeque<ErrorEntry>,
    max_entries: usize,
}

pub struct ErrorEntry {
    pub message: String,
    pub timestamp: Instant,
    pub context: Option<String>,
}

impl ErrorLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    pub fn push(&mut self, message: String, context: Option<String>) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(ErrorEntry {
            message,
            timestamp: Instant::now(),
            context,
        });
    }

    pub fn entries(&self) -> &VecDeque<ErrorEntry> {
        &self.entries
    }
}

/// 慢操作追踪器
pub struct SlowOperationTracker {
    /// (操作名, 耗时 ms)
    operations: VecDeque<(String, u64)>,
    /// 超过此阈值才记录 (ms)
    threshold_ms: u64,
    max_entries: usize,
}

impl SlowOperationTracker {
    pub fn new(threshold_ms: u64, max_entries: usize) -> Self {
        Self {
            operations: VecDeque::new(),
            threshold_ms,
            max_entries,
        }
    }

    /// 记录一次操作，仅当耗时超过阈值时保留
    pub fn record(&mut self, name: String, duration_ms: u64) {
        if duration_ms >= self.threshold_ms {
            if self.operations.len() >= self.max_entries {
                self.operations.pop_front();
            }
            self.operations.push_back((name, duration_ms));
        }
    }

    pub fn operations(&self) -> &VecDeque<(String, u64)> {
        &self.operations
    }
}
```

#### `timing.rs` — 耗时追踪

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// 累计耗时追踪 — 线程安全，无需持锁
pub struct DurationTracker {
    total_ms: AtomicU64,
    count: AtomicU64,
}

impl DurationTracker {
    pub fn new() -> Self {
        Self {
            total_ms: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    pub fn record(&self, duration_ms: u64) {
        self.total_ms.fetch_add(duration_ms, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_ms(&self) -> u64 {
        self.total_ms.load(Ordering::Relaxed)
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}
```

#### `state.rs` — 全局单例

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

use super::diagnostics::{ErrorLog, SlowOperationTracker};
use super::ids::SessionId;
use super::model::ModelStrings;
use super::timing::DurationTracker;

/// 进程级全局状态 — 整个应用只有一份
///
/// 对应 TypeScript: bootstrap/state.ts
///
/// 设计约束:
/// - DO NOT ADD MORE STATE HERE — BE JUDICIOUS WITH GLOBAL STATE
/// - 只放真正需要全局访问的状态
/// - 与 AppState 的区别: AppState 是 React/UI 层的状态容器,
///   ProcessState 是进程级不可变身份 + 累计统计
pub static PROCESS_STATE: LazyLock<RwLock<ProcessState>> =
    LazyLock::new(|| RwLock::new(ProcessState::default()));

pub struct ProcessState {
    // ── 路径/会话身份 (启动时设定, 不可变) ─────────────────────
    /// 进程启动时的工作目录 — 永不变更
    pub original_cwd: PathBuf,
    /// 项目根目录 — 启动时通过 git/配置文件探测, 不随 worktree 更新
    /// 用于项目身份标识 (history, skills, sessions), 而非文件操作
    pub project_root: PathBuf,
    /// 会话 ID
    pub session_id: SessionId,
    /// 父会话 ID (子代理场景)
    pub parent_session_id: Option<SessionId>,

    // ── 计费/性能统计 (累计, 只增不减) ──────────────────────
    /// 总花费 (USD)
    pub total_cost_usd: f64,
    /// API 调用累计耗时
    pub api_duration: DurationTracker,
    /// Tool 执行累计耗时
    pub tool_duration: DurationTracker,

    // ── 模型配置 ──────────────────────────────────────────
    /// 用户在运行中覆盖的模型 (通过 /model 命令)
    pub main_loop_model_override: Option<String>,
    /// 启动时确定的初始模型 (CLI > config > provider default)
    pub initial_main_loop_model: Option<String>,
    /// 模型显示字符串
    pub model_strings: Option<ModelStrings>,

    // ── 会话标志位 ────────────────────────────────────────
    /// 是否为交互式会话 (false = print mode / pipe mode)
    pub is_interactive: bool,

    // ── 调试/诊断 ─────────────────────────────────────────
    /// 内存错误日志 (最近 100 条)
    pub error_log: ErrorLog,
    /// 慢操作记录 (> 500ms)
    pub slow_operations: SlowOperationTracker,

    // ── Skill 追踪 ────────────────────────────────────────
    /// 已调用的 skills — key: "agentId:skillName"
    /// 跨 compaction 保留, 确保 compaction 后仍能恢复
    pub invoked_skills: HashMap<String, bool>,
}
```

#### `mod.rs` — 模块入口

```rust
//! bootstrap/ — 进程级全局单例层
//!
//! 导入 DAG 叶节点: 任何模块可以依赖 bootstrap,
//! 但 bootstrap 绝不依赖应用层模块 (engine, query, tools, api, ui, ...)
//!
//! 对应 TypeScript: src/bootstrap/

pub mod state;
pub mod ids;
pub mod signal;
pub mod model;
pub mod diagnostics;
pub mod timing;

// 公开常用类型, 方便其他模块引用
pub use ids::SessionId;
pub use state::PROCESS_STATE;
pub use signal::Signal;
pub use model::{ModelSetting, ModelTier, ModelStrings};
```

---

## 迁移计划

### Phase 1: 建立模块骨架 (无破坏性)

创建 `src/bootstrap/` 目录及所有文件。在 `main.rs` 中 `mod bootstrap;` 声明。此阶段不修改任何现有代码，只是让模块编译通过。

预计改动：
- 新增 7 个文件 (`mod.rs`, `state.rs`, `ids.rs`, `signal.rs`, `model.rs`, `diagnostics.rs`, `timing.rs`)
- `main.rs` 新增一行 `mod bootstrap;`

### Phase 2: 替换 SessionId

将 `QueryEngine.session_id: String` 改为 `SessionId`，同步更新：

| 文件 | 改动 |
|---|---|
| `engine/lifecycle.rs` | `session_id: String` → `SessionId`; `Uuid::new_v4().to_string()` → `SessionId::new()` |
| `session/storage.rs` | 函数签名 `session_id: &str` → `session_id: &SessionId` |
| `session/transcript.rs` | 同上 |
| `session/resume.rs` | 返回类型中的 `session_id: String` → `SessionId` |
| `shutdown.rs` | `&engine.session_id` 的使用适配 |

涟漪范围约 10-15 处 `&str` → `&SessionId` / `.as_str()` 适配。

### Phase 3: 收编 CWD 到 ProcessState

将 `utils/cwd.rs` 中的全局 `CWD` 迁移到 `ProcessState`：

```
之前:
  utils/cwd.rs  →  static CWD: Mutex<Option<PathBuf>>
  main.rs       →  resolve_cwd() 返回 String

之后:
  bootstrap/state.rs  →  PROCESS_STATE.original_cwd (不变)
                      →  动态 cwd 仍由 utils/cwd.rs 维护 (职责清晰)
  main.rs             →  启动时写入 PROCESS_STATE.original_cwd + project_root
```

保留 `utils/cwd.rs` 管理运行时动态 CWD (工具执行时可能 cd)，但在 `ProcessState` 中记录不可变的 `original_cwd` 和 `project_root`。

### Phase 4: 统一计费/性能统计

将 `QueryEngine.usage` 的写入同步到 `PROCESS_STATE`：

```rust
// engine/lifecycle.rs — 在 UsageTracking::add_usage 时同步写入
impl UsageTracking {
    pub fn add_usage(&mut self, usage: &Usage, cost_usd: f64) {
        // ... 现有逻辑 ...

        // 同步写入全局状态
        if let Ok(mut state) = PROCESS_STATE.write() {
            state.total_cost_usd += cost_usd;
        }
    }
}
```

`api_duration` / `tool_duration` 使用 `DurationTracker` (AtomicU64)，无需持锁，在 API 调用和 tool 执行的前后各记录时间戳即可。

### Phase 5: 初始化流程适配

修改 `main.rs::run_full_init()` 在 Phase B 早期初始化 `PROCESS_STATE`：

```rust
async fn run_full_init(cli: Cli) -> anyhow::Result<ExitCode> {
    let cwd = resolve_cwd(&cli);

    // ★ 新增: 初始化 ProcessState
    {
        let mut state = bootstrap::PROCESS_STATE.write().unwrap();
        state.original_cwd = PathBuf::from(&cwd);
        state.project_root = detect_project_root(&cwd);
        state.session_id = bootstrap::SessionId::new();
        state.is_interactive = !cli.print;
        state.initial_main_loop_model = Some(model.clone());
    }

    // ... 现有 B.1 ~ B.10 ...
}
```

---

## 不纳入实现的部分

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

### 与 `AppState` 的分工

| | `ProcessState` (bootstrap) | `AppState` (types/app_state.rs) |
|---|---|---|
| 生命周期 | 进程级，启动到退出 | 会话级，可跨 QueryEngine 共享 |
| 可变性 | 身份信息不可变，统计只增不减 | UI/配置状态随时变更 |
| 访问方式 | `PROCESS_STATE` 全局静态 | `Arc<RwLock<AppState>>` 实例传递 |
| 内容 | 路径、session ID、计费、诊断 | settings、model、permission context |
| 依赖方向 | 叶节点，不依赖任何应用层 | 依赖 `bootstrap::SessionId` 等 |

### 与 `QueryEngine` 的分工

| | `ProcessState` | `QueryEngine` |
|---|---|---|
| 统计 | 全局累计 (跨多次 submit) | 单次 submit 的中间状态 |
| Session ID | 权威来源 | 从 `ProcessState` 读取 |
| Usage | `total_cost_usd` (全局) | `UsageTracking` (详细明细) |
| 生命周期 | 进程级 | 单个会话 |
