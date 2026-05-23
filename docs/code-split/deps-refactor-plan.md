# `deps.rs` 重构拆分方案

> 目标文件: `crates/cc-engine/src/lifecycle/deps.rs` (3147 行)
> 生成日期: 2026-05-23

---

## 1. 文件概览（当前结构分析）

`deps.rs` 是 `QueryEngineDeps` 结构体的核心实现文件，它实现了 `QueryDeps` trait，将查询循环（query loop）与引擎共享状态（abort flag、app state、tools）和可选的 `ApiClient` 连接起来。

### 当前代码组成

| 代码区域 | 行号范围 | 说明 |
|---------|---------|------|
| 模块文档 + use 声明 | 1–41 | 顶层文档注释及导入 |
| `QueryEngineDeps` struct | 46–87 | 核心依赖注入桥接结构体，包含 17 个字段 |
| 自动压缩（auto-compact）辅助函数 | 89–110 | `auto_compact_trigger_tracking`, `exact_auto_compact_triggered` |
| **权限决策核心函数** | 112–227 | `central_permission_decision_for_tool`, `auto_classifier_needed`, `permission_result_from_decision`, `permission_behavior_label`, `permission_reason_summary` |
| **权限事件发射函数** | 229–303 | `emit_permission_decision_debug`, `emit_hook_permission_decision`, `central_permission_result_for_tool` (#[cfg(test)]) |
| Hook 错误 / 权限消息辅助 | 305–350 | `hook_error_is_critical`, `permission_denied_message`, `permission_feedback_message`, `tool_execution_result_to_exec_result` |
| MCP 工具合并 | 352–372 | `merge_refreshed_mcp_tools` |
| 模型调用参数准备 | 374–451 | `prepare_model_call_params_for_client`, `model_for_autocompact`, `build_auto_compact_exact_count_request`, `record_request_snapshot` |
| Auto-mode 白名单常量 | 453–467 | `AUTO_MODE_ALLOWLISTED_TOOLS` |
| `impl QueryEngineDeps` (私有方法) | 469–506 | `compute_auto_classifier` |
| **`impl QueryDeps for QueryEngineDeps`** | 508–1820 | 核心 trait 实现，包含以下方法: |
| - `tool_progress_callback` | 510–512 | 进度回调 getter |
| - `call_model` | 514–561 | 同步模型调用 |
| - `call_model_streaming` | 563–614 | 流式模型调用 |
| - `microcompact` | 616–625 | 微压缩 |
| - `autocompact` | 627–847 | 自动压缩（~220 行） |
| - `reactive_compact` | 849–875 | 响应式压缩 |
| - `collapse_drain` | 877–921 | 上下文折叠 |
| - **`execute_tool`** | 927–1759 | **核心工具执行方法（~832 行）** |
| - `get_app_state` | 1761–1763 | 状态 getter |
| - `uuid` | 1765–1767 | UUID 生成 |
| - `is_aborted` | 1769–1771 | 检查 abort |
| - `get_tools` | 1773–1775 | 工具 getter |
| - `refresh_tools` | 1777–1797 | MCP 工具刷新 |
| - `drain_background_results` | 1799–1801 | 后台结果排空 |
| - `hook_runner` | 1803–1805 | Hook runner getter |
| - `audit_context` / `langfuse_*` | 1807–1819 | 观测性 getter |
| **`#[cfg(test)] mod tests`** | 1822–3147 | 测试模块（~1325 行） |

### 主要问题

1. **`execute_tool` 方法过长**（~832 行），包含了权限检查、Hook 运行、安全验证、Audit 事件发射、结果后处理等多个关注点。
2. 权限相关辅助函数（~10 个独立函数）与自动压缩逻辑混在同一个文件。
3. 模型调用参数准备逻辑与 MCP 工具合并逻辑无关联但同处一处。
4. 测试模块占据 42% 的文件（1325/3147 行），应独立存放。

---

## 2. 拆分方案（目标文件列表及职责）

建议将 `deps.rs` 拆分为 **6 个新文件** + **1 个保留文件**:

```
lifecycle/
├── mod.rs              (已有，不需要修改)
├── deps/
│   ├── mod.rs          ← struct QueryEngineDeps + re-exports
│   ├── permission.rs   ← 权限决策、事件发射、辅助函数
│   ├── execute.rs      ← execute_tool 方法（从 QueryDeps impl 中提取）
│   ├── autocompact.rs  ← autocompact / reactive_compact / collapse_drain + 辅助
│   ├── model_call.rs   ← call_model / call_model_streaming + 参数准备
│   └── tests/
│       └── mod.rs      ← 原 #[cfg(test)] mod tests 的全部内容
├── helpers.rs          (已有，不动)
├── submit_message.rs   (已有，不动)
├── types.rs            (已有，不动)
└── tests/              (已有，不动)
```

### 各文件职责说明

| 文件 | 职责 |
|------|------|
| `deps/mod.rs` | `QueryEngineDeps` 结构体定义 + `QueryDeps` trait 实现骨架（委托到子模块） |
| `deps/permission.rs` | 权限决策链路：`central_permission_decision_for_tool`、`permission_result_from_decision`、事件发射、Hook 权限交互 |
| `deps/execute.rs` | `execute_tool` 方法：输入验证 → Hook → 权限检查 → 工具调用 → 后置 Hook → 审计 |
| `deps/autocompact.rs` | 自动压缩管道：`autocompact`、`reactive_compact`、`collapse_drain`、`microcompact` 及相关辅助函数 |
| `deps/model_call.rs` | 模型 API 调用：`call_model`、`call_model_streaming`、参数准备、请求快照、MCP 工具合并、`refresh_tools` |
| `deps/tests/mod.rs` | 全部测试代码（测试 helper structs + test functions） |

---

## 3. 每个新文件的包含内容（具体到 struct/fn/impl 及行号范围）

### 3.1 `deps/mod.rs`

> 行号范围参照原文件

| 内容 | 原文件行号 | 说明 |
|------|-----------|------|
| 模块文档 + use 声明 | 1–41 | 保留并调整 imports |
| `QueryEngineDeps` struct | 46–87 | 保持不变 |
| `AUTO_MODE_ALLOWLISTED_TOOLS` const | 453–467 | 移入此处（被 `permission.rs` 和 `execute.rs` 共用） |
| `impl QueryEngineDeps` (compute_auto_classifier) | 469–506 | 移入此处 |
| `impl QueryDeps for QueryDeps` trait 实现骨架 | — | 新增，将各方法委托到子模块函数 |

`mod.rs` 中将声明并 re-export 子模块：

```rust
mod autocompact;
mod execute;
mod model_call;
mod permission;
#[cfg(test)]
mod tests;

// Re-export for external visibility
pub(crate) use permission::{
    central_permission_decision_for_tool,
    emit_hook_permission_decision,
    permission_result_from_decision,
};
```

`impl QueryDeps for QueryEngineDeps` 中的短方法（`get_app_state`、`uuid`、`is_aborted`、`get_tools`、`drain_background_results`、`hook_runner`、`audit_context`、`langfuse_trace`、`langfuse_provider_name`、`tool_progress_callback`）保留在 `mod.rs` 中（行号 1761–1819）。

---

### 3.2 `deps/permission.rs`

| 内容 | 原文件行号 | 类型 |
|------|-----------|------|
| `central_permission_decision_for_tool()` | 112–159 | `pub(crate) fn` |
| `auto_classifier_needed()` | 161–169 | `pub(crate) fn` |
| `permission_result_from_decision()` | 171–195 | `pub(crate) fn` |
| `permission_behavior_label()` | 197–205 | `fn` (私有) |
| `permission_reason_summary()` | 207–227 | `fn` (私有) |
| `emit_permission_decision_debug()` | 229–262 | `pub(crate) fn` |
| `emit_hook_permission_decision()` | 264–284 | `pub(crate) fn` |
| `central_permission_result_for_tool()` | 287–303 | `#[cfg(test)] pub fn` |
| `hook_error_is_critical()` | 305–316 | `pub(crate) fn` |
| `permission_denied_message()` | 318–323 | `pub(crate) fn` |
| `permission_feedback_message()` | 325–335 | `pub(crate) fn` |

预估行数: ~230 行

---

### 3.3 `deps/execute.rs`

| 内容 | 原文件行号 | 类型 |
|------|-----------|------|
| `execute_tool()` 方法 | 927–1759 | `impl QueryEngineDeps` 中的 `pub(crate) async fn` |

此文件将 `execute_tool` 从 `impl QueryDeps for QueryEngineDeps` 中提取为 `impl QueryEngineDeps` 的关联函数，然后在 `deps/mod.rs` 中的 trait 实现里调用 `self.execute_tool(...)`。

预估行数: ~835 行

> **注意**: 虽然此文件仍然较长，但 `execute_tool` 本身是一个高内聚的方法，内部不建议再拆分（否则会破坏控制流的可读性）。未来可考虑将 Hook 运行逻辑提取为独立函数。

---

### 3.4 `deps/autocompact.rs`

| 内容 | 原文件行号 | 类型 |
|------|-----------|------|
| `auto_compact_trigger_tracking()` | 89–103 | `fn` (私有) |
| `exact_auto_compact_triggered()` | 105–110 | `pub(crate) fn` |
| `build_auto_compact_exact_count_request()` | 422–432 | `fn` (私有) |
| `microcompact()` 方法 | 616–625 | `impl QueryEngineDeps` async fn |
| `autocompact()` 方法 | 627–847 | `impl QueryEngineDeps` async fn |
| `reactive_compact()` 方法 | 849–875 | `impl QueryEngineDeps` async fn |
| `collapse_drain()` 方法 | 877–921 | `impl QueryEngineDeps` async fn |

预估行数: ~300 行

---

### 3.5 `deps/model_call.rs`

| 内容 | 原文件行号 | 类型 |
|------|-----------|------|
| `prepare_model_call_params_for_client()` | 374–396 | `pub(crate) fn` |
| `model_for_autocompact()` | 398–420 | `fn` (私有) |
| `record_request_snapshot()` | 434–451 | `fn` (私有) |
| `merge_refreshed_mcp_tools()` | 352–372 | `pub(crate) fn` |
| `tool_execution_result_to_exec_result()` | 337–350 | `pub(crate) fn` |
| `call_model()` 方法 | 514–561 | `impl QueryEngineDeps` async fn |
| `call_model_streaming()` 方法 | 563–614 | `impl QueryEngineDeps` async fn |
| `refresh_tools()` 方法 | 1777–1797 | `impl QueryEngineDeps` async fn |

预估行数: ~300 行

---

### 3.6 `deps/tests/mod.rs`

| 内容 | 原文件行号 | 类型 |
|------|-----------|------|
| 测试辅助 structs (`CanonicalTool`, `FailingTool` 等) | 1834–2056 | test helpers |
| Hook Runner mocks (`FailingPreToolHookRunner`, `FailingPostToolHookRunner`) | 2058–2228 | test mocks |
| MCP tool helper (`mcp_tool`) | 2216–2227 | test helper |
| 全部 `#[test]` / `#[tokio::test]` 函数 | 2229–3147 | test functions |

预估行数: ~1325 行

---

## 4. 模块间依赖关系

```
deps/mod.rs
├── deps/permission.rs   (mod.rs import 权限函数用于 trait 实现骨架)
├── deps/execute.rs      (mod.rs 中 trait 的 execute_tool 委托到此处)
│   ├── deps/permission.rs  (execute_tool 调用权限决策函数)
│   └── deps/model_call.rs  (execute_tool 使用 tool_execution_result_to_exec_result)
├── deps/autocompact.rs  (mod.rs 中 trait 的 autocompact/reactive/collapse 委托到此处)
│   └── deps/model_call.rs  (autocompact 调用 prepare_model_call_params_for_client, model_for_autocompact)
├── deps/model_call.rs   (mod.rs 中 trait 的 call_model/refresh_tools 委托到此处)
└── deps/tests/mod.rs    (测试代码 import 各子模块的 pub(crate) 函数)
```

**依赖方向**:
- `permission.rs` 和 `model_call.rs` 是**叶子模块**，不依赖其他子模块。
- `autocompact.rs` 依赖 `model_call.rs`（使用 `model_for_autocompact`、`prepare_model_call_params_for_client`）。
- `execute.rs` 依赖 `permission.rs`（权限决策）和 `model_call.rs`（`tool_execution_result_to_exec_result`）。
- `mod.rs` 依赖所有子模块（组装 trait 实现）。

---

## 5. 重构步骤（迁移顺序）

### 步骤 1: 创建目录结构

```
mkdir -p crates/cc-engine/src/lifecycle/deps/tests
```

### 步骤 2: 迁移测试代码（最低风险）

1. 将 `deps.rs` 中 `#[cfg(test)] mod tests` 块（行 1822–3147）提取到 `deps/tests/mod.rs`。
2. 将 `deps.rs` 转为 `deps/mod.rs`，添加 `#[cfg(test)] mod tests;`。
3. 验证编译通过、测试全部通过。

### 步骤 3: 迁移 `permission.rs`

1. 创建 `deps/permission.rs`，将权限相关函数（行 112–335）移入。
2. 在 `deps/mod.rs` 中添加 `mod permission;` + `use` 重新导出。
3. 确保 `execute_tool` 方法能正确引用这些函数。
4. 验证编译通过。

### 步骤 4: 迁移 `model_call.rs`

1. 创建 `deps/model_call.rs`，将模型调用相关函数（行 337–451、514–614、1777–1797）移入。
2. 在 `deps/mod.rs` 中添加 `mod model_call;` + `use`。
3. 验证编译通过。

### 步骤 5: 迁移 `autocompact.rs`

1. 创建 `deps/autocompact.rs`，将压缩相关函数（行 89–110、422–432、616–921）移入。
2. 在 `deps/mod.rs` 中添加 `mod autocompact;`。
3. 验证编译通过。

### 步骤 6: 迁移 `execute.rs`

1. 创建 `deps/execute.rs`，将 `execute_tool` 方法（行 927–1759）提取为 `impl QueryEngineDeps` 的关联函数。
2. 在 `deps/mod.rs` 中添加 `mod execute;`。
3. trait 实现中改为 `self.execute_tool(request, tools, parent_message, on_progress).await`。
4. 验证编译通过、测试全部通过。

### 步骤 7: 清理 `deps/mod.rs`

1. 确认 `mod.rs` 只保留：结构体定义、`compute_auto_classifier`、trait 实现骨架（委托）、短 getter 方法。
2. 清理多余的 `use` 语句。
3. 运行完整测试套件。

---

## 6. 注意事项

### 6.1 可见性变更

- 原文件中许多函数是模块私有 (`fn`)，拆分后如果被其他子模块引用，需要提升为 `pub(crate)`。
- 特别注意: `auto_compact_trigger_tracking`、`exact_auto_compact_triggered`、`build_auto_compact_exact_count_request`、`prepare_model_call_params_for_client`、`model_for_autocompact`、`record_request_snapshot`、`tool_execution_result_to_exec_result`、`merge_refreshed_mcp_tools` 等函数的可见性需要根据实际跨子模块调用来决定。

### 6.2 `impl` 块的拆分

- Rust 允许对同一类型有多个 `impl` 块。
- `execute.rs`、`autocompact.rs`、`model_call.rs` 中的方法将以 `impl QueryEngineDeps { ... }` 块的形式存在。
- `mod.rs` 中保留 `impl QueryDeps for QueryEngineDeps` 的 trait 实现，各方法委托到对应子模块的关联函数。
- **模式**:

```rust
// deps/mod.rs
#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
    async fn execute_tool(&self, ...) -> Result<ToolExecResult> {
        self.execute_tool_impl(request, tools, parent_message, on_progress).await
    }
    async fn autocompact(&self, ...) -> Result<Option<CompactionResult>> {
        self.autocompact_impl(params, tracking).await
    }
    // ...
}
```

### 6.3 测试中的 `use super::*`

- 原测试模块使用 `use super::*` 访问所有模块级函数。
- 迁移到 `deps/tests/mod.rs` 后，需要改为 `use super::*`（仍指向 `deps/mod.rs`）。
- 需要在 `deps/mod.rs` 中将测试需要的函数 re-export 到模块级别，或在测试中显式 `use super::permission::*` 等。
- **建议**: 在 `deps/mod.rs` 中用 `pub(crate) use` 将关键函数 re-export，保持测试代码尽量少改。

### 6.4 常量位置

- `AUTO_MODE_ALLOWLISTED_TOOLS`（行 453–467）在 `compute_auto_classifier`（`mod.rs`）和 `central_permission_decision_for_tool`（`permission.rs`）中都可能被引用。建议放在 `mod.rs` 或 `permission.rs` 中并 re-export。

### 6.5 外部调用者

- 检查 `lifecycle/mod.rs`、`lifecycle/submit_message.rs` 等文件对 `deps` 模块中函数的引用。
- 拆分后应保持 `deps` 模块的公共 API 不变（通过 `mod.rs` 中的 re-export），避免影响外部代码。

### 6.6 编译验证

- 每一步迁移后必须运行:
  ```bash
  export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
  export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
  export PATH="$CARGO_HOME/bin:$PATH"
  cargo build --workspace --release 2>&1 | grep -i warning
  cargo test -p cc-engine --lib lifecycle::deps
  ```
- 确保零 warning、全部测试通过后再进行下一步。

### 6.7 后续优化方向

- `execute_tool`（~832 行）内部可进一步提取:
  - Hook 前置/后置运行逻辑 → `execute_hooks.rs`
  - 权限检查完整流程（含 interactive prompt、audit 事件）→ 与 `permission.rs` 合并
  - Audit 事件发射 → `execute_audit.rs`
- 但当前阶段不建议过度拆分，保持 `execute_tool` 为单个方法是合理的（控制流清晰）。
