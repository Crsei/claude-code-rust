## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `render.rs::build_message_render_context()` | ❌ **保留** — 测试专用上下文构造；生产 `render_messages()` 直接使用 Theme | 保持 `#[cfg(test)]` |
| 2 | `render.rs::render_single_message()` | ❌ **保留** — 测试入口 | 保持 `#[cfg(test)]` |
| 3 | `render.rs::abbreviate_json()` | ✅ **接入** — 消息渲染中缩写长 JSON 在生产场景也有用（如展示 ToolUse 截断内容） | 移除 `#[cfg(test)]` |
| 4 | `render.rs::tool_input_summary()` | ✅ **接入** — 同上，在工具输入渲染中缩短摘要 | 移除 `#[cfg(test)]` |
| 5 | `assistant_tool_use_message.rs::ToolUseState` 枚举变体 `Queued/WaitingForPermission/ClassifierChecking` | ✅ **接入** — 如果引擎发送这些状态，生产 `ToolUseState` 应包含所有可能变体 | 移除 `#[cfg(test)]`；确保 `from_tool_use()` 处理所有变体 |
| 6 | `user_tool_result_message/utils.rs::with_tool_use_result()` | ❌ **保留** — 测试构造辅助 | 保持 `#[cfg(test)]` |
| 7 | `user_tool_result_message/utils.rs::line_to_text()` | ✅ **解禁** — 纯函数，可用于生产 | 移除 `#[cfg(test)]` |
| 8 | 所有 `render_*()` 辅助函数 | ❌ **保留** — 这些是消息类型级 snapshot 测试的基础设施，生产消息渲染在 `render_messages()` 路径中已有更完整的实现 | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
- `messages/render.rs`: 移除 L2061, L2075 的 `#[cfg(test)]`
- `messages/assistant_tool_use_message.rs`: 移除 L16, L25, L28, L54, L92, L100 的 `#[cfg(test)]`
- `messages/user_tool_result_message/utils.rs`: 移除 L110 的 `#[cfg(test)]`

### 测试/构建验证
```bash
cargo test -p claude-code-rs ui::messages
cargo build --workspace --release
```
