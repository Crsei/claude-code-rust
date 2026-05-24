## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `PermissionDecision::Escalate` 枚举变体 | ✅ **接入** — 在权限选项中添加"升级"选项 | 移除 `#[cfg(test)]`；在 `default_permission_options()` 或特定工具渲染器中添加 `Escalate` 选项；在 `App` 的权限决策处理中添加分支 |
| 2 | `PermissionRequestView::for_worker()` | ✅ **接入** — 检测 worker 权限请求并显示上下文 | 移除 `#[cfg(test)]`；在 `EngineEvent::PermissionRequest` 处理中检测 worker 并调用 `.for_worker()` |
| 3 | `PermissionDialog::new()` (dialog_overlay.rs) | ✅ **解禁** — `PermissionDialog` 已在 App 字段中，`new()` 是标准构造器 | 移除 `#[cfg(test)]`；同时移除 `PermissionDialogRequest::legacy()` 的 `#[cfg(test)]`（`new()` 依赖它） |
| 4 | `ask_user_question_permission_request/mod.rs` 4个模块 | ✅ **接入** — 接入 `QuestionDialog` 渲染路径 | 移除 `#[cfg(test)]`；将 `render_ask_user_question_permission_request()` / `MultipleChoiceState` / `question_view` / `question_navigation_bar` 集成到 `QuestionDialog` 的 `render()` 和 `handle_key()` 中，替换当前内联状态 |
| 5 | `bash_permission_request/mod.rs::bash_tool_use_options` | ✅ **接入** — 为 bash 权限请求添加选项渲染 | 移除 `#[cfg(test)]`；在 `PermissionRequestRouter::route_bash()` 中调用 `render_bash_tool_use_options()` |
| 6 | `power_shell_permission_request/mod.rs::powershell_tool_use_options` | ✅ **接入** — 为 PowerShell 权限请求添加选项渲染 | 移除 `#[cfg(test)]`；在 `PermissionRequestRouter::route_powershell()` 中调用 |
| 7 | `file_permission_dialog/mod.rs` 4个模块 | ✅ **接入** — 接入文件权限 dialog 生产链路 | 移除 `#[cfg(test)]`；在 `PermissionRequestRouter` 中新增 `FileDialog` 路由分支，使用 `render_file_permission_dialog()`；将 `FilePermissionDialogState` 接入 `PermissionDialog` 的文件审批路径 |
| 8 | `rules/mod.rs` 4个模块 | ✅ **接入** — 接入 `/permissions` CommandSurface | 移除 `#[cfg(test)]`；在 `PermissionsSurface` 中新增规则编辑子页面，路由到 `render_add_permission_rules()` / `add_workspace_directory()` 等 |

### 需要修改的生产文件
- `utils.rs`: 移除 L11, L22, L139 的 `#[cfg(test)]`
- `dialog_overlay.rs`: 移除 L73 的 `#[cfg(test)]`
- `permission_request_router.rs`: 移除 L52 的 `#[cfg(test)]`；在 `route()` 函数中新增 `bash_tool_use_options` / `powershell_tool_use_options` / `file_permission_dialog` 路由
- `ask_user_question_permission_request/mod.rs`: 移除 L4-14 的 `#[cfg(test)]`
- `bash_permission_request/mod.rs`: 移除 L6 的 `#[cfg(test)]`
- `power_shell_permission_request/mod.rs`: 移除 L6 的 `#[cfg(test)]`
- `file_permission_dialog/mod.rs`: 移除 L4-12 的 `#[cfg(test)]`
- `rules/mod.rs`: 移除 L2-12 的 `#[cfg(test)]`
- `permissions/surfaces` (PermissionsSurface): 新增规则编辑子页面路由

### 测试/构建验证
```bash
cargo test -p allthecodes ui::permissions
cargo build --workspace --release
```
