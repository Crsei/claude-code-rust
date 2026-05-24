## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `command_palette.rs::apply_command_suggestion()` | ✅ **接入** — 命令调色板的建议应用逻辑，可通过 `selected_command_input()` 或 `CommandAction::Execute` 路径使用 | 移除 `#[cfg(test)]` |
| 2 | `command_palette.rs::CommandAction` 枚举 | ✅ **接入** — 区分 Insert(填入输入) 和 Execute(直接执行) 两种 Palette action | 移除 `#[cfg(test)]`；在 `CommandPalette.handle_key()` 中返回 `CommandAction` 供 `App` 处理 |
| 3 | `filter.rs::find_mid_input_slash_command()` | ✅ **解禁** — 辅助函数 | 移除 `#[cfg(test)]` |
| 4 | `tests.rs` 所有辅助函数 | ❌ **保留** — 测试模块 | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
- `command_palette/mod.rs`: 移除 L131, L281 的 `#[cfg(test)]`
- `command_palette/filter.rs`: 移除 L283 的 `#[cfg(test)]`
- `app.rs` 或 `tui.rs`: 在 `CommandPalette.handle_key()` 返回 `CommandAction::Execute` 时直接提交 prompt

### 测试/构建验证
```bash
cargo test -p allthecodes ui::command_palette
cargo build --workspace --release
```
