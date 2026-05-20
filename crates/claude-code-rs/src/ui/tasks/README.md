## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs` 13个 `pub mod` 声明 | ✅ **接入** — `CommandSurface::Tasks` 存在但只有列表；各子模块是任务详情/进度渲染器 | 移除 `#[cfg(test)]`；在 `TasksSurface` 状态机中添加列表→详情导航，根据 `TaskKind` 分发到各详情渲染器；从 `BackendMessage` 事件中构建 `TaskStatus` 列表 |
| 2 | `mod.rs::TaskStatus::new()` | ✅ **接入** — 构造器可用于从后端数据构建 TaskStatus | 移除 `#[cfg(test)]` |
| 3 | `task_status_utils.rs` 4个辅助函数 + 3个use导入 | ✅ **接入** — 进度条/标题渲染是任务 UI 的基础 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `tasks/mod.rs`: 移除 L3-28, L63-75 的 `#[cfg(test)]`
- `tasks/task_status_utils.rs`: 移除 L3-99 的 `#[cfg(test)]`
- `command_surface/surfaces/tasks.rs`: 在 `handle_key()` / `render()` 中实现任务列表→详情状态机；在 `handle_event()` 中处理 `BackendMessage` 事件更新任务状态

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::tasks
cargo build --workspace --release
```
