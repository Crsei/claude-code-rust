## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs::pub mod memory_update_notification` | ✅ **接入** — 内存更新通知是生产功能，`CommandSurface::Memory` 存在  | 移除 `#[cfg(test)]`；在 `MemorySurface::render()` 中检测到内存更新时调用 `render_memory_update_notification()` |
| 2 | `memory_file_selector.rs::MemoryFileOption::with_parent()` | ✅ **解禁** — builder 方法 | 移除 `#[cfg(test)]` |
| 3 | `memory_file_selector.rs::MemoryFileSelectorState::selected_path()` | ✅ **解禁** — 只读 getter | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `memory/mod.rs`: 移除 L3-4 的 `#[cfg(test)]`
- `memory/memory_file_selector.rs`: 移除 L38, L63 的 `#[cfg(test)]`
- `command_surface/surfaces/memory.rs`: 在 `render()` 中集成 `memory_update_notification`

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::memory
cargo build --workspace --release
```
