## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs::pub mod hooks_config_menu` | ✅ **接入** — `CommandSurface::Hooks` 存在，但缺少概览页面；`hooks_config_menu` 就是顶层的 hooks 配置摘要页面 | 移除 `#[cfg(test)]`；在 `HooksSurface::render()` 中当无选中事件时渲染 `render_hooks_config_menu()` |
| 2 | `select_hook_mode.rs::HookListItem::new()` | ✅ **解禁** — Hook 列表选项构造器 | 移除 `#[cfg(test)]` |
| 3 | `select_matcher_mode.rs::HookMatcher::all_tools()` / `for_tool()` | ✅ **解禁** — Matcher 构造器 | 移除 `#[cfg(test)]` |
| 4 | `select_event_mode.rs::HOOK_EVENTS` re-export | ✅ **解禁** — Hooks 事件常量 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `hooks/mod.rs`: 移除 L2 的 `#[cfg(test)]`
- `hooks/select_hook_mode.rs`: 移除 L11 的 `#[cfg(test)]`
- `hooks/select_matcher_mode.rs`: 移除 L11, L20 的 `#[cfg(test)]`
- `hooks/select_event_mode.rs`: 移除 L4-5 的 `#[cfg(test)]`
- `command_surface/surfaces/hooks.rs`: 在 `HooksSurface::render()` 中添加 hooks_config_menu 的渲染

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::hooks
cargo build --workspace --release
```
