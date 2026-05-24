## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `app_event.rs::AppEvent::Tick` | ✅ **解禁** — `AppEvent` 是生产事件枚举，可用于通知队列处理/轮询 | 移除 `#[cfg(test)]`，确保 `tui.rs` 主循环的 `app.tick()` 路径与 `AppEvent::Tick` 处理一致 |
| 2 | `app_event.rs::AppEvent::Shutdown` | ✅ **解禁** — IPC断开等致命错误可通过事件通道触发优雅关闭 | 移除 `#[cfg(test)]`，在 `tui.rs` 后端断开时发送 `AppEvent::Shutdown` |
| 3 | `input.rs::CompletionState::select_next()` | ✅ **接入** — 用户按 Tab 应循环前进（Shift+Tab 已有 `select_prev()`）；当前 Tab 总是接受而非循环，这是缺失功能 | 移除 `#[cfg(test)]`；在 `input.rs` Tab 处理分支（~L327-369）中，当 `completion_state.active` 为 true 时调用 `select_next()` 并更新 ghost suffix |
| 4 | `status.rs::StatusState::status_line_runner()` | ✅ **解禁** — 只读访问器，移除守卫即可 | 移除 `#[cfg(test)]` |
| 5 | `agent_navigation.rs::short_thread_id()` | ✅ **接入** — 与 `agent_tree_dialog.rs` 的重复实现合并 | 移除 `#[cfg(test)]`，设为 `pub(super)`；删除 `agent_tree_dialog.rs` 中的重复 |
| 6 | `agent_navigation.rs::render_agent_tree()` | ❌ **保留** — 纯文本调试输出，生产使用 ratatui 版 `AgentTreeDialog::render_lines()` | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
- `app_event.rs`: 移除 L22、L24 的 `#[cfg(test)]`
- `app.rs`: 移除 `handle_app_event` 中 `AppEvent::Tick` / `AppEvent::Shutdown` 匹配分支的 `#[cfg(test)]`
- `input.rs`: 移除 `select_next()` 的 `#[cfg(test)]`；在 Tab 处理分支新增 completion-active 时的 `select_next()` 逻辑
- `status.rs`: 移除 `status_line_runner()` 的 `#[cfg(test)]`
- `agent_navigation.rs`: 移除 `short_thread_id()` 的 `#[cfg(test)]`，设为 `pub(super)`
- `agent_tree_dialog.rs`: 删除本地 `short_thread_id()`，改为从 `agent_navigation` 导入

### 测试/构建验证
```bash
cargo test -p allthecodes ui::app
cargo build --workspace --release
```
