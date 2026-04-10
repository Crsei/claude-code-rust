# cc-rust 技术债务审计报告

> 审计日期: 2026-04-08 | 分支: rust-lite
>
> **修复记录**: 第一批 (安全性) 和第二批 (可维护性) 已完成 — 详见 [已修复](#已修复的问题)

---

## 目录

- [CRITICAL — 必须优先修复](#critical--必须优先修复)
- [HIGH — 严重影响可维护性](#high--严重影响可维护性)
- [MEDIUM — 代码异味与一致性问题](#medium--代码异味与一致性问题)
- [严重程度汇总](#严重程度汇总)
- [建议修复路线图](#建议修复路线图)

---

## CRITICAL — 必须优先修复

### ~~1. `query/loop_impl.rs` — 1105 行巨型异步生成器~~ ✅ 已修复

> **已拆分为 3 个文件**: `loop_impl.rs` (451行, 核心 stream 循环) + `loop_helpers.rs` (253行, 辅助函数) + `loop_tests.rs` (369行, 测试)
>
> 辅助函数 `handle_prompt_too_long()`, `handle_max_output_tokens()`, `execute_tool_calls()`, `make_abort_message()`, `make_error_message()`, `make_user_message()` 已提取到 `loop_helpers.rs`。

---

### ~~2. `engine/lifecycle.rs` — 1703 行上帝文件~~ ✅ 已修复

> **已拆分为 `engine/lifecycle/` 目录模块 (6 个文件)**:
> - `mod.rs` (201行) — QueryEngine struct + 构造器 + pub 方法 + re-exports
> - `submit_message.rs` (637行) — Phase A-E 主流式管道
> - `deps.rs` (399行) — QueryEngineDeps struct + QueryDeps trait 实现
> - `helpers.rs` (194行) — format_conversation_for_summary, build_messages_request
> - `types.rs` (61行) — UsageTracking, PermissionDenial, AbortReason
> - `tests.rs` (197行) — 所有测试

---

### 3. 命令处理器中的系统性 `panic!()` — 26+ 处

**受影响文件**: `commands/` 目录下 20+ 个文件

```rust
// 当前代码 (出现在 compact.rs, config_cmd.rs, context.rs, copy.rs,
// cost.rs, exit.rs, extra_usage.rs, fast.rs 等)
match result {
    CommandResult::Output(text) => { /* ... */ },
    _ => panic!("Expected Output result"),  // 生产代码中的 panic!
}
```

**影响**: 如果 `CommandResult` 枚举新增变体或逻辑变更，进程直接崩溃而非优雅降级。

**建议**:
```rust
// 修复方案
match result {
    CommandResult::Output(text) => { /* ... */ },
    other => return Err(anyhow::anyhow!("Expected Output result, got: {:?}", other)),
}
```

---

### 4. API 客户端中的 `panic!()` — 5 处

**文件**: `src/api/client.rs:800+`

```rust
panic!("expected OpenAiCompat");
panic!("expected Anthropic provider, got {:?}", other);
```

**影响**: 如果 API 服务器返回异常格式或 provider 配置错误，整个进程崩溃。

**建议**: 替换为 `Result` 返回，上层统一处理。

---

## HIGH — 严重影响可维护性

### 5. QueryEngine 的 10 个 `Arc<Mutex/RwLock>` 字段

**文件**: `src/engine/lifecycle/mod.rs:56-88`

```rust
pub struct QueryEngine {
    mutable_messages: Arc<RwLock<Vec<Message>>>,
    abort_reason: Arc<Mutex<Option<AbortReason>>>,
    aborted: Arc<AtomicBool>,
    usage: Arc<Mutex<UsageTracking>>,
    permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    total_turn_count: Arc<Mutex<usize>>,
    app_state: Arc<RwLock<AppState>>,
    tools: Arc<RwLock<Tools>>,
    discovered_skill_names: Arc<Mutex<HashSet<String>>>,
    loaded_nested_memory_paths: Arc<Mutex<HashSet<String>>>,
}
```

**问题**:
- 每个字段独立加锁，`submit_message` 闭包中逐一 clone (lines 210-224)
- 无法保证跨字段的一致性（需要修改 A 和 B 时，它们各自独立加锁）
- 潜在的锁竞争和死锁风险

**建议**: 合并为单一状态结构:
```rust
struct QueryEngineState {
    messages: Vec<Message>,
    abort_reason: Option<AbortReason>,
    usage: UsageTracking,
    permission_denials: Vec<PermissionDenial>,
    // ...
}

pub struct QueryEngine {
    state: Arc<RwLock<QueryEngineState>>,
    aborted: Arc<AtomicBool>,  // 保留独立，因为需要无锁检查
}
```

---

### 6. 全局 `PROCESS_STATE` 单例

**文件**: `src/bootstrap/state.rs:21-26`

```rust
pub static PROCESS_STATE: LazyLock<RwLock<ProcessState>> =
    LazyLock::new(|| RwLock::new(ProcessState::default()));
```

**问题**:
- 从 async 流式闭包内写入 `total_cost_usd`，存在锁竞争
- 15+ 个 `pub` 字段，无任何封装
- 5 处 `.expect("PROCESS_STATE poisoned")` — RwLock poison 时直接 panic
- 代码注释承认: "DO NOT ADD MORE STATE HERE"

**建议**: 
- 使用 `parking_lot::RwLock` (不会 poison)
- 将频繁更新的字段（如 `total_cost_usd`）改用 `AtomicF64` 或独立的 `Arc<Mutex<f64>>`
- 添加 getter/setter 方法封装字段访问

---

### 7. API 重试逻辑代码重复 ~90 行

**文件**: `src/api/client.rs:409-499`

`messages_stream_with_retry` 和 `messages_with_retry` 包含几乎相同的重试循环:

```rust
// 两处独立维护的相同逻辑:
let is_retryable = err_msg.contains("RateLimit")
    || err_msg.contains("Overloaded")
    || err_msg.contains("ServerError")
    || err_msg.contains("HTTP 429")
    || err_msg.contains("HTTP 500")
    || err_msg.contains("HTTP 502")
    || err_msg.contains("HTTP 503")
    || err_msg.contains("HTTP 529");
```

**双重问题**:
1. ~90 行重复代码
2. 错误检测靠**字符串匹配 HTTP 状态码**，极其脆弱

**建议**: 提取 `RetryPolicy` 结构体，使用类型化的错误分类（而非字符串匹配）。

---

### 8. API 提供商抽象不足

**文件**: `src/api/client.rs`, `src/api/google_provider.rs`, `src/api/openai_compat.rs`

**问题**:
- 三个提供商各自实现消息格式转换，存在大量重复逻辑
- Provider routing 通过散落在多个函数中的 match 语句实现 (client.rs:302-370)
- `ApiProvider` 枚举字段命名不一致: `base_url` vs `endpoint` vs `project_id`
- `Bedrock` 和 `Vertex` 变体标记 `#[allow(dead_code)]` — 未实现的空壳

**建议**: 定义 `trait ApiProviderImpl`，每个提供商独立实现，通过 trait object 分发。

---

### ~~9. `mcp/client.rs` — 1008 行混合 6 种职责~~ ✅ 已修复

> **已拆分为 4 个文件**:
> - `client.rs` (502行) — McpClient struct + 连接/初始化/断开 + 工具/资源操作 + JSON-RPC 消息
> - `transport.rs` (142行) — reader_loop + dispatch_response (后台 I/O)
> - `manager.rs` (127行) — McpManager (多服务器编排)
> - `client_tests.rs` (234行) — 所有测试

---

## MEDIUM — 代码异味与一致性问题

### 10. 广泛的 `#![allow(unused)]` 指令

- **commands/**: 20/32 个文件顶部有此指令
- **tools/**: 7/20+ 个文件顶部有此指令

**影响**: 掩盖了真正的未使用代码和导入警告，累积死代码。

---

### 11. 测试样板代码重复 — 13+ 处

`test_ctx()` 辅助函数在 13+ 个命令文件中完全相同地复制粘贴:

```rust
// 在 help.rs, config_cmd.rs, context.rs, copy.rs, fast.rs,
// init.rs, model.rs, mcp_cmd.rs, permissions_cmd.rs,
// resume.rs, session.rs, skills_cmd.rs, status.rs 中重复
fn test_ctx() -> CommandContext {
    CommandContext {
        messages: Vec::new(),
        cwd: PathBuf::from("."),
        app_state: AppState::default(),
        session_id: SessionId::from_string("test-session"),
    }
}
```

**建议**: 提取到 `commands/test_utils.rs` 或 `#[cfg(test)] mod test_helpers`。

---

### 12. 通配符导入 `use crate::types::tool::*`

**受影响文件**: `tools/agent.rs`, `tools/grep.rs`, `tools/lsp.rs`, `tools/plan_mode.rs`, `tools/config_tool.rs` 等

**影响**: 降低依赖可见性，难以追踪实际使用了哪些类型。

---

### 13. 模型别名硬编码重复 — 3 处

同一组模型 ID 映射出现在:

| 位置 | 内容 |
|------|------|
| `tools/agent.rs:49-57` | `resolve_model_alias()` |
| `commands/model.rs:18-22` | 模型别名映射 |
| `config/constants.rs:26-68` | 3 个几乎相同的 if-else 链 (`marketing_name_for_model`, `knowledge_cutoff` 等) |

**建议**: 使用单一的 `ModelInfo` 查找表:
```rust
struct ModelInfo {
    canonical: &'static str,
    alias: &'static str,
    marketing_name: &'static str,
    knowledge_cutoff: &'static str,
}
static MODELS: &[ModelInfo] = &[ /* ... */ ];
```

---

### 14. 工具输入解析方式不一致

| 方式 | 使用的工具 |
|------|-----------|
| 手动 `fn parse_input()` | bash, file_read, glob_tool, file_write, file_edit |
| `call()` 中内联解析 | 多数小型工具 |
| serde 反序列化到结构体 | grep, agent, skill |

**影响**: 新工具作者无法参考一致的模式，增加出错概率。

---

### 15. `QueryDeps` trait 过于宽泛

**文件**: `src/query/deps.rs:82-147`

9 个方法混合了 4 种职责:
- 模型调用 (2 methods)
- 上下文压缩 (3 methods)
- 工具执行 (1 method)
- 状态访问 (3 methods)

**影响**: 测试时需要 mock 全部 9 个方法，即使只测试某一阶段。

**建议**: 拆分为 `ModelCaller`、`Compactor`、`ToolExecutor` 等独立 trait。

---

### 16. 字符串匹配做错误分类

**文件**: `src/api/retry.rs:69-71`

```rust
if body.contains("prompt is too long") || body.contains("too many tokens") {
    ApiErrorCategory::PromptTooLong
}
```

**影响**: API 返回消息格式变化（多语言、大小写、措辞调整）就会导致分类失败。

---

### 17. Google Provider 中的 `.unwrap()` 调用

**文件**: `src/api/google_provider.rs:149, 159, 160, 167`

对 JSON 数组/对象访问直接 `.unwrap()`，如果 API 返回格式异常会 panic。

---

### 18. IPC 协议无版本策略

**文件**: `src/ipc/protocol.rs`

- 后端消息使用泛型 `serde_json::Value`
- 无版本号字段
- 前后端可以在无感知的情况下协议不同步

---

### ~~19. 运行时正则编译~~ ✅ 已在第一批修复

> 4 处 `Regex::new().unwrap()` 已改为 `static LazyLock<Regex>`。

---

### 20. `ProcessState` 字段全部 pub 暴露

**文件**: `src/bootstrap/state.rs`

15+ 个字段直接 `pub`，无封装，鼓励跨模块边界的直接修改。types/ 下的大多数类型也有同样问题。

---

## 严重程度汇总

| 等级 | 总问题数 | 已修复 | 剩余 | 典型代表 |
|------|---------|--------|------|----------|
| **CRITICAL** | 4 | 2 ✅ | 2 | ~~巨型函数~~、生产代码 panic 26+ 处 (测试中) |
| **HIGH** | 5 | 1 ✅ | 4 | ~~mcp/client.rs~~、Arc 泛滥、全局状态、重试代码重复 |
| **MEDIUM** | 11 | 1 ✅ | 10 | ~~运行时正则~~、allow(unused)、测试样板重复 |

---

## 建议修复路线图

### ~~第一批: 安全性~~ — 已修复 {#已修复的问题}

> **勘误**: 原始审计将命令处理器和 API 客户端中的 `panic!()` 错误归类为生产代码，
> 实际上它们**全部在 `#[cfg(test)]` 测试模块中**。真正的生产代码问题是散布在多个
> 模块中的裸 `.unwrap()` 调用。

**已修复内容:**

| 修复 | 影响文件 | 变更 |
|------|---------|------|
| 运行时正则 → `LazyLock<Regex>` | `utils/bash.rs` | 4 处 `Regex::new().unwrap()` → 4 个 static LazyLock |
| `engine/lifecycle.rs` 54 处裸 `.unwrap()` | `engine/lifecycle.rs` | 全部替换为 `.expect("descriptive message")` |
| `utils/cwd.rs` 3 处裸 `.lock().unwrap()` | `utils/cwd.rs` | → `.lock().expect("CWD lock poisoned")` |
| `utils/abort.rs` 2 处裸 `.lock().unwrap()` | `utils/abort.rs` | → `.lock().expect("abort reason lock poisoned")` |
| `tools/tasks.rs` 5 处裸 `.lock().unwrap()` | `tools/tasks.rs` | → `.lock().expect("task store lock poisoned")` |
| `query/token_budget.rs` unsafe unwrap 模式 | `query/token_budget.rs` | `budget.unwrap()` → `match` 安全解构 |
| `services/prompt_suggestion.rs` NaN 风险 | `prompt_suggestion.rs` | `partial_cmp().unwrap()` → `.unwrap_or(Equal)` |
| `session/audit_export.rs` 2 处 | `audit_export.rs` | `last().unwrap()` → `last().map().unwrap_or_else()` |
| `tools/config_tool.rs` 1 处 | `config_tool.rs` | `.as_object_mut().unwrap()` → `.expect("guaranteed object")` |
| `tools/worktree.rs` 1 处 | `worktree.rs` | `session.unwrap()` → `.expect("guaranteed Some")` |

### ~~第二批: 可维护性~~ — 已修复

**已修复内容:**

| 修复 | 原始行数 | 拆分后 | 最大单文件 |
|------|---------|--------|-----------|
| `engine/lifecycle.rs` → `lifecycle/` 目录模块 (6 文件) | 1703 | 1689 | 637 (submit_message.rs) |
| `query/loop_impl.rs` → 核心 + helpers + tests (3 文件) | 1105 | 1073 | 451 (loop_impl.rs) |
| `mcp/client.rs` → client + transport + manager + tests (4 文件) | 1008 | 1005 | 502 (client.rs) |

### ~~第三批: 架构改善~~ — 大部分已修复

| 任务 | 状态 | 说明 |
|------|------|------|
| ~~合并 QueryEngine 的 10 个 Arc 字段~~ | ✅ | 已合并为 `QueryEngineState` struct (lifecycle/mod.rs) |
| 提取重试逻辑到 `RetryPolicy` | — | `retry.rs` 已有 `RetryConfig` + `categorize_api_error`，不再重复 |
| ~~重构 API 提供商为 trait 抽象~~ | ✅ | 已有 `trait StreamProvider` (stream_provider.rs) |

### 第四批: 代码卫生 (预计影响: 全局)

| 任务 | 影响文件数 | 复杂度 |
|------|-----------|--------|
| 清理 `#![allow(unused)]` | ~27 | 低 |
| 提取 `test_ctx()` 到共享模块 | ~13 | 低 |
| 统一模型别名到查找表 | 3-4 | 低 |
| 统一工具输入解析模式 | ~15 | 中 |
