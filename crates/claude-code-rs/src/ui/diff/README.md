## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `diff_dialog.rs` 顶层3个 `use` 导入 | ✅ **接入** — `CommandSurface::Diff` 存在，`DiffDialogMode` / `DiffSource` 是 diff surface 的核心状态 | 移除 `#[cfg(test)]` |
| 2 | `diff_dialog.rs` 8个辅助函数 + `render_diff_dialog_lines()` | ✅ **接入** — `pluralize()`, `stats_line()`, `source_selector()`, `empty_message()` 是 diff 对话框的核心渲染函数 | 移除 `#[cfg(test)]`；在 `DiffSurface::render()` 中集成 |
| 3 | `diff_dialog.rs` 条件 use 导入 | ✅ **接入** — `DiffFile`, `DiffStats` 已用于 diff 状态 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `diff/diff_dialog.rs`: 移除所有 `#[cfg(test)]`
- `command_surface/surfaces/diff.rs`: 在 `render()` 中使用 `DiffDialogMode` / `DiffSource` 状态机分发到各渲染函数

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::diff
cargo build --workspace --release
```
