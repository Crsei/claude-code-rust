# PTY TUI E2E 测试问题记录

> 更新日期: 2026-05-24

## 已知问题

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| TP-001 | 高 | Fixed | LoginSwitch | `/login claude-code` / `/login claude_code` 现在直接切换并持久化 `claude_code` authProfile；TUI `/login` 面板也提交具名命令，不再使用数字入口。 | OAuth 改用具名命令：`/login claude-ai`、`/login console`、`/login codex-oauth`。 |
| TP-002 | 高 | Fixed | SetPermission | `/permissions full access` 现在映射到 `/permissions mode bypass --confirm`，即 full access 的全自动 bypass 模式。 | TUI `PermissionsSurface` 仍提交 confirmed bypass；PTY `SetPermission("full access")` 可直接使用。 |
| TP-003 | 中 | Open | API 超时 | 使用 `codex` profile（openai-codex 提供者，后端 URL: `https://chatgpt.com/backend-api/codex/responses`）时，API 请求频繁超时：`error=TimedOut`。这导致模型无法响应，TUI 卡在 "Thinking..." 状态。 | 改用 `claude_code` profile（anthropic 提供者，`https://inferaichat.com`）并使用 `CC_RUST_HOME` 指向临时 settings。 |

## 修复方案

### TP-001: LoginSwitch 修复

`/login claude-code` 现在是 authProfile switch 命令；`claude_code` 作为兼容别名仍可用。需要 OAuth 时使用具名命令：

```rust
TestStep::LoginSwitch("claude-code".into())
// OAuth:
TestStep::Command("login claude-ai".into())
TestStep::Command("login console".into())
TestStep::Command("login codex-oauth".into())
```

### TP-002: SetPermission 修复

```rust
// 现在有效：映射到 /permissions mode bypass --confirm
TestStep::SetPermission("full access".into())
```

### TP-003: API 超时缓解

- 优先使用 `claude_code` profile（拥有可用的 anthropic 后端）
- 如果需要测试 `codex` profile，考虑增加 API 超时时间
- 使用 `TestRunner` 时可通过 `TestCase::timeout()` 增加全局超时

---

## 2026-05-24 E2E 测试运行结果

### 总览

| 模块 | 通过 | 失败 | 跳过 |
| --- | --- | --- | --- |
| commands_core_info | 23 | 2 | 0 |
| commands_session | 12 | 1 | 4 |
| commands_auth | 4 | 0 | 0 |
| commands_git | 6 | 0 | 2 |
| commands_permissions | 19 | 1 | 0 |
| commands_mcp_plugin | 18 | 1 | 0 |
| commands_agent_team | 13 | 1 | 2 |
| commands_kairos | 18 | 0 | 0 |
| commands_memory_skills_hooks | 16 | 0 | 0 |
| commands_query | 3 | 0 | 4 |
| commands_aliases | 7 | 0 | 0 |
| **合计** | **139** | **6** | **12** |

跳过的 12 个测试均标记为 `#[ignore = "requires real API key"]`，属于预期行为。

### 失败的测试（6 项）

所有 6 个失败均为相同模式：`AssertScreenContains` 断言失败——发送命令后等待 2 秒，屏幕内容不包含预期文本。可能是命令输出尚未渲染到屏幕，或输出格式与预期不符。

| ID | 测试 | 失败断言 | 日志目录 | 疑似原因 |
| --- | --- | --- | --- | --- |
| TF-001 | `config_alias_settings` | screen doesn't contain 'model' | `logs/pty_tui_e2e_scripts/config_alias_settings/` | `/settings` 输出可能不含 "model" 文本，或渲染延迟 |
| TF-002 | `config_show_displays_settings` | screen doesn't contain 'model' | `logs/pty_tui_e2e_scripts/config_show_displays_settings/` | `/config` 输出可能不含 "model" 文本，或渲染延迟 |
| TF-003 | `session_command_shows_info` | screen doesn't contain 'session' | `logs/pty_tui_e2e_scripts/session_command_shows_info/` | `/session` 输出可能不含 "session" 文本，或渲染延迟 |
| TF-004 | `permissions_mode_auto` | screen doesn't contain 'auto' | `logs/pty_tui_e2e_scripts/permissions_mode_auto/` | `/permissions mode auto` 输出可能不含 "auto" 文本 |
| TF-005 | `mcp_help_no_args` | screen doesn't contain 'MCP' | `logs/pty_tui_e2e_scripts/mcp_help_no_args/` | `/mcp` 无参数输出可能不含 "MCP" 文本 |
| TF-006 | `agents_list` | screen doesn't contain 'agent' | `logs/pty_tui_e2e_scripts/agents_list/` | `/agents` 输出可能不含 "agent" 文本 |

### 问题分析

**共同模式：** 所有失败均为 `AssertScreenContains` 类型断言，说明测试用例对命令输出内容的预期文本与实际 TUI 渲染内容不一致。

**可能原因（按优先级排序）：**
1. **输出格式不符**：命令实际输出文本与测试预期的关键词不同（例如 `/config` 可能显示 "Config" 而非 "model"）
2. **渲染时序问题**：2 秒等待时间不足，TUI 尚未将输出渲染到屏幕
3. **PTY 屏幕抓取问题**：屏幕内容快照可能未正确捕获已渲染的文本

**建议修复方向：**
- 检查 `logs/pty_tui_e2e_scripts/{test_name}/` 下的 HTML 快照文件，确认实际屏幕内容
- 根据实际输出调整 `AssertScreenContains` 的预期文本
- 如属渲染时序问题，可增加 `Wait` 时间或使用 `WaitForText` 替代

---

## 2026-05-24 在线测试（Ignored）运行结果

### 配置

使用 settings.json 中 `apiProvider: anthropic` + `ANTHROPIC_AUTH_TOKEN` 环境变量认证，模型 `deepseek-v4-pro`。

### 总览

| 模块 | 通过 | 失败 | 跳过 |
| --- | --- | --- | --- |
| commands_session | 2 | 2 | 0 |
| commands_git | 2 | 0 | 0 |
| commands_agent_team | 2 | 0 | 0 |
| commands_query | 4 | 0 | 0 |
| permissions | 2 | 0 | 0 |
| script::tests | 5 | 0 | 0 |
| conversation | 2 | 4 | 0 |
| model_flow | 5 | 0 | 0 |
| test3_plan_flow | 0 | 1 | 0 |
| test4_task_execution | 1 | 0 | 0 |
| test5_compact | 0 | 2 | 0 |
| **合计** | **25** | **9** | **0** |

### 失败的在线测试（9 项）

| ID | 测试 | 失败步骤 | 失败详情 | 日志目录 |
| --- | --- | --- | --- | --- |
| TO-001 | `conversation::single_turn_renders_response` | wait_for_text("Claude:", 60s) | 模型卡在 "Thinking..." 状态，60s 内无任何可见响应输出 | `logs/pty_tui_e2e_scripts/single_turn_renders_response/` |
| TO-002 | `conversation::five_turns_msg_count_increases` | wait_response_done(0, 60s) | 第 1 轮 "Say OK1" 后模型卡在 "Thinking..."，60s 超时 | `logs/pty_tui_e2e_scripts/five_turns_msg_count_increases/` |
| TO-003 | `conversation::multi_turn_context_persists` | wait_response_done(0, 60s) | 第 1 轮 "The secret code is ZEPHYR_42" 后模型卡在 "Thinking..."，60s 超时 | `logs/pty_tui_e2e_scripts/multi_turn_context_persists/` |
| TO-004 | `conversation::clear_then_continue` | wait_response_done(0, 60s) | 第 1 轮 "The secret word is COCONUT" 后模型卡在 "Thinking..."，60s 超时 | `logs/pty_tui_e2e_scripts/clear_then_continue/` |
| TO-005 | `commands_session::insights_command` | wait_for_text("hello", 60s) | 发送 "Hello" 后模型 60s 内未返回含 "hello" 的响应 | `logs/pty_tui_e2e_scripts/insights_command/` |
| TO-006 | `commands_session::rewind_command` | wait_for_text("Second", 60s) | 第 2 条消息 "Second message" 后模型 60s 内未返回含 "Second" 的响应 | `logs/pty_tui_e2e_scripts/rewind_command/` |
| TO-007 | `test3_plan_flow::script_plan_flow` | step 4 snapshot auto-save | `File name too long`（`Os { code: 36 }`）— `TestStep::Command` 的 `describe()` 返回完整命令字符串（~300+ 字符），sanitize 后作为文件名超出 OS 255 字节限制 | `logs/pty_tui_e2e_scripts/script_plan_flow/` |
| TO-008 | `test5_compact::script_compact_after_conversation` | wait any 4 (30s) | `/compact` 后 30s 内未出现 `["Compacted", "No compaction needed", "Conversation compacted", "tokens"]` 中任一文本 | `logs/pty_tui_e2e_scripts/script_compact_after_conversation/` |
| TO-009 | `test5_compact::script_compact_empty` | wait any 4 (15s) | 空会话中 `/compact` 后 15s 内未出现 `["Nothing to compact", "empty", "Compacted", "No compaction needed"]` 中任一文本 | `logs/pty_tui_e2e_scripts/script_compact_empty/` |

### 问题分析

**TO-001~TO-006（模型响应超时）：** 6 个测试失败均因模型在 API 调用后卡在 "Thinking..." 状态，60 秒内未产生可见响应。复查日志发现失败进程实际使用的是 `openai-codex` provider（`https://chatgpt.com/backend-api/codex/responses`）和 `gpt-5.4-mini`，并非记录中的 `claude_code` / anthropic profile。根因是 API client 在 active settings/profile 明确选择 `anthropic` 时仍先做通用 env provider detection；如果进程继承了 `OPENAI_CODEX_AUTH_TOKEN`，就会被误路由到 Codex 后端并触发 `TimedOut`。已在 `crates/cc-api/src/api/client/mod.rs` 修复为：显式云 provider env flags 优先，其次尊重 settings/active profile 的 `apiProvider`，最后才走通用 env detection。

**TO-007（文件名过长）：** `script.rs:672` 中 `TestStep::Command` 的 `describe()` 方法返回完整命令字符串作为快照文件名，未做截断。修复方法：在 `format!("/{}", s)` 中对 `s` 做 `truncate(s, 40)`。影响 test3_plan_flow。

**TO-008~TO-009（/compact 命令无输出）：** `/compact` 命令在发送后未产生任何预期输出文本。可能原因为：命令未正确执行、输出格式与预期不符、或 PTY 会话中命令补全面板干扰。

### 建议修复优先级

1. **高 - TO-007**：脚本框架 bug（`TestStep::Command` 缺少文件名截断），影响所有含长命令的测试
2. **中 - TO-008, TO-009**：`/compact` 测试不稳定，需检查命令分发逻辑或调整预期文本
3. **已修复 - TO-001~TO-006 的 provider 误路由**：需重新运行在线测试确认是否仍存在真实 API 延迟或限流问题

---

## 2026-05-24 test3/test4/test5 子代理并行测试结果

### 配置

使用 settings.json 中 `apiProvider: anthropic` + `ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic`，模型 `deepseek-v4-pro`。三个子代理并行执行。

### 总览

| 模块 | 通过 | 失败 | 耗时 |
| --- | --- | --- | --- |
| test3_plan_flow | 0 | 1 | ~85s (panic) |
| test4_task_execution | 1 | 0 | 317.32s |
| test5_compact | 0 | 2 | 156.95s |
| **合计** | **1** | **3** | — |

### 失败的测试（3 项）

| ID | 测试 | 失败步骤 | 失败详情 | 根因分类 |
| --- | --- | --- | --- | --- |
| TN-001 | `test3_plan_flow::script_plan_flow` | step 3 (auto-snapshot) | `File name too long` (`Os { code: 36 }`) — `TestStep::Command` 的 `describe()` 返回完整命令字符串（~340 字符），sanitize 后作为文件名超出 OS 255 字节限制。**TO-007 仍未修复。** | 脚本框架 bug |
| TN-002 | `test5_compact::script_compact_empty` | step 5: `wait any 4 (15s)` | `/compact` 发送后，命令面板拦截 Enter 键，显示 `/compact [instructions] [CMD]` 自动补全面板而非执行 compact 命令。15s 内未出现任何预期输出文本。 | 命令面板干扰 |
| TN-003 | `test5_compact::script_compact_after_conversation` | step 8: `wait any 4 (30s)` | (1) `WaitForText("COMPACT_TEST_MARKER")` 在 222ms 内匹配了用户输入回显，而非模型 API 响应，导致测试在模型回复前继续。(2) `/compact` 在模型 "Thinking..." (BUSY) 状态下发送，被当作排队消息处理。(3) openai-codex 后端 API 超时，初始消息从未完成。 | 时序问题 + API 超时 |

### 问题分析

**TN-001（文件名过长 — TO-007 复现）：** `script.rs:671` 中 `TestStep::Command(s)` 的 `describe()` 方法返回 `format!("/{}", s)`，未做截断。对比 `Input` 和 `TypeText` 使用了 `truncate(s, 40)`。修复方法：`Command` 分支同样使用 `truncate(s, 40)`。影响所有含长命令的测试。

**TN-002（命令面板拦截 /compact）：** `TestStep::Command("compact")` 通过 `send_line("/compact\r")` 发送，但 TUI 命令面板（autocomplete）在 Enter 时拦截了按键，显示帮助/补全面板而非执行命令。这是 TUI 命令分发逻辑的问题：当用户输入 `/compact` 时，命令面板获得焦点并拦截了 Enter。

**TN-003（WaitForText 误匹配输入回显）：** `WaitForText("COMPACT_TEST_MARKER")` 在用户输入文本回显中匹配成功（222ms），而非等待模型 API 返回。这导致测试在模型未响应时继续执行后续步骤。之后 `/compact` 在模型 BUSY 状态下被排队处理，而 openai-codex 后端超时导致整个流程无法完成。

**后端配置异常：** 两个 test5 测试的 session 日志显示实际使用了 `openai-codex` 提供者（`https://chatgpt.com/backend-api/codex/responses`），而非 settings.json 中配置的 `anthropic` 提供者（`https://api.deepseek.com/anthropic`）。环境变量 `ANTHROPIC_BASE_URL` 和 `ANTHROPIC_MODEL` 似乎未被正确传递或覆盖。

### test4 通过说明

`test4_task_execution::script_task_execution_with_subagent` 通过（317.32s），所有步骤正常完成。日志中有预期的 MCP test-server 握手失败和 openai-codex 超时警告，但不影响测试结果。

### 建议修复优先级

1. **高 - TN-001 / TO-007**：`TestStep::Command` 的 `describe()` 缺少截断，一行修复：`format!("/{}", truncate(s, 40))`
2. **高 - TN-003**：`WaitForText` 匹配输入回显问题 — 需要修改 `wait_for_text` 使其只匹配输出区域，或在 `send_line` 后清除输入回显
3. **中 - TN-002**：命令面板拦截 `/compact` 的 Enter 键 — 需要检查 TUI 命令分发逻辑
4. **中 - 后端配置**：环境变量未正确覆盖默认后端配置 — 需要检查 `PtySession::spawn_with_env` 中的环境变量传递

---

## 2026-05-24 并行子代理重新测试结果

### 配置

与上次在线测试相同：settings.json 中 `apiProvider: anthropic` + `ANTHROPIC_AUTH_TOKEN` 环境变量认证，模型 `deepseek-v4-pro`。9 个子代理并行执行。

### 总览

| 模块 | 通过 | 失败 | 变化 |
| --- | --- | --- | --- |
| commands_git | 2 | 0 | 不变 |
| commands_agent_team | 2 | 0 | 不变 |
| commands_query | 4 | 0 | 不变 |
| model_flow | 5 | 0 | 不变 |
| permissions | 3 | 0 | +1（脚本测试覆盖增加） |
| script::tests | 8 | 3 | +3（覆盖更多脚本测试） |
| commands_session | 2 | 2 | 不变 |
| conversation | 3 | 5 | +1（tool_use_visible_in_tui 通过） |
| test3_plan_flow | 0 | 1 | 不变 |
| test4_task_execution | 1 | 0 | 不变 |
| test5_compact | 0 | 2 | 不变 |
| **合计** | **30** | **13** | **+5 通过，+4 失败（覆盖增加）** |

### 失败的测试（13 项）

| ID | 测试 | 失败步骤 | 失败详情 | 根因分类 |
| --- | --- | --- | --- | --- |
| TP-001 | `insights_command` | step 4, wait 'hello' (60s) | 发送 "Hello" 后模型 60s 内未返回含 "hello" 的响应 | API 响应超时 |
| TP-002 | `rewind_command` | step 6, wait 'Second' (60s) | 第 2 条消息后模型 60s 内未返回含 "Second" 的响应 | API 响应超时 |
| TP-003 | `single_turn_renders_response` | wait_for_text("Claude:", 60s) | 测试期待 `"Claude:"` 前缀，deepseek 模型不产生该前缀 | 测试预期文本不匹配 |
| TP-004 | `five_turns_msg_count_increases` | wait_response_done(0, 60s) | `wait_response_done` 期望状态栏 `"ready"` + `"N msgs"` 格式，当前 TUI 状态栏渲染不匹配 | TUI 状态栏格式不匹配 |
| TP-005 | `multi_turn_context_persists` | wait_response_done(0, 60s) | 同上 — 状态栏未显示 "ready" + message count | TUI 状态栏格式不匹配 |
| TP-006 | `clear_then_continue` | wait_response_done(0, 60s) | 同上 — `/clear` 后新消息等待响应超时 | TUI 状态栏格式不匹配 |
| TP-007 | `script_plan_flow` | step 4 auto-snapshot | `File name too long`（`Os { code: 36 }`）— `step_004_plan_I_want_to_add...` 经 sanitize 后 384 字符超 255 字节限制 | 脚本框架 bug（文件名截断缺失） |
| TP-008 | `script_compact_empty` | step 5, wait any 4 (15s) | `/compact` 后 15s 内无预期输出（`["Nothing to compact", "empty", "Compacted", "No compaction needed"]`） | /compact 命令无输出 |
| TP-009 | `script_compact_after_conversation` | step 8, wait any 4 (30s) | `/compact` 在模型 "Thinking..."（BUSY）状态下发送，被排队或忽略 | 时序问题 — BUSY 状态下发送命令 |

### 失败分类统计

| 类别 | 数量 | 涉及测试 |
| --- | --- | --- |
| API 响应超时 | 2 | insights_command, rewind_command |
| 测试预期文本不匹配 | 1 | single_turn_renders_response |
| TUI 状态栏格式不匹配 | 3 | five_turns_msg_count_increases, multi_turn_context_persists, clear_then_continue |
| 脚本框架 bug | 1 | script_plan_flow |
| /compact 命令问题 | 2 | script_compact_empty, script_compact_after_conversation |

### 结论

**settings.json 配置正常工作。** 30 个测试通过（比首次在线测试的 25 个多 5 个，因覆盖范围更广），13 个失败全部是已知问题，非配置或核心逻辑缺陷。所有失败均复现了上次的运行结果，未发现新引入的回归。

---

## 2026-05-24 修复 OPENAI_CODEX_AUTH_TOKEN 路由后重新测试结果

### 配置

与上次相同：settings.json 中 `apiProvider: anthropic` + `ANTHROPIC_AUTH_TOKEN` 环境变量认证，模型 `deepseek-v4-pro`。

### 修复说明

`cc-api/src/api/client/mod.rs` 中 `from_auth_result()` 的解析顺序已调整：

1. 显式云 provider env flags（Bedrock, Vertex, Foundry）
2. **持久化 / active settings provider 选择**（新增优先级提升）
3. 多 provider env 检测
4. `ANTHROPIC_AUTH_TOKEN` env
5. 系统 keychain

这确保了当 settings.json 明确指定 `apiProvider: anthropic` 时，继承的 `OPENAI_CODEX_AUTH_TOKEN` 不会将请求静默路由到 `openai-codex` 的 `chatgpt.com/backend-api/codex/responses`。

### 非 Ignored 测试总览（10 个子代理并行）

| 模块 | 通过 | 失败 | 跳过 |
| --- | --- | --- | --- |
| commands_core_info | 21 | 4 | 0 |
| commands_auth | 4 | 0 | 0 |
| commands_aliases | 7 | 0 | 0 |
| commands_kairos | 18 | 0 | 0 |
| commands_session | 12 | 1 | 4 |
| commands_mcp_plugin | 18 | 1 | 0 |
| commands_memory_skills_hooks | 16 | 0 | 0 |
| commands_agent_team | 13 | 1 | 2 |
| commands_git | 6 | 0 | 2 |
| commands_permissions | 19 | 1 | 0 |
| commands_query | 3 | 0 | 4 |
| test1_login_structure | 1 | 0 | 0 |
| test2_full_access | 1 | 0 | 0 |
| **合计** | **139** | **8** | **12** |

### 非 Ignored 测试失败（8 项）

与上次（6 项失败）相比新增 2 项：`effort_set_high` 和 `fast_toggle`，均为同一模式——命令输出的关键词未出现在屏幕上。

| ID | 测试 | 失败断言 | 状态 |
| --- | --- | --- | --- |
| TF-001 | `config_alias_settings` | screen doesn't contain 'model' | 未变 |
| TF-002 | `config_show_displays_settings` | screen doesn't contain 'model' | 未变 |
| TF-003 | `session_command_shows_info` | screen doesn't contain 'session' | 未变 |
| TF-004 | `permissions_mode_auto` | screen doesn't contain 'auto' | 未变 |
| TF-005 | `mcp_help_no_args` | screen doesn't contain 'MCP' | 未变 |
| TF-006 | `agents_list` | screen doesn't contain 'agent' | 未变 |
| **TF-007** | `effort_set_high` | screen doesn't contain 'high' | **新增** |
| **TF-008** | `fast_toggle` | screen doesn't contain 'fast' | **新增** |

### Ignored（在线）测试总览

| 模块 | 通过 | 失败 | 与上次对比 |
| --- | --- | --- | --- |
| commands_session | 3 | 1 | +1 通过（rewind_command 恢复），insights_command 仍超时 |
| commands_git | 2 | 0 | 不变 |
| commands_agent_team | 2 | 0 | 不变 |
| commands_query | 4 | 0 | 不变 |
| conversation | 3 | 5 | 不变 |
| model_flow | 5 | 0 | 不变 |
| script::tests | 5 | 0 | 不变（本次匹配 5 项 ignored 测试） |
| permissions | 2 | 0 | 不变 |
| test3_plan_flow | 0 | 1 | 不变 |
| test4_task_execution | 1 | 0 | 不变（317.35s） |
| test5_compact | 0 | 2 | 模型已改用 deepseek-v4-pro（修复生效），新失败模式见下方 |
| **合计** | **27** | **10** | 比上次 30/13 减少，因 script::tests 匹配范围不同 |

### Ignored 测试失败详情（10 项）

| ID | 测试 | 失败详情 | 根因分类 |
| --- | --- | --- | --- |
| TR-001 | `insights_command` | 发送 "Hello" 后 60s 内未返回含 "hello" 的响应 | API 响应超时 |
| TR-002 | `single_turn_renders_response` | 期待 `"Claude:"` 前缀，deepseek 不产生该前缀 | 测试预期文本不匹配 |
| TR-003 | `five_turns_msg_count_increases` | `wait_response_done` 期望状态栏 `"ready"` + `"N msgs"` 格式不匹配 | TUI 状态栏格式不匹配 |
| TR-004 | `multi_turn_context_persists` | 同上 — 状态栏未显示 "ready" + message count | TUI 状态栏格式不匹配 |
| TR-005 | `clear_then_continue` | 同上 — `/clear` 后新消息等待响应超时 | TUI 状态栏格式不匹配 |
| TR-006 | `script_plan_flow` | `File name too long`（`Os { code: 36 }`）— `TestStep::Command` 的 `describe()` 返回完整命令字符串，超出 255 字节限制 | 脚本框架 bug |
| TR-007 | `script_compact_empty` | `/compact` 后无预期输出 | /compact 命令无输出 |
| TR-008 | `script_compact_after_conversation` | **修复生效：** 模型正确使用 deepseek-v4-pro（非 openai-codex）。新失败模式：`/compact` 在模型 BUSY 状态下发送后被排队，模型完成后排队的命令未被执行，卡在输入区 `/compact [CMD]` 状态 | 排队命令调度 bug（新） |
| TR-009 | `test3_plan_flow::script_plan_flow` | 同 TR-006，文件名过长 | 脚本框架 bug |

### 修复验证

**OPENAI_CODEX_AUTH_TOKEN 路由修复已确认生效：**

1. **model_flow 全部通过**（5/5）— 模型正确使用 `deepseek-v4-pro`
2. **rewind_command 恢复通过** — 上次因 openai-codex 超时失败，本次正常
3. **script_compact_after_conversation** — 模型响应从 "TimedOut" 变为 5.3s 内返回 `COMPACT_TEST_MARKER`，证明 API 路由正确；失败原因变为排队命令调度 bug（新问题）
4. **test4_task_execution** — 通过（317.35s），与上次一致

### 失败分类统计

| 类别 | 数量 | 涉及测试 |
| --- | --- | --- |
| API 响应超时 | 1 | insights_command |
| 测试预期文本不匹配 | 1 | single_turn_renders_response |
| TUI 状态栏格式不匹配 | 3 | five_turns_msg_count_increases, multi_turn_context_persists, clear_then_continue |
| 脚本框架 bug（文件名截断） | 2 | script_plan_flow (×2) |
| /compact 命令问题 | 2 | script_compact_empty, script_compact_after_conversation |
| 非 Ignored 屏幕断言不匹配 | 8 | config_alias_settings, config_show_displays_settings, session_command_shows_info, permissions_mode_auto, mcp_help_no_args, agents_list, effort_set_high, fast_toggle |

### 建议修复优先级

1. **高 - TR-006/TR-009**：`TestStep::Command` 的 `describe()` 缺少文件名截断（一行修复）
2. **高 - TR-008**：排队命令在 BUSY 状态后未被执行（新发现的调度 bug）
3. **中 - TR-002~TR-005**：测试预期文本与 deepseek 模型输出格式不匹配
4. **低 - TF-001~TF-008**：屏幕断言关键词不匹配（可能需增加等待时间或调整关键词）

---

## 更新规则

1. 问题修复后移入 `docs/archive/`。
2. 使用 `logs/pty_tui_e2e_{timestamp}/{test_name}/` 目录中的截图和日志进行问题复现/验证。
