## 执行计划（全部补齐生产链路）

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `vim.rs::VimMode::short_indicator()` | ✅ **解禁** — 只读状态访问器 | 移除 `#[cfg(test)]` |
| 2 | `vim.rs::EditorModeSetting::as_str()` | ✅ **解禁** — 字符串序列化 | 移除 `#[cfg(test)]` |
| 3 | `vim.rs::VimState::editor_mode_setting()` | ✅ **解禁** — getter | 移除 `#[cfg(test)]` |
| 4 | `slack_channel_completion.rs::MAX_CACHED_QUERIES` 常量 | ✅ **解禁** — 配置化常量，可暴露给用户配置 | 移除 `#[cfg(test)]` |
| 5 | `slack_channel_completion.rs::set_known_channels()` 等4个方法 | ❌ **保留** — 测试专用状态注入，生产中数据来自 engine | 保持 `#[cfg(test)]` |
| 6 | `form_navigation.rs::TabbedFormState::selected_option()` | ✅ **解禁** — 只读状态查询 | 移除 `#[cfg(test)]` |
| 7 | `path_completion.rs::set_include_hidden()` 等3个方法 | ❌ **保留** — 测试配置注入 | 保持 `#[cfg(test)]` |
| 8 | `shell_history_completion.rs::with_paths()` / `notify_command_executed()` | ❌ **保留** — 测试构造/模拟 | 保持 `#[cfg(test)]` |
| 9 | `completions.rs::is_empty()` / `provider_count()` | ✅ **解禁** — 标准查询方法 | 移除 `#[cfg(test)]` |
| 10 | `completions.rs::find_command_token_range()` | ✅ **解禁** — 辅助函数 | 移除 `#[cfg(test)]` |
| 11 | `clipboard_paste.rs::normalize_pasted_path()` | ✅ **接入** — 路径标准化在生产剪贴板处理中有用 | 移除 `#[cfg(test)]`；在 `app/input.rs` 粘贴处理中调用 |
| 12 | `clipboard_paste.rs::normalize_windows_path()` | ✅ **接入** — 同上 | 移除 `#[cfg(test)]` |
| 13 | `clipboard_paste.rs::pasted_image_format()` | ✅ **接入** — 剪贴板图片格式检测 | 移除 `#[cfg(test)]` |

### 需要修改的生产文件
- `input/vim.rs`: 移除 L38, L62, L159 的 `#[cfg(test)]`
- `input/slack_channel_completion.rs`: 移除 L14 的 `#[cfg(test)]`
- `input/form_navigation.rs`: 移除 L84 的 `#[cfg(test)]`
- `input/completions.rs`: 移除 L403, L408, L434 的 `#[cfg(test)]`
- `input/clipboard_paste.rs`: 移除 L287, L357, L387 的 `#[cfg(test)]`
- `app/input.rs`: 在粘贴事件处理中调用 `normalize_pasted_path()` / `pasted_image_format()`

### 测试/构建验证
```bash
cargo test -p allthecodes ui::input
cargo build --workspace --release
```
