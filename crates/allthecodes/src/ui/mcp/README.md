## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `mod.rs` 17个 `pub mod` 声明 | ✅ **接入** — `CommandSurface::Mcp` 存在但只有列表面板；各子模块是 MCP 服务器详情/菜单/对话框视图 | 移除 `#[cfg(test)]`；在 `McpSurface` 状态机中添加 列表→服务器类型菜单→工具列表→工具详情→设置 的导航路径，分发到各子模块渲染 |
| 2 | `index.rs::McpServerKind::Agent` 变体+分支 | ✅ **接入** — MCP Agent 服务器是生产概念 | 移除 `#[cfg(test)]` |
| 3 | `utils/mod.rs::reconnect_helpers` | ✅ **接入** — 重连逻辑是生产 MCP 功能 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `mcp/mod.rs`: 移除 L2-35 的所有 `#[cfg(test)]`
- `mcp/index.rs`: 移除 L7-8, L16-17 的 `#[cfg(test)]`
- `mcp/utils/mod.rs`: 移除 L2-3 的 `#[cfg(test)]`
- `command_surface/surfaces/mcp.rs`: 在 `handle_key()` / `render()` 中实现完整的 MCP 子页面状态机

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::mcp
cargo build --workspace --release
```
