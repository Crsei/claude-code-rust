# app_subsystem_handlers.rs 拆分重构方案

## 文件概览

**当前文件**: `crates/claude-code-rs/src/app_subsystem_handlers.rs` (2370 行)

**公共入口**: `main.rs` 中通过 `mod app_subsystem_handlers;` 声明，仅从 `app_runtime_adapters/mod.rs` 中的 `RootSubsystemHost` 调用。

**当前结构**：

| 段 | 行号 | 内容 | 类型 |
|---|------|------|------|
| 头部 | 1-18 | 文档注释、use 导入 | 模块级 |
| LSP 类型转换 | 24-62 | `lsp_diagnostic_to_ipc`、`lsp_document_change_from_ipc`、`lsp_server_info_to_ipc` | 私有函数 |
| LSP 命令处理 | 70-244 | `handle_lsp_command`、`spawn_lsp_task` | 1 公有 + 1 私有函数 |
| LSP 推荐设置 | 250-381 | `load_lsp_recommendation_settings`、`save_lsp_recommendation_settings`、`apply_recommendation_decision`、`install_lsp_recommendation_plugin`、`unmute_lsp_plugin`、`set_lsp_recommendations_disabled` | 1 公有 + 5 私有函数 |
| MCP 类型+运行时操作 | 383-568 | `McpRuntimeOperation`(enum)、`McpRuntimeReport`(struct)、`handle_mcp_command`、`handle_mcp_command_with_runtime`、`run_mcp_runtime_operation` | 2 公有 + 1 私有函数 + 2 类型 |
| MCP 命令处理（同步） | 570-712 | `handle_mcp_command_at_cwd` | 私有函数 |
| MCP OAuth | 714-798 | `start_mcp_auth`、`complete_mcp_auth`、`clear_mcp_auth`、`query_mcp_auth` | 私有函数 |
| MCP 辅助函数 | 800-874 | `mcp_config_error_message`、`mcp_runtime_unavailable_report`、`mcp_runtime_report`、`mcp_runtime_report_messages`、`find_mcp_runtime_config` | 私有函数 |
| Plugin 命令处理 | 880-951 | `handle_plugin_command` | 1 公有函数 |
| IDE 命令处理 | 959-1015 | `handle_ide_command` | 1 公有函数 |
| Skill 命令处理 | 1021-1072 | `handle_skill_command` | 1 公有函数 |
| 状态快照构建器 | 1078-1641 | `build_lsp_server_info_list` 到 `build_subsystem_status_snapshot` 共 12 个函数 | 7 公有 + 5 私有 |
| MCP 配置持久化 | 1297-1537 | `settings_path_for_scope`、`read_settings_value`、`write_settings_value`、`upsert_mcp_entry`、`remove_mcp_entry`、`toggle_mcp_entry_enabled`、`entry_to_settings_value` | 7 私有函数 |
| 测试 | 1647-2370 | `mod tests` | 测试模块 |

---

## 拆分方案

将当前文件按子系统拆分到 `app_subsystem_handlers/` 目录下，每个子系统一个子模块。

### 目标文件列表及职责

```
crates/claude-code-rs/src/
├── app_subsystem_handlers.rs      （缩减，只保留 re-export + mod 声明）
└── app_subsystem_handlers/
    ├── mod.rs                     （重导出所有子模块的公有 API）
    ├── lsp.rs                     （LSP 命令处理 + 推荐设置）
    ├── mcp.rs                     （MCP 命令处理 + 运行时操作 + OAuth）
    ├── mcp_config.rs              （MCP 配置持久化：scope 感知的 CRUD）
    ├── plugin.rs                  （Plugin 命令处理）
    ├── ide.rs                     （IDE 命令处理）
    ├── skill.rs                   （Skill 命令处理）
    └── snapshot.rs                （状态快照构建器，调用各子系统的构建器）
```

### 每个新文件的包含内容

#### 1. `app_subsystem_handlers/mod.rs`（重导出模块）

**职责**: 对外提供所有公有 API 的重导出，保持调用方 `app_runtime_adapters/mod.rs` 的 `crate::app_subsystem_handlers::xxx` 引用不变。

```rust
mod lsp;
mod mcp;
mod mcp_config;
mod plugin;
mod ide;
mod skill;
mod snapshot;

pub use lsp::*;
pub use mcp::*;
pub use mcp_config::*;
pub use plugin::*;
pub use ide::*;
pub use skill::*;
pub use snapshot::*;
```

---

#### 2. `app_subsystem_handlers/lsp.rs`（~380 行）

**职责**: LSP 相关的类型转换、命令处理、推荐设置持久化。

**包含内容**（从原文件移动）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `lsp_diagnostic_to_ipc` | 24-37 | 私有 |
| `lsp_document_change_from_ipc` | 39-52 | 私有 |
| `lsp_server_info_to_ipc` | 54-62 | 私有 |
| `handle_lsp_command` | 70-210 | `pub` |
| `spawn_lsp_task` | 212-244 | 私有 |
| `LSP_RECOMMENDATIONS_KEY` 常量 | 252 | 私有 |
| `load_lsp_recommendation_settings` | 259-268 | `pub` |
| `save_lsp_recommendation_settings` | 272-295 | 私有 |
| `apply_recommendation_decision` | 303-337 | 私有 |
| `install_lsp_recommendation_plugin` | 339-360 | 私有 |
| `unmute_lsp_plugin` | 363-371 | 私有 |
| `set_lsp_recommendations_disabled` | 374-381 | 私有 |

**依赖导入**:
- `cc_ipc_protocol::subsystem_events::{LspCommand, LspEvent}`
- `cc_ipc_protocol::subsystem_types::*`（特别是 `LspDiagnostic`, `LspServerInfo`, `LspRecommendationSettings`, `LspDiagnosticSnapshotEntry` 等）
- `cc_ipc_protocol::BackendMessage`
- `cc_lsp_service`（`LspDiagnostic`, `SourceRange`, `DocumentChange`, `LspServerInfo`, `LspEvent` 等）
- `cc_config::settings::user_settings_path`
- `cc_plugins::installation::install_plugin`
- `crate::command_runtime_bridge::*`

---

#### 3. `app_subsystem_handlers/mcp.rs`（~470 行）

**职责**: MCP 类型定义、运行时操作（connect/disconnect/reconnect）、同步/异步命令处理、OAuth 流程。

**包含内容**（从原文件移动）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `McpRuntimeOperation` struct | 383-388 | `pub` |
| `impl McpRuntimeOperation` | 390-406 | `pub` |
| `McpRuntimeReport` struct | 408-415 | `pub` |
| `handle_mcp_command` | 426-431 | `pub` |
| `handle_mcp_command_with_runtime` | 433-469 | `pub` |
| `run_mcp_runtime_operation` | 471-568 | `pub` |
| `handle_mcp_command_at_cwd` | 570-712 | 私有(模块内或 `pub(crate)`) |
| `start_mcp_auth` | 714-740 | 私有 |
| `complete_mcp_auth` | 742-767 | 私有 |
| `clear_mcp_auth` | 769-778 | 私有 |
| `query_mcp_auth` | 780-798 | 私有 |
| `mcp_config_error_message` | 800-807 | 私有（用在 mcp.rs 和 snapshot.rs）==需要考虑放 mod.rs 或改为 pub(crate)== |
| `mcp_runtime_unavailable_report` | 809-824 | 私有 |
| `mcp_runtime_report` | 826-841 | 私有 |
| `mcp_runtime_report_messages` | 843-857 | 私有 |
| `find_mcp_runtime_config` | 859-874 | 私有 |

**依赖导入**:
- `cc_ipc_protocol::subsystem_events::{McpCommand, McpEvent}`
- `cc_ipc_protocol::subsystem_types::*`（`McpServerStatusInfo`, `McpServerConfigEntry`, `ConfigScope` 等）
- `cc_ipc_protocol::BackendMessage`
- `cc_mcp`（`discovery`, `runtime`, `client`, `auth`, `manager`, `McpServerConfig` 等）

---

#### 4. `app_subsystem_handlers/mcp_config.rs`（~250 行）

**职责**: MCP 配置的作用域感知持久化（Scope-aware config editor，issue #44）。`mcp.rs` 中的 `handle_mcp_command_at_cwd` 会调用这里的函数。

**包含内容**（从原文件移动）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `settings_path_for_scope` | 1310-1323 | 私有 |
| `read_settings_value` | 1326-1336 | 私有（与 lsp.rs 共享 == 需考虑提取为模块级工具或 `pub(crate)`） |
| `write_settings_value` | 1339-1358 | 私有（同样可能与 lsp.rs 共享） |
| `upsert_mcp_entry` | 1364-1403 | `pub` |
| `remove_mcp_entry` | 1406-1446 | `pub` |
| `toggle_mcp_entry_enabled` | 1454-1510 | `pub` |
| `entry_to_settings_value` | 1517-1537 | 私有 |

**公共函数被 `mcp.rs` 中的 `handle_mcp_command_at_cwd` 调用**。

**关键依赖**: `cc_config::settings::user_settings_path`、`serde_json`。

---

#### 5. `app_subsystem_handlers/plugin.rs`（~70 行）

**职责**: Plugin 子系统命令处理。

**包含内容**（从原文件移动，行号 880-951）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `handle_plugin_command` | 880-951 | `pub` |

**依赖导入**:
- `cc_ipc_protocol::subsystem_events::{PluginCommand, PluginEvent}`
- `cc_ipc_protocol::subsystem_types::*`
- `cc_ipc_protocol::BackendMessage`
- `cc_plugins`

---

#### 6. `app_subsystem_handlers/ide.rs`（~55 行）

**职责**: IDE 子系统命令处理。

**包含内容**（从原文件移动，行号 959-1015）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `handle_ide_command` | 959-1015 | `pub` |

**依赖导入**:
- `cc_ipc_protocol::subsystem_events::{IdeCommand, IdeEvent}`
- `cc_ipc_protocol::subsystem_types::*`
- `cc_ipc_protocol::BackendMessage`
- `cc_lsp_service::ide`

---

#### 7. `app_subsystem_handlers/skill.rs`（~55 行）

**职责**: Skill 子系统命令处理。

**包含内容**（从原文件移动，行号 1021-1072）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `handle_skill_command` | 1021-1072 | `pub` |

**依赖导入**:
- `cc_ipc_protocol::subsystem_events::{SkillCommand, SkillEvent}`
- `cc_ipc_protocol::subsystem_types::*`
- `cc_ipc_protocol::BackendMessage`
- `cc_skills`
- `cc_config::paths`

---

#### 8. `app_subsystem_handlers/snapshot.rs`（~560 行）

**职责**: 所有子系统的状态快照构建函数。

**包含内容**（从原文件移动）：

| 条目 | 行号范围 | 可见性 |
|------|---------|--------|
| `build_lsp_server_info_list` | 1079-1084 | `pub` |
| `build_mcp_server_info_list` | 1087-1090 | `pub` |
| `build_mcp_server_info_list_for_cwd` | 1092-1108 | `pub` (同步版本，仍被使用) |
| `build_mcp_server_info_list_for_cwd_async` | 1110-1125 | `pub` |
| `mcp_discovery_error_status` | 1127-1138 | 私有 |
| `discover_mcp_runtime_configs_with_diagnostics` | 1140-1177 | 私有（模块内部） |
| `build_mcp_server_info_list_from_configs` | 1179-1187 | 私有 |
| `build_mcp_server_info` | 1189-1243 | 私有 |
| `build_mcp_server_config_entries` | 1249-1285 | `pub` |
| `scope_from_discovery` | 1288-1295 | 私有 |
| `build_plugin_info_list` | 1540-1564 | `pub` |
| `discover_plugin_skills_for_handlers` | 1566-1594 | 私有（被 `handle_skill_command` 调用 == 需要 `pub(crate)`） |
| `build_skill_info_list` | 1597-1619 | `pub` |
| `build_ide_info_list` | 1627-1629 | `pub` |
| `build_subsystem_status_snapshot` | 1632-1641 | `pub` |

**关键依赖**: 需要导入 `lsp.rs` 中的 `lsp_server_info_to_ipc`（当前为私有，需改为 `pub(super)`）、或将其内联/提取到 snapshot 中。`discover_plugin_skills_for_handlers` 被 `skill.rs` 中的 `handle_skill_command` 调用。

---

### 模块间依赖关系

```
                      ┌────────────────┐
                      │   mod.rs       │  (重导出)
                      └───────┬────────┘
            ┌─────────────────┼────────────────────┐
            │                 │                      │
     ┌──────▼──────┐  ┌──────▼──────┐   ┌──────────▼──────────┐
     │ lsp.rs      │  │ mcp.rs      │   │ snapshot.rs         │
     │             │  │             │   │                     │
     │ - handle_   │  │ - handle_   │   │ - build_lsp_*       │
     │   lsp_*     │  │   mcp_*     │   │ - build_mcp_*       │  → 调用 mcp.rs 的
     │ - load_lsp_*│  │ - run_mcp_* │   │ - build_plugin_*    │    mcp_config_error_message
     │             │  │ - auth_*    │   │ - build_skill_*     │    → 调用 lsp.rs 的
     └──────┬──────┘  │             │   │ - build_ide_*       │    lsp_server_info_to_ipc
            │         │             │   │ - discover_plugin_  │
            │         └──────┬──────┘   │   skills_for_*      │  → 被 skill.rs 调用
            │                │          └──────────────────────┘
            │         ┌──────▼──────┐         │
            │         │ mcp_config   │         │
            │         │ .rs          │         │
            │         │              │         │
            │         │ - upsert_*   │         │
            │         │ - remove_*   │         │
            │         │ - toggle_*   │         │
            │         └─────────────┘         │
     ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐
     │ plugin.rs   │  │ ide.rs      │  │ skill.rs    │
     │             │  │             │  │             │
     │ - handle_   │  │ - handle_   │  │ - handle_   │
     │   plugin_*  │  │   ide_*     │  │   skill_*   │→ 调用 snapshot 的
     └─────────────┘  └─────────────┘  │   discover_ │
                                       │   plugin_   │
                                       │   skills_*  │
                                       └─────────────┘
```

**需要解决的跨模块共享问题**:

1. `lsp_server_info_to_ipc`（lsp.rs 私有）→ `snapshot.rs` 中的 `build_lsp_server_info_list` 调用它。方案：改为 `pub(super)` 或 `pub(crate)`。
2. `mcp_config_error_message`（mcp.rs 私有）→ `snapshot.rs` 中的 `build_mcp_server_config_entries` 可能参考它，且 `mcp.rs` 中的 OAuth 和命令处理也调用它。方案：保持 `pub(crate)` 或将 MCP 错误函数集中在 mcp.rs 中让 snapshot 通过模块引用。
3. `discover_plugin_skills_for_handlers`（snapshot.rs）→ `skill.rs` 中的 `handle_skill_command` 调用它。方案：保留在 snapshot.rs 中并声明为 `pub(super)`。
4. `read_settings_value` / `write_settings_value` 被 `lsp.rs` 和 `mcp_config.rs` 共享。方案：提取到一个 `util.rs` 子模块，或复制简化版（推荐提取）。
5. 测试模块 `mod tests` 当前包含对所有子系统的集成测试。拆分后需要按子系统分散到各子模块的 `#[cfg(test)]` 中。

---

## 重构步骤（迁移顺序）

### 第 1 步：创建目录和空文件

```bash
mkdir -p crates/claude-code-rs/src/app_subsystem_handlers
touch crates/claude-code-rs/src/app_subsystem_handlers/mod.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/lsp.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/mcp.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/mcp_config.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/plugin.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/ide.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/skill.rs
touch crates/claude-code-rs/src/app_subsystem_handlers/snapshot.rs
```

### 第 2 步：迁移独立子模块（零外部依赖，或仅依赖标准库/IPC 协议）

优先级：`plugin.rs` → `ide.rs` → `skill.rs`

这三个子模块的 `handle_*_command` 函数不依赖其他子模块的内容，是纯函数式调度。可以安全地先迁移。

- 将对应的代码块（含私有辅助函数）剪贴到各文件。
- 为每个文件添加所需的 `use` 导入。
- 为该子模块的测试（原 `mod tests` 中对应的 `#[test]` 块）剪贴到各文件内部的 `#[cfg(test)] mod tests` 中。

### 第 3 步：迁移 `lsp.rs`（包含推荐设置持久化）

- 将 LSP 转换函数、`handle_lsp_command`、`spawn_lsp_task`、推荐设置全部移到 `lsp.rs`。
- 注意：`lsp_server_info_to_ipc` 虽然只被 `snapshot.rs` 调用，但它是 LSP 类型映射的逻辑归属。先声明为 `pub(super)`。
- 迁移对应的测试。

### 第 4 步：迁移 `mcp.rs`

- 将 `McpRuntimeOperation`、`McpRuntimeReport`、`handle_mcp_command`、`handle_mcp_command_with_runtime`、`run_mcp_runtime_operation`、`handle_mcp_command_at_cwd` 以及所有 OAuth 辅助函数移到 `mcp.rs`。
- 将 `mcp_config_error_message` 保留为 `pub(crate)`（因为 snapshot 也会用到）。
- 注意：`handle_mcp_command_at_cwd` 调用了 `mcp_config.rs` 中的 CRUD 函数，需确保 `use` 路径正确。

### 第 5 步：迁移 `mcp_config.rs`

- 将 MCP 配置 CRUD（`settings_path_for_scope`、`read_settings_value`、`write_settings_value`、`upsert_mcp_entry`、`remove_mcp_entry`、`toggle_mcp_entry_enabled`、`entry_to_settings_value`）移到 `mcp_config.rs`。
- 保留所有函数为 `pub(super)` 或 `pub(crate)`（被 `mcp.rs` 调用）。

### 第 6 步：提取共享工具到 `util.rs`（可选但推荐）

- 将 `read_settings_value` 和 `write_settings_value` 从 `lsp.rs` 和 `mcp_config.rs` 中提取到 `mod.rs` 或独立的 `util.rs` 子模块。
- `lsp.rs` 和 `mcp_config.rs` 都通过 `use super::util::*` 引用。

### 第 7 步：迁移 `snapshot.rs`

- 将所有 `build_*` 函数移到 `snapshot.rs`。
- 解决跨模块引用：
  - `lsp_server_info_to_ipc` → 从 `super::lsp` 或声明 `pub(crate)` 后直接 `use super::lsp::*` 引用。
  - `discover_plugin_skills_for_handlers` → 声明为 `pub(super)`。
  - `mcp_config_error_message` → 从 `super::mcp` 引用。
- 迁移对应的测试。

### 第 8 步：重写根 `app_subsystem_handlers.rs`

将原文件内容替换为：

```rust
//! Subsystem command handlers and status snapshot builders.
//!
//! This file is split into submodules under `app_subsystem_handlers/`.
//! Each subsystem (LSP, MCP, Plugin, IDE, Skill) has its own module.
//! Snapshot builders live in `snapshot.rs`.
//! MCP config persistence lives in `mcp_config.rs`.

mod lsp;
mod mcp;
mod mcp_config;
mod plugin;
mod ide;
mod skill;
mod snapshot;

pub use lsp::*;
pub use mcp::*;
pub use mcp_config::*;
pub use plugin::*;
pub use ide::*;
pub use skill::*;
pub use snapshot::*;
```

### 第 9 步：验证和测试

```bash
cargo build --workspace --release
cargo test -p claude-code-rs --test '*' 2>&1 | head -100
```

---

## 注意事项

### 1. 测试模块的处理

当前原始文件末尾（行 1647-2370）包含一个巨大的 `#[cfg(test)] mod tests`，其中混合了 LSP、MCP、Plugin、Skill 的测试。拆分方案：

- **每个子模块的测试**：将与子模块直接相关的测试移至该模块的 `#[cfg(test)] mod tests` 中。
- **跨模块集成测试**（例如测试需要 `build_mcp_server_info_list` 和 `handle_mcp_command` 一起验证的）建议放在 `mod.rs` 的 `#[cfg(test)]` 中或 `snapshot.rs` 中。

具体测试分配：
| 原测试函数 | 行号 | 归属 |
|-----------|------|------|
| `build_lsp_server_info_list_returns_configured_servers` | 1652 | lsp.rs |
| `build_lsp_server_info_list_has_dotted_extensions` | 1661 | lsp.rs |
| `build_mcp_server_info_list_defaults_to_pending` | 1672 | mcp.rs |
| `build_plugin_info_list_maps_status` | 1681 | plugin.rs |
| `build_skill_info_list_returns_skills` | 1734 | skill.rs |
| `build_skill_info_list_maps_sources` | 1757 | skill.rs |
| `build_subsystem_status_snapshot_has_timestamp` | 1801 | snapshot.rs |
| `handle_lsp_query_status_returns_server_list` | 1810 | lsp.rs |
| `handle_lsp_start_returns_info` | 1818 | lsp.rs |
| `apply_recommendation_decision_no_is_noop` | 1828 | lsp.rs |
| `apply_recommendation_decision_unknown_is_warned_but_silent` | 1835 | lsp.rs |
| `apply_recommendation_decision_yes_attempts_install` | 1841 | lsp.rs |
| `handle_mcp_query_status_returns_server_list` | 1853 | mcp.rs |
| `upsert_mcp_entry_persists_to_user_scope` | 1909 | mcp_config.rs |
| `upsert_mcp_entry_persists_to_project_scope` | 1944 | mcp_config.rs |
| `upsert_mcp_entry_rejects_plugin_scope` | 1974 | mcp_config.rs |
| `remove_mcp_entry_round_trips_user_scope` | 2002 | mcp_config.rs |
| `remove_mcp_entry_rejects_plugin_scope` | 2036 | mcp_config.rs |
| `remove_mcp_entry_errors_on_missing_file` | 2049 | mcp_config.rs |
| `handle_mcp_upsert_config_emits_config_changed` | 2063 | mcp_config.rs |
| `handle_mcp_upsert_config_on_read_only_emits_config_error` | 2103 | mcp_config.rs |
| `handle_mcp_query_config_returns_config_list` | 2134 | mcp_config.rs |
| `toggle_mcp_entry_enabled_flips_disabled_flag` | 2148 | mcp_config.rs |
| `toggle_mcp_entry_enabled_rejects_plugin_scope` | 2190 | mcp_config.rs |
| `handle_mcp_toggle_enabled_emits_config_changed_and_state` | 2210 | mcp_config.rs |
| `handle_mcp_reconnect_uses_runtime_manager_and_emits_final_state` | 2271 | mcp.rs |
| `build_mcp_server_config_entries_tags_user_scope` | 2330 | mcp_config.rs |
| `handle_plugin_query_status_returns_plugin_list` | 2356 | plugin.rs |
| `handle_skill_query_status_returns_skill_list` | 2364 | skill.rs |

### 2. 测试辅助类型的处理

`EnvGuard`（行 1867-1887）和 `RuntimeMcpGuard`（行 1889-1905）被 `mcp_config.rs` 和 `mcp.rs` 的测试共享。建议：

- 放在 `mod.rs` 的 `#[cfg(test)]` 中并声明为 `pub(super)`。
- 或放在 `mcp_config.rs` 中并声明为 `pub(crate)`（仅对测试可见）。

### 3. 验证测试隔离性

使用 `#[serial_test::serial]` 的测试需要在同一线程串行运行。跨模块串行测试需要在 cargo 配置中确保单线程执行或保持 `serial_test` 工作正常。

### 4. 避免测试中路径重写

`#[cfg(test)]` 中的 `use super::*;` 在子模块中变为 `use super::super::*;`，需统一处理为 `use crate::*` 或 `use super::*` + 子模块重导出。

### 5. 命名冲突风险

`build_mcp_server_info_list` 等函数如果被多个子模块使用，需要确保 `use` 链不发生歧义。由于统一通过 `mod.rs` 重导出，调用方只需写 `crate::app_subsystem_handlers::build_mcp_server_info_list()`，与重构前完全一致。
