## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs::pub mod overlay_stack` | ❌ **保留** — 当前 App 使用直接 Option 字段管理 dialog/surface，无需 z-index 栈。若未来多个叠加 overlay 再接入。 | 保持 `#[cfg(test)]` |
| 2 | `dialog.rs::ExitGuard::{arm, disarm, confirm}` | ✅ **接入** — Ctrl+C 按两次退出的状态机工作在生产中需要这些方法 | 移除 `#[cfg(test)]`；在 `tui.rs` 的 Ctrl+C 处理中集成 `ExitGuard::arm()`/`confirm()` |
| 3 | `dialog.rs::DialogEvent` 枚举 | ✅ **接入** — `Dialog::handle_key()` 返回的事件类型，生产中需处理 | 移除 `#[cfg(test)]` |
| 4 | `dialog.rs::Dialog::handle_key()` | ✅ **接入** — Modal 对话框应能在生产 TUI 中接收键盘输入 | 移除 `#[cfg(test)]`；在 `app/input.rs` 中，当 `dialog` 处于活跃状态时分发到 `handle_key()` |
| 5 | `dialog.rs::Dialog::subtitle()` / `hide_border()` / `cancel_active()` / `input_guide()` | ✅ **解禁** — builder 方法，应可自由使用 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `overlays/dialog.rs`: 移除相关所有 `#[cfg(test)]`
- `overlays/mod.rs`: 保留 `overlay_stack` 的 `#[cfg(test)]`
- `app/input.rs`: 当 `app.active_dialog()` 存在时，调用 `Dialog::handle_key()`
- `tui.rs`: 在 Ctrl+C 事件处理中使用 `ExitGuard::{arm,confirm}`

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::overlays
cargo build --workspace --release
```
