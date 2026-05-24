## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs` 的 `NotificationMethod` + `DesktopNotificationBackend` + `bel`/`osc9` 模块 + `detect_backend` + `supports_osc9` | ✅ **接入** — 终端原生桌面通知（OSC9/BEL）是生产功能 | 移除 `#[cfg(test)]`；在 `run_tui` 初始化时调用 `detect_backend()` 检测终端能力；在 `InAppNotification::show()` 路径中集成 `DesktopNotificationBackend::notify()` |
| 2 | `in_app.rs::with_rendered()` | ✅ **解禁** — builder 方法应可自由使用 | 移除 `#[cfg(test)]` |
| 3 | `in_app.rs::with_invalidates()` | ✅ **解禁** — builder 方法应可自由使用 | 移除 `#[cfg(test)]` |
| 4 | `in_app.rs::queued_len()` | ✅ **解禁** — 只读访问器 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `notifications/mod.rs`: 移除 L1-88 的 `#[cfg(test)]`
- `notifications/in_app.rs`: 移除 L71, L82, L119 的 `#[cfg(test)]`
- `tui.rs`: 在 `run_tui` 初始化中添加 `detect_backend()` 调用

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::notifications
cargo build --workspace --release
```
