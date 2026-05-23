# `read_only_shell.rs` 重构拆分方案

## 一、文件概览（当前结构分析）

**文件路径**: `crates/cc-permissions/src/read_only_shell.rs`
**总行数**: 2320 行
**职责**: 只读 Shell 命令验证——判断一条 shell 命令是否为只读（Plan 模式 / Explore 模式下免审批执行）

### 当前内容分布

| 区段 | 行号范围 | 内容 | 行数 |
|------|----------|------|------|
| 模块文档 + imports | 1–19 | 文件级文档注释、`use` 语句 | 19 |
| 类型定义 | 26–72 | `FlagArgType` enum + `ExternalCommandConfig` struct + impl | 47 |
| 工具函数 | 78–90 | `map_from_pairs()`, `validate_flag_arg()` | 13 |
| Git flag 分组函数 | 96–165 | `git_ref_selection_flags()`, `git_date_filter_flags()`, `git_log_display_flags()`, `git_count_flags()`, `git_stat_flags()`, `git_color_flags()`, `git_patch_flags()`, `git_author_filter_flags()` | 70 |
| **Git 命令配置** | 171–733 | `make_git_read_only_commands()` — git diff/log/show/shortlog/reflog/stash/ls-remote/status/branch/ls-files/describe/blame/grep/config/tag/help/remote | 563 |
| **GH 命令配置** | 739–944 | `make_gh_read_only_commands()` — gh root/pr view/list/issue list/view/run list/view/search prs/search issues/api | 206 |
| **Docker 命令配置** | 950–1065 | `make_docker_read_only_commands()` — docker ps/images/inspect/network ls/volume ls/logs/info/version/stats | 116 |
| **Ripgrep 命令配置** | 1071–1203 | `make_rg_read_only_commands()` — rg 全量 flag | 133 |
| **Pyright 命令配置** | 1209–1239 | `make_pyright_read_only_commands()` — pyright | 31 |
| **外部通用命令白名单** | 1245–1343 | `make_external_readonly_commands()` — ls/cat/rg/man 等 80+ 条命令 | 99 |
| Flag 验证引擎 | 1351–1469 | `is_flag()`, `validate_flags()` | 119 |
| UNC 路径漏洞检测 | 1482–1558 | `contains_vulnerable_unc_path()` | 77 |
| Shell 文本分类 | 1569–1693 | `is_read_only_bash_command()`, `is_read_only_powershell_command()`, `classify_shell_command_text()`, `forbidden_shell_syntax()` | 125 |
| 命令名匹配 | 1703–1733 | `match_readonly_command()` | 31 |
| 主入口函数 | 1747–1879 | `is_read_only_shell_command()` — 路由到各 tool map | 133 |
| 合并辅助函数 | 1885–1891 | `merge_maps()` | 7 |
| **单元测试** | 1897–2320 | `mod tests` — 覆盖 git/gh/docker/rg/pyright/外部/shell文本/UNC | 424 |

### 关键问题

1. **单一文件过大** (2320 行)，违反单一职责原则。
2. **命令配置数据**占 ~1148 行 (49.5%)，与验证逻辑混合。
3. **6 个 `make_*_commands()` 函数**模式完全相同，适合作为独立数据模块。
4. **Git flag 分组** (70 行) 仅被 Git 命令配置使用，应随 Git 配置迁移。
5. **测试占 424 行 (18.3%)**，应在拆分后按模块归属。

---

## 二、拆分方案（目标文件列表及职责）

建议将 `read_only_shell.rs` 拆分为 **7 个文件**，以 `read_only_shell/` 子模块目录形式组织：

```
crates/cc-permissions/src/read_only_shell/
├── mod.rs              -- 模块声明 + 公开 API 重导出
├── types.rs            -- 核心类型定义 (FlagArgType, ExternalCommandConfig)
├── flag_validation.rs  -- Flag 验证引擎 + 辅助函数
├── shell_classifier.rs -- Shell 文本分类 + forbidden 语法检测 + UNC 检测
├── commands/
│   ├── mod.rs           -- 命令配置模块声明 + 通用白名单
│   ├── git.rs           -- Git 命令配置 + Git flag 分组
│   ├── gh.rs            -- GitHub CLI 命令配置
│   ├── docker.rs        -- Docker 命令配置
│   └── tools.rs         -- rg / pyright / 其他工具命令配置
└── tests.rs            -- 整合测试（或按子模块拆分测试）
```

### 各文件详细职责

| 文件 | 职责 | 预估行数 |
|------|------|----------|
| `mod.rs` | `pub mod` 声明、`is_read_only_shell_command` / `is_read_only_bash_command` / `is_read_only_powershell_command` 等公开 API 的 re-export 或委托实现 | ~150 |
| `types.rs` | `FlagArgType`、`ExternalCommandConfig` 及其 impl | ~60 |
| `flag_validation.rs` | `map_from_pairs()`、`validate_flag_arg()`、`merge_maps()`、`is_flag()`、`validate_flags()`、`match_readonly_command()` | ~180 |
| `shell_classifier.rs` | `contains_vulnerable_unc_path()`、`is_read_only_bash_command()`、`is_read_only_powershell_command()`、`classify_shell_command_text()`、`forbidden_shell_syntax()` | ~200 |
| `commands/mod.rs` | `make_external_readonly_commands()` + re-export 各子模块的 `make_*` 函数 | ~120 |
| `commands/git.rs` | 8 个 Git flag 分组函数 + `make_git_read_only_commands()` | ~640 |
| `commands/gh.rs` | `make_gh_read_only_commands()` | ~210 |
| `commands/docker.rs` | `make_docker_read_only_commands()` | ~120 |
| `commands/tools.rs` | `make_rg_read_only_commands()` + `make_pyright_read_only_commands()` | ~170 |
| `tests.rs` | 所有 `#[cfg(test)]` 测试代码 | ~430 |

---

## 三、每个新文件的包含内容（具体到 struct/fn/impl 及行号范围）

### 1. `read_only_shell/mod.rs`

**来源行号**: 1–19 (imports) + 1747–1879 (主入口) + 1569–1580 (公开 API 包装)

包含内容：
- 模块声明 (`mod types; mod flag_validation; mod shell_classifier; mod commands;`)
- `pub use types::*;` 等 re-export
- `is_read_only_bash_command()` — 来自 1569–1571
- `is_read_only_powershell_command()` — 来自 1573–1580
- `is_read_only_shell_command()` — 来自 1747–1879（主路由逻辑）
- `#[cfg(test)] mod tests;` 或内联测试

### 2. `read_only_shell/types.rs`

**来源行号**: 26–72

包含内容：
- `pub enum FlagArgType` (26–33) — 5 个变体
- `pub struct ExternalCommandConfig` (36–46) — 3 个字段
- `impl ExternalCommandConfig` (48–72) — `new()`, `with_check()`, `without_double_dash()`

### 3. `read_only_shell/flag_validation.rs`

**来源行号**: 78–90 + 1351–1469 + 1703–1733 + 1885–1891

包含内容：
- `pub(crate) fn map_from_pairs()` (78–80)
- `pub(crate) fn validate_flag_arg()` (82–90)
- `pub(crate) fn is_flag()` (1351–1353)
- `pub(crate) fn validate_flags()` (1356–1469)
- `pub(crate) fn match_readonly_command()` (1703–1733)
- `pub(crate) fn merge_maps()` (1885–1891)

### 4. `read_only_shell/shell_classifier.rs`

**来源行号**: 1482–1693

包含内容：
- `pub fn contains_vulnerable_unc_path()` (1482–1558)
- `fn classify_shell_command_text()` (1582–1628)
- `fn forbidden_shell_syntax()` (1630–1693)

### 5. `read_only_shell/commands/mod.rs`

**来源行号**: 1245–1343

包含内容：
- `mod git; mod gh; mod docker; mod tools;`
- `pub(crate) use git::make_git_read_only_commands;` 等 re-export
- `pub(crate) fn make_external_readonly_commands()` (1245–1343) — 80+ 个白名单命令

### 6. `read_only_shell/commands/git.rs`

**来源行号**: 96–733

包含内容：
- 8 个 Git flag 分组函数 (96–165):
  - `git_ref_selection_flags()`
  - `git_date_filter_flags()`
  - `git_log_display_flags()`
  - `git_count_flags()`
  - `git_stat_flags()`
  - `git_color_flags()`
  - `git_patch_flags()`
  - `git_author_filter_flags()`
- `pub(crate) fn make_git_read_only_commands()` (171–733) — 16 个 git 子命令配置

### 7. `read_only_shell/commands/gh.rs`

**来源行号**: 739–944

包含内容：
- `pub(crate) fn make_gh_read_only_commands()` (739–944) — 10 个 gh 子命令配置

### 8. `read_only_shell/commands/docker.rs`

**来源行号**: 950–1065

包含内容：
- `pub(crate) fn make_docker_read_only_commands()` (950–1065) — 9 个 docker 子命令配置

### 9. `read_only_shell/commands/tools.rs`

**来源行号**: 1071–1239

包含内容：
- `pub(crate) fn make_rg_read_only_commands()` (1071–1203)
- `pub(crate) fn make_pyright_read_only_commands()` (1209–1239)

### 10. `read_only_shell/tests.rs`

**来源行号**: 1897–2320

包含内容：
- 测试辅助函数: `ro()`, `is_ro()`, `is_not_ro()`, `is_unsupported()` (1903–1918)
- Git 测试: 17 个 test (1922–2090)
- GH 测试: 7 个 test (2094–2133)
- Docker 测试: 4 个 test (2138–2159)
- Ripgrep 测试: 1 个 test (2164–2171)
- Pyright 测试: 1 个 test (2174–2180)
- 外部命令测试: 3 个 test (2185–2206)
- Shell 文本测试: 3 个 test (2211–2248)
- UNC 测试: 3 个 test (2253–2267)
- Plan 模式集成测试: 2 个 test (2271–2319)

---

## 四、模块间依赖关系

```
mod.rs
 ├── types          (无依赖)
 ├── flag_validation
 │    └── types
 ├── shell_classifier
 │    └── cc_shell_command (外部 crate)
 ├── commands/
 │    ├── commands/mod.rs
 │    │    ├── types
 │    │    └── flag_validation (map_from_pairs, merge_maps)
 │    ├── commands/git.rs
 │    │    └── types + flag_validation
 │    ├── commands/gh.rs
 │    │    └── types + flag_validation
 │    ├── commands/docker.rs
 │    │    └── types + flag_validation
 │    └── commands/tools.rs
 │         └── types + flag_validation
 └── tests
      └── 所有上述模块 (通过 super::*)
```

**核心依赖链**:
- `types.rs` → 无内部依赖（最底层）
- `flag_validation.rs` → 仅依赖 `types.rs`
- `commands/*.rs` → 依赖 `types.rs` + `flag_validation.rs`（`map_from_pairs`, `merge_maps`）
- `shell_classifier.rs` → 依赖外部 crate `cc_shell_command`
- `mod.rs` → 依赖上述所有模块

**外部依赖**:
- `std::collections::HashMap`
- `cc_shell_command::fallback` (shell 命令解析器)
- `cc_shell_command::model::{DiagnosticSeverity, ParseMode, ReadOnlyResult, ShellDialect}`

---

## 五、重构步骤（迁移顺序）

按照**自底向上**的顺序，确保每一步编译和测试均通过：

### 步骤 1：创建目录结构 + types.rs

1. 创建 `read_only_shell/` 目录
2. 将 `FlagArgType` + `ExternalCommandConfig` 提取到 `read_only_shell/types.rs`
3. 创建 `read_only_shell/mod.rs`，写入 `mod types; pub use types::*;`
4. 原文件改为 `pub use read_only_shell::*;` 的 facade（或直接删除原文件，改用目录模块）
5. **验证**: `cargo build` + `cargo test`

### 步骤 2：提取 flag_validation.rs

1. 将 `map_from_pairs`、`validate_flag_arg`、`is_flag`、`validate_flags`、`match_readonly_command`、`merge_maps` 移入 `read_only_shell/flag_validation.rs`
2. 更新 `mod.rs` 声明
3. **验证**: `cargo build` + `cargo test`

### 步骤 3：提取命令配置 → commands/ 子模块

**注意**: 这一步行数量大但逻辑最简单——纯粹的数据搬运。

1. 创建 `read_only_shell/commands/` 目录
2. **3a**: 提取 Git flag 分组函数 + `make_git_read_only_commands()` → `commands/git.rs`
3. **3b**: 提取 `make_gh_read_only_commands()` → `commands/gh.rs`
4. **3c**: 提取 `make_docker_read_only_commands()` → `commands/docker.rs`
5. **3d**: 提取 `make_rg_read_only_commands()` + `make_pyright_read_only_commands()` → `commands/tools.rs`
6. **3e**: 提取 `make_external_readonly_commands()` → `commands/mod.rs` + re-export
7. **验证**: `cargo build` + `cargo test`

> **可选**：步骤 3a–3e 可以一次性完成（因为它们互不依赖），也可以逐个工具进行以降低风险。

### 步骤 4：提取 shell_classifier.rs

1. 将 `contains_vulnerable_unc_path`、`classify_shell_command_text`、`forbidden_shell_syntax` 移入 `read_only_shell/shell_classifier.rs`
2. 更新 `mod.rs` 中的调用关系
3. **验证**: `cargo build` + `cargo test`

### 步骤 5：整理 mod.rs 公开 API

1. 将 `is_read_only_bash_command`、`is_read_only_powershell_command`、`is_read_only_shell_command` 写入 `mod.rs`
2. 使用 `pub use` 或直接委托函数暴露公开 API
3. 确保原 `read_only_shell` 模块的所有 `pub` item 在新结构中均有对应导出
4. **验证**: `cargo build` + `cargo test`

### 步骤 6：迁移测试

1. 将原测试代码移入 `read_only_shell/tests.rs`
2. 在 `mod.rs` 中声明 `#[cfg(test)] mod tests;`
3. 调整 `use super::*` 为 `use crate::read_only_shell::*`（或按需）
4. **验证**: `cargo test`

### 步骤 7：清理 + 最终检查

1. 删除旧的 `read_only_shell.rs` 文件（如果使用目录模块替代）
2. 检查 `crate` 中所有 `use cc_permissions::read_only_shell::*` 引用是否正常
3. 运行 `cargo clippy` 确保无新 warning
4. **验证**: `cargo build --workspace --release` + `cargo test --workspace`

---

## 六、注意事项

### 1. 公开 API 兼容性

当前 `read_only_shell` 模块导出以下 `pub` 符号：
- `enum FlagArgType`
- `struct ExternalCommandConfig`
- `fn contains_vulnerable_unc_path()`
- `fn is_read_only_bash_command()`
- `fn is_read_only_powershell_command()`
- `fn is_read_only_shell_command()`

**拆分后必须保持这些符号的路径可访问性不变**（或在 crate 根做 re-export）。

### 2. 可见性策略

- `FlagArgType` 和 `ExternalCommandConfig` 需要 `pub`（外部使用者可能需要自定义配置）
- 命令配置函数（`make_*`）建议用 `pub(crate)`
- 内部辅助函数（`map_from_pairs`、`merge_maps`、`is_flag` 等）用 `pub(crate)` 即可
- `match_readonly_command`、`validate_flags` 在 `mod.rs` 主逻辑中使用，用 `pub(super)` 或 `pub(crate)`

### 3. 性能考量

当前每次调用 `is_read_only_shell_command()` 都会重新创建 `HashMap`（通过 `make_*` 函数）。拆分后：
- **短期**: 保持现状，函数内局部创建 HashMap
- **中期**: 可考虑用 `lazy_static!` / `std::sync::LazyLock` 缓存这些配置表，避免每次调用的重复分配
- 这不在本次拆分范围内，但文件分离后更容易独立优化

### 4. `commands/git.rs` 特别说明

该文件预计 ~640 行，是拆分后最大的文件。进一步拆分的可选方案：
- 按 git 子命令类别分组（读写混合类如 branch/config/tag 单独一个文件）
- 但考虑到这些函数都是纯数据声明、模式统一，保持单一文件更利于维护

### 5. 命名一致性

当前 `make_*` 函数使用 `fn` 构建。文件拆分后建议统一：
- 函数命名: `make_<tool>_readonly_commands()`
- 返回类型: `HashMap<String, ExternalCommandConfig>`
- 在 `commands/mod.rs` 中统一 re-export

### 6. 测试覆盖

拆分过程中**不修改任何测试逻辑**，仅迁移位置。拆分完成后再考虑：
- 为 `flag_validation.rs` 添加独立单元测试
- 为 `shell_classifier.rs` 添加 `forbidden_shell_syntax` 的细分测试
- 为 `commands/` 各子模块添加配置完整性测试（确保所有预期子命令都被注册）
