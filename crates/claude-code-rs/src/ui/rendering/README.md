## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `theme.rs::ThemeKind` 枚举 + impl | ❌ **保留** — 测试专用主题类型；生产通过 `ThemeName` + `ThemeProvider` 路径 | 保持 `#[cfg(test)]` |
| 2 | `theme.rs::progress_fill` / `progress_empty` 字段 | ✅ **解禁** — `Theme` 是生产结构体；`progress_bar.rs::render_styled_progress_bar()` 无条件引用这些字段，非测试构建会编译失败 | 移除字段和 `Default` impl 中的 `#[cfg(test)]`；在 `theme/mod.rs` 的 `from_design_colors()` 非测试分支中初始化为合理默认值 |
| 3 | `theme.rs::Theme::new()` / `Theme::named()` | ❌ **保留** — 测试构造器；生产通过 `Theme::default()` / `from_design_colors()` | 保持 `#[cfg(test)]` |
| 4 | `theme.rs::light_theme()` / `tron_theme()` | ❌ **保留** — 仅被 `ThemeKind::build()` 消费 | 保持 `#[cfg(test)]` |
| 5 | `syntax_highlight.rs::supports_language()` 桩 | ❌ **保留** — 无真实语法高亮引擎接入 | 保持 `#[cfg(test)]` |
| 6 | `tool_activity.rs::ToolState` 4个变体 | ✅ **解禁** — `tasks/render_tool_activity.rs`（非测试模块）已通过 `task_state_to_tool_state()` 映射到这些变体，缺失会导致编译失败 | 移除 L13, L16, L18, L20 的 `#[cfg(test)]` |
| 7 | `tool_activity.rs::ToolState::label()` | ✅ **解禁** — 随变体同步解禁 | 移除 L25 及内部 match 分支的 `#[cfg(test)]` |
| 8 | `tool_activity.rs::ToolActivity::new()` | ✅ **解禁** — `render_tool_activity.rs` L17 已调用 `ToolActivity::new()` | 移除 L57 的 `#[cfg(test)]` |
| 9 | `tool_activity.rs::render_grouped_activity()` | ✅ **解禁** — `render_tool_activity.rs` L13 已调用 | 移除 L261 的 `#[cfg(test)]` |
| 10 | `tool_activity.rs::format_elapsed()` | ✅ **解禁** — 被 `compact_line()` 消费，`compact_line()` 被 `render_grouped_activity()` 调用 | 移除 L362 的 `#[cfg(test)]` |
| 11 | `tool_activity.rs` 其余方法（`compact_line`, `compact_styled_line`, `transcript_block`, `progress_text`, `line_to_plain`）| ✅ **部分解禁** — `compact_line()` 和 `line_to_plain()` 被 `render_grouped_activity()` 调用，需同步解禁；其余保留 | `compact_line()`(L88), `line_to_plain()`(L286): 解禁；`compact_styled_line()`, `transcript_block()`, `progress_text()`: 保留 |
| 12 | `virtual_scroll.rs` 5个 getter 方法 | ✅ **解禁** — 基础查询 API，`visual_offset_of()` 等同级方法已解禁 | 移除 L161, L175, L201, L208, L253 的 `#[cfg(test)]` |

### 需要修改的生产文件
- `rendering/theme.rs`: 移除 `progress_fill`/`progress_empty` 字段及其 `Default` 初始化的 `#[cfg(test)]`
- `theme/mod.rs`: 在 `from_design_colors()` 的 `#[cfg(not(test))]` 分支添加 `progress_fill`/`progress_empty` 初始化
- `rendering/tool_activity.rs`: 移除 `ToolState` 变体(L13-20)、`label()`(L25)、`new()`(L57)、`compact_line()`(L88)、`render_grouped_activity()`(L261)、`line_to_plain()`(L286)、`format_elapsed()`(L362) 的 `#[cfg(test)]`
- `rendering/virtual_scroll.rs`: 移除 5 个 getter 方法的 `#[cfg(test)]`

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::rendering
cargo build --workspace --release
```
