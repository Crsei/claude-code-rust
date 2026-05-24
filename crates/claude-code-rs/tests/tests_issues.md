# PTY TUI E2E 测试问题记录

> 更新日期: 2026-05-24

本文件只保留当前仍需跟踪的问题。已经被后续实现覆盖的旧批次结果已删除，包括：

- `/login claude-code` / `SetPermission("full access")` 已修复记录
- `OPENAI_CODEX_AUTH_TOKEN` provider 误路由已修复记录
- `TestStep::Command` 快照文件名过长已修复记录
- `/compact` 精确命令被补全面板拦截已修复记录
- 在线对话测试依赖 `"Claude:"` 前缀、旧状态栏 `ready` + `N msgs` 格式的落后记录
- `/compact` 在模型 BUSY 状态下发送导致的旧测试时序记录

## 当前已知问题

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| TP-003 | 中 | Open | API 超时 | 使用 `codex` profile（openai-codex provider，`https://chatgpt.com/backend-api/codex/responses`）时，API 请求仍可能频繁 `TimedOut`，TUI 表现为停在 `Thinking...`。 | 在线 E2E 默认优先使用 `claude_code` / anthropic profile；若必须覆盖 `codex` profile，需要单独放宽超时并按网络/API 可用性判断结果。 |
| TF-001 | 低 | Open | 非 Ignored E2E | 部分命令类脚本仍存在屏幕关键词断言不匹配。 | 最新保留的失败集合：`config_alias_settings`、`config_show_displays_settings`、`session_command_shows_info`、`permissions_mode_auto`、`mcp_help_no_args`、`agents_list`、`effort_set_high`、`fast_toggle`。需要基于实际 HTML 快照调整关键词或等待方式。 |

## 当前建议

1. 非 Ignored 屏幕断言问题优先检查 `logs/pty_tui_e2e_scripts/{test_name}/` 下的 HTML 快照，确认实际命令输出文本。
2. 若输出正确但不在当前屏幕可见区域，优先改用 `WaitForText` / `AssertTextContains`，不要继续扩大固定 `Wait`。
3. `codex` profile 超时按网络/API 可用性单独跟踪，不与 anthropic/deepseek profile 的在线 E2E 混为同一类问题。
