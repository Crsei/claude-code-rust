## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs` 8个 `pub mod` 声明（agent_editor, color_picker, generate_agent, model_selector, new_agent_creation, tool_selector, validate_agent, agent_navigation_footer） | ✅ **接入** — 这些是 Agents 创建/编辑向导的内部页面，`CommandSurface::Agents` 存在但向导流程未接入 | 移除 `#[cfg(test)]`；在 `AgentsSurface::handle_key()` 中添加状态机转换（如 `EditAgent` → `agent_editor`, `CreateAgent` → `new_agent_creation` 向导步骤），在 `AgentsSurface::render()` 中分发到各子模块渲染 |
| 2 | `types.rs` 测试枚举变体/方法 | ✅ **部分接入** — `AgentMemoryScope::None` + `AgentModeState` 等应进入生产 | 移除相关 `#[cfg(test)]`；`AgentSource::is_editable()`, builder 方法等接入 `AgentDefinition` 的生产 API |
| 3 | `utils.rs` 3个辅助函数 | ✅ **解禁** — `group_agents_by_source()` 等是独立纯函数 | 移除 `#[cfg(test)]` |
| 4 | `agent_file_utils.rs` 6个辅助函数+use导入 | ✅ **部分接入** — `format_agent_as_markdown()`, `sanitize_agent_filename()` 是生产有用函数 | 移除对应 `#[cfg(test)]`；确保 `AgentsSurface` 中使用这些函数 |
| 5 | `agents_list.rs::render()` + use导入 | ✅ **接入** — `AgentsListState` 已在 `AgentsSurface` 中持有，`render()` 应是生产渲染路径 | 移除 `#[cfg(test)]` |
| 6 | `agents_menu.rs` 3个方法 | ✅ **接入** — 导航方法应由键盘事件直接调用 | 移除 `#[cfg(test)]`；在 `AgentsSurface::handle_key()` 中确保上下键调用 `move_next/prev` |

### 需要修改的生产文件
- `agents/mod.rs`: 移除 L3-23 的所有 `#[cfg(test)]`
- `agents/types.rs`: 移除 L32-217 的 `#[cfg(test)]`
- `agents/utils.rs`: 移除 L55-172 的 `#[cfg(test)]`
- `agents/agent_file_utils.rs`: 移除 L3-123 的 `#[cfg(test)]`
- `agents/agents_list.rs`: 移除 L5-72 的 `#[cfg(test)]`
- `agents/agents_menu.rs`: 移除 L43-64 的 `#[cfg(test)]`
- `command_surface/surfaces/agents.rs`: 在 `handle_key()` / `render()` 中接入各子模块的状态机

### 测试/构建验证
```bash
cargo test -p allthecodes ui::agents
cargo build --workspace --release
```
