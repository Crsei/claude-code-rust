## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `approval_overlay.rs::ApprovalKind::subject()` | ✅ **解禁** — `ApprovalKind` 已在 `PermissionDialog` 生产链路中使用 | 移除 `#[cfg(test)]` |
| 2 | `approval_overlay.rs::ApprovalChoice` / `ApprovalOverlay` | ❌ **保留** — 与生产 `PermissionChoice` / `PermissionDialog` 功能重复，无需重复接入 | 保持 `#[cfg(test)]` |
| 3 | `bottom_pane.rs::BottomPaneView` / `BottomPane` | ❌ **保留** — 仅 `visual_regression.rs` 使用；生产通过 `BottomPaneHeights` + `app/render.rs` 直接布局 | 保持 `#[cfg(test)]` |
| 4 | `divider.rs::char()` / `padding()` / `title()` | ✅ **解禁** — `Divider` 已是生产组件，builder 方法应可自由使用 | 移除 `#[cfg(test)]` |
| 5 | `tabs.rs::Tab.id` 字段 | ✅ **解禁** — 存储 tab 标识用于断言和调试 | 移除 `#[cfg(test)]`，将构造参数 `_id` 改为 `id` 并赋值 |
| 6 | `tabs.rs` 字段 + 方法（`select_next/prev`, `selected/selected_tab`, builder, `render_header`） | ✅ **解禁** — `Tabs` 是通用组件，所有公开 API 应在生产中可用 | 移除所有相关 `#[cfg(test)]` |
| 7 | `fuzzy_match.rs::weighted_fuzzy_match()` | ✅ **解禁** — 独立函数，可用于增强模糊搜索 | 移除 `#[cfg(test)]` |
| 8 | `search_box.rs` 5个 builder 方法 | ✅ **解禁** — `fuzzy_picker.rs`（生产 `pub mod`）已在调用 `focused()`/`terminal_focused()`/`cursor_offset()`，当前 `#[cfg(test)]` 是 bug | 移除全部 5 个 `#[cfg(test)]` |
| 9 | `keyboard_shortcut.rs::with_bold_key()` / `with_color()` | ✅ **解禁** — builder 方法应可自由使用 | 移除 `#[cfg(test)]` |
| 10 | `keyboard_shortcut.rs::render_byline()` / `Byline` | ✅ **接入** — `fuzzy_picker.rs`（生产 `pub mod`）已在调用 `render_byline()`，当前 `#[cfg(test)]` 是 bug | 移除 `#[cfg(test)]`；`Byline` 结构体及 impl 同步解禁 |
| 11 | `history_search_dialog.rs::loading()` / `query()` | ✅ **解禁** — `HistorySearchDialog` 已在 `App` 字段中，构造/查询应可用 | 移除 `#[cfg(test)]` |
| 12 | `better_view_panel.rs::key_value_row()` | ✅ **解禁** — 工具函数，`BetterViewPanel` 已在生产中使用 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `search_box.rs`: 移除 L35, L41, L47, L53, L64 的 `#[cfg(test)]`
- `keyboard_shortcut.rs`: 移除 L60, L65, L141, L152-196 的 `#[cfg(test)]`
- `tabs.rs`: 移除 L17, L24, L41-51, L59-66, L79-105, L109-177 的 `#[cfg(test)]`
- `divider.rs`: 移除 L63-81 的 `#[cfg(test)]`
- `fuzzy_match.rs`: 移除 L72-137 的 `#[cfg(test)]`
- `history_search_dialog.rs`: 移除 L68-73, L96 的 `#[cfg(test)]`
- `better_view_panel.rs`: 移除 L122 的 `#[cfg(test)]`
- `approval_overlay.rs`: 仅移除 L29（`subject()`）的 `#[cfg(test)]`

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::components
cargo build --workspace --release
```
