# Claude Code Rust 重写计划

> 最后更新: 2026-04-01
> 结构对比详见: [`STRUCTURE_DIFF.md`](STRUCTURE_DIFF.md)

## 原始项目概况

- **语言**: TypeScript/React (Ink 终端渲染)
- **运行时**: Bun
- **文件数**: ~1896 个 .ts/.tsx 文件, 35 个顶级子目录
- **代码行数**: ~91,000 行
- **核心架构**: Generator-based 流式查询状态机 + 工具系统 + 终端 UI

## Rust 实现现状

- **文件数**: 96 个 .rs 文件, 16 个顶级子目录
- **代码行数**: ~16,461 行 (占 TS 的 ~18%)
- **测试数**: 164 个测试，全部通过
- **完成状态**: 核心状态机 + 本地工具 + UI + 会话持久化完整；API 客户端 / MCP 为脚手架
- **目录覆盖率**: 16/35 顶级目录已存在 (~46%)

---

## 状态图例

| 标记 | 含义 |
|------|------|
| ✅ | 完整实现：有真实逻辑、条件分支、算法，可直接使用 |
| 🔧 | 脚手架：类型/trait 已定义，函数体为占位符或 Phase 1 简化 |
| 📦 | 仅模块声明：mod.rs 或类型定义，无业务逻辑 |
| ❌ | 缺失：文档要求但尚未创建 |

---

## 架构参考文档

| 文档 | 对应实现 | 覆盖度 |
|------|---------|--------|
| [`LIFECYCLE_STATE_MACHINE.md`](LIFECYCLE_STATE_MACHINE.md) | main.rs, shutdown.rs, engine/, query/ | ✅ Phase A-I 完整 |
| [`QUERY_ENGINE_SESSION_LIFECYCLE.md`](QUERY_ENGINE_SESSION_LIFECYCLE.md) | engine/lifecycle.rs, sdk_types.rs | ✅ Phase A-E 完整 |
| [`TOOL_EXECUTION_STATE_MACHINE.md`](TOOL_EXECUTION_STATE_MACHINE.md) | tools/execution.rs, orchestration.rs, hooks.rs | ✅ 管线完整，hooks 为脚手架 |
| [`COMPACTION_RETRY_STATE_MACHINE.md`](COMPACTION_RETRY_STATE_MACHINE.md) | compact/pipeline.rs, compaction.rs | ✅ 决策 + 管线完整，full compact 需 API |
| [`STRUCTURE_DIFF.md`](STRUCTURE_DIFF.md) | 目录结构对比 | 📋 TS vs Rust 全量对比 |

---

## 目录结构映射总览

Rust 重组了 TS 的目录结构，主要变化:

| TS 原始位置 | Rust 新位置 | 变化类型 |
|---|---|---|
| `services/api/` | `api/` | 提升为顶级 |
| `services/analytics/` | `analytics/` | 提升为顶级 |
| `services/compact/` | `compact/` | 提升为顶级 |
| `services/mcp/` | `mcp/` | 提升为顶级 |
| `services/oauth/` + `utils/secureStorage/` | `auth/` | 合并提升 |
| `utils/permissions/` | `permissions/` | 提升为顶级 |
| `utils/settings/` + config 相关 | `config/` | 合并提升 |
| `components/` + `ink/` | `ui/` | 合并简化 |
| `memdir/` + 会话相关 | `session/` | 合并重命名 |
| `QueryEngine.ts` + `query.ts` | `engine/` | 提取为模块 |
| `state/` | `types/app_state.rs` | 合入 types |
| `entrypoints/cli.tsx` + `main.tsx` | `main.rs` | 合并 |

---

## Phase 0: 类型基础 (无网络依赖) — ✅ 完成

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P0.1 | Message 枚举 | `types/message.rs` | ✅ | ContentBlock, Usage, 7 种消息类型, QueryYield |
| P0.2 | Tool trait | `types/tool.rs` | ✅ | Tool trait 18 个方法, ToolUseContext, FileStateCache |
| P0.3 | 循环状态 | `types/state.rs` | ✅ | QueryLoopState, AutoCompactTracking, BudgetTracker |
| P0.4 | 查询配置 | `types/config.rs` | ✅ | QueryParams, QueryEngineConfig, ThinkingConfig |
| P0.5 | 应用状态 | `types/app_state.rs` | ✅ | AppState, SettingsJson |
| P0.6 | 状态转换 | `types/transitions.rs` | ✅ | Terminal (10 种), Continue (7 种) |

## Phase 1: 状态机骨架 — ✅ 完成

| # | 模块 | 文件 | 状态 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|------|
| P1.1 | query loop | `query/loop_impl.rs` | ✅ | 1025 | 4 | 完整 8 步循环 + 恢复路径 |
| P1.2 | 依赖注入 | `query/deps.rs` | ✅ | 147 | 0 | QueryDeps trait (可 mock) |
| P1.3 | token 预算 | `query/token_budget.rs` | ✅ | 65 | 0 | checkTokenBudget + diminishing returns |
| P1.4 | stop hooks | `query/stop_hooks.rs` | ✅ | 164 | 4 | has_tool_use, extract_tool_uses |
| P1.5 | QueryEngine | `engine/lifecycle.rs` | ✅ | 1237 | 8 | Phase A-E 完整分发 + QueryEngineDeps |
| P1.6 | SDK 类型 | `engine/sdk_types.rs` | ✅ | 143 | 0 | SdkMessage 7 种变体 |
| P1.7 | 输入处理 | `engine/input_processing.rs` | ✅ | 156 | 5 | 斜杠命令解析 + UserMessage 构建 |
| P1.8 | 系统提示 | `engine/system_prompt.rs` | ✅ | 157 | 4 | 默认/自定义/追加提示词 + 工具描述 |
| P1.9 | 结果判定 | `engine/result.rs` | ✅ | 234 | 6 | isResultSuccessful + extractTextResult |
| P1.10 | CLI 入口 | `main.rs` | ✅ | 468 | 0 | clap CLI + 快速路径 + REPL + print mode |
| P1.11 | 关闭清理 | `shutdown.rs` | ✅ | 129 | 0 | SIGINT handler + abort + transcript flush |

## Phase 2: 本地工具系统 — ✅ 核心完成

| # | 模块 | 文件 | 状态 | 行数 | 说明 |
|---|------|------|------|------|------|
| P2.1 | 工具注册 | `tools/registry.rs` | ✅ | 99 | get_all_tools + find_tool_by_name |
| P2.2 | 并发编排 | `tools/orchestration.rs` | ✅ | 534 | partitionToolCalls + 并行/串行批次 |
| P2.3 | 执行管线 | `tools/execution.rs` | ✅ | 604 | run_tool_use() 8 步管线 |
| P2.4 | Hook 基础 | `tools/hooks.rs` | 🔧 | 153 | 类型定义完整，执行逻辑占位 |
| P2.5 | Bash | `tools/bash.rs` | ✅ | 199 | 进程执行 + timeout + 输出捕获 |
| P2.6 | FileRead | `tools/file_read.rs` | ✅ | 236 | 二进制检测 + 行号 + offset/limit |
| P2.7 | FileWrite | `tools/file_write.rs` | ✅ | 157 | 路径验证 + 内容写入 |
| P2.8 | FileEdit | `tools/file_edit.rs` | ✅ | 230 | 字符串替换 + replace_all |
| P2.9 | Glob | `tools/glob_tool.rs` | ✅ | 199 | glob 匹配 + 修改时间排序 |
| P2.10 | Grep | `tools/grep.rs` | ✅ | 185 | 正则搜索 + 上下文行 + 输出模式 |
| P2.11 | NotebookEdit | `tools/notebook_edit.rs` | 🔧 | 97 | JSON 解析框架，处理不完整 |
| P2.12 | AskUser | `tools/ask_user.rs` | 🔧 | 62 | 占位符返回，无真实 UI 集成 |
| P2.13 | ToolSearch | `tools/tool_search.rs` | 🔧 | 73 | 占位符，搜索逻辑未实现 |
| P2.14 | Tasks | `tools/tasks.rs` | ✅ | 189 | 内存 HashMap CRUD |

## Phase 3: 权限与配置 — ✅ 完成

| # | 模块 | 文件 | 状态 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|------|
| P3.1 | 规则引擎 | `permissions/rules.rs` | ✅ | 274 | 7 | deny→allow→ask 优先级 + glob 匹配 |
| P3.2 | 决策状态机 | `permissions/decision.rs` | ✅ | 459 | 7 | 模式匹配 + denial tracker |
| P3.3 | 危险检测 | `permissions/dangerous.rs` | ✅ | 218 | 11 | 16 种危险模式正则 |
| P3.4 | 设置加载 | `config/settings.rs` | ✅ | 295 | 3 | 3 层合并 (global → project → env) |
| P3.5 | CLAUDE.md | `config/claude_md.rs` | ✅ | ~100 | 0 | 文件发现 + 上下文注入 |

## Phase 4: 上下文管理 — ✅ 管线完成

| # | 模块 | 文件 | 状态 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|------|
| P4.1 | 消息工具 | `compact/messages.rs` | ✅ | 306 | 4 | normalizeForAPI + 交替模式 |
| P4.2 | 微压缩 | `compact/microcompact.rs` | ✅ | 261 | 2 | 阈值裁剪 + 最近 N 结果保护 |
| P4.3 | 历史裁剪 | `compact/snip.rs` | ✅ | 217 | 2 | turn 识别 + 边界消息 |
| P4.4 | 结果预算 | `compact/tool_result_budget.rs` | 🔧 | 224 | 2 | 磁盘持久化框架，async I/O 部分 |
| P4.5 | 管线编排 | `compact/pipeline.rs` | ✅ | 269 | 3 | snip → micro → autocompact 编排 |
| P4.6 | 压缩决策 | `compact/auto_compact.rs` | ✅ | 52 | 3 | 80% 阈值判定 |
| P4.7 | 全量压缩 | `compact/compaction.rs` | ✅ | 426 | 8 | 决策 + 跟踪 + prompt + boundary |
| P4.8 | token 估算 | `utils/tokens.rs` | ✅ | 170 | 5 | 4 chars/token 启发式 |
| P4.9 | 文件缓存 | `utils/file_state_cache.rs` | ✅ | 193 | 0 | LRU 缓存 + hash/timestamp |

## Phase 5: 终端 UI — ✅ 完成

| # | 模块 | 文件 | 状态 | 行数 | 说明 |
|---|------|------|------|------|------|
| P5.1 | 主框架 | `ui/app.rs` | ✅ | 409 | ratatui 布局 + 消息管理 + 快捷键 |
| P5.2 | 消息渲染 | `ui/messages.rs` | ✅ | 404 | 7 种消息类型渲染 + 工具调用格式化 |
| P5.3 | 输入框 | `ui/prompt_input.rs` | ✅ | 250 | Ctrl+U/A/W 编辑 + 光标管理 |
| P5.4 | 加载动画 | `ui/spinner.rs` | ✅ | 95 | 帧动画 + 状态管理 |
| P5.5 | 权限对话框 | `ui/permissions.rs` | ✅ | 244 | 交互式选择 + 键盘导航 |
| P5.6 | Diff 渲染 | `ui/diff.rs` | ✅ | 96 | TextDiff + 颜色样式 |
| P5.7 | Markdown | `ui/markdown.rs` | ✅ | 259 | pulldown_cmark 解析 + 样式映射 |
| P5.8 | 主题 | `ui/theme.rs` | ✅ | 116 | 颜色/修饰符组合 |

## Phase 6: 会话持久化 — ✅ 完成

| # | 模块 | 文件 | 状态 | 行数 | 说明 |
|---|------|------|------|------|------|
| P6.1 | 会话存储 | `session/storage.rs` | ✅ | 329 | JSON 持久化 + NDJSON 序列化 |
| P6.2 | 对话记录 | `session/transcript.rs` | ✅ | 188 | NDJSON append + sync |
| P6.3 | 会话恢复 | `session/resume.rs` | ✅ | 54 | cwd 匹配 + 消息加载 |

## Phase 7: 命令系统 — 🔧 部分完成

| # | 模块 | 文件 | 状态 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|------|
| P7.1 | 注册表 | `commands/mod.rs` | ✅ | 187 | 6 | 别名 + 参数解析 |
| P7.2 | /compact | `commands/compact.rs` | 🔧 | ~30 | 0 | 框架，需 API |
| P7.3 | /clear | `commands/clear.rs` | ✅ | ~20 | 0 | 直接实现 |
| P7.4 | /help | `commands/help.rs` | ✅ | ~30 | 0 | 列出所有命令 |
| P7.5 | /config | `commands/config_cmd.rs` | 🔧 | ~30 | 0 | 框架 |
| P7.6 | /diff | `commands/diff.rs` | ✅ | ~30 | 0 | git diff |

## Phase 8: 高级本地工具 — 🔧 脚手架

| # | 模块 | 文件 | 状态 | 行数 | 说明 |
|---|------|------|------|------|------|
| P8.1 | Agent | `tools/agent.rs` | 🔧 | 87 | 框架，需子进程/子 QueryEngine |
| P8.2 | PlanMode | `tools/plan_mode.rs` | 🔧 | 51 | 模式切换占位 |
| P8.3 | Worktree | `tools/worktree.rs` | 🔧 | 75 | git worktree 框架 |
| P8.4 | Skill | `tools/skill.rs` | 🔧 | 57 | 技能调用占位 |

---

## Phase 9-13: 网络功能 — 🔧 脚手架 (低优先级)

这些模块依赖网络。当前采用 **本地优先** 策略。

### Phase 9: API 客户端

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P9.1 | 客户端 | `api/client.rs` | 🔧 | HTTP 框架，call 方法未实现 |
| P9.2 | 流解析 | `api/streaming.rs` | ✅ | SSE 解析 + StreamAccumulator |
| P9.3 | 重试 | `api/retry.rs` | ✅ | 错误分类 + 指数退避 |
| P9.4 | 提供商 | `api/providers.rs` | ✅ | Provider trait + 4 家抽象 |

### Phase 10: 认证

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P10.1 | 入口 | `auth/mod.rs` | ✅ | AuthMethod 解析 + env 检测 |
| P10.2 | API Key | `auth/api_key.rs` | 🔧 | feature-gated 存储 |
| P10.3 | Token | `auth/token.rs` | ✅ | 文件 I/O + 过期检查 |

### Phase 11: MCP 协议

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P11.1 | 客户端 | `mcp/client.rs` | 🔧 | 5 个方法均 stub |
| P11.2 | 发现 | `mcp/discovery.rs` | ✅ | JSON 配置加载 |
| P11.3 | 工具 | `mcp/tools.rs` | 🔧 | 代理 trait impl 但委托 stub |

### Phase 12: 网络工具

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P12.1 | WebFetch | `tools/web_fetch.rs` | 🔧 | feature-gated 占位 |
| P12.2 | WebSearch | `tools/web_search.rs` | 🔧 | feature-gated 占位 |

### Phase 13: 远程/遥测

| # | 模块 | 文件 | 状态 | 说明 |
|---|------|------|------|------|
| P13.1 | 遥测 | `analytics/mod.rs` | 🔧 | 类型定义 + 占位日志 |
| P13.2 | 远程会话 | `remote/session.rs` | 🔧 | bail!("not yet implemented") |

---

## Phase 14: 目录结构对齐 — ❌ 新增

> 基于 [`STRUCTURE_DIFF.md`](STRUCTURE_DIFF.md) 的分析，以下是需要新增的目录和模块，
> 按 **本地优先** 原则排序。

### Phase 14A: 必要本地补充 (无网络依赖)

这些是当前 Rust 实现中明显缺失的本地功能。

| # | 新模块 | 对应 TS | 文件 | 预估行数 | 说明 |
|---|--------|---------|------|---------|------|
| P14A.1 | 常量定义 | `constants/` (21 文件) | `config/constants.rs` | ~200 | OAuth URL, API 版本, 模型 ID, 限制值等 |
| P14A.2 | utils/bash | `utils/bash/` + `utils/bash/specs/` | `utils/bash/` | ~300 | 命令解析, 危险检测规格, shell 转义 |
| P14A.3 | utils/git | `utils/git/` | `utils/git.rs` | ~200 | git status/diff/log/branch 封装 |
| P14A.4 | utils/shell | `utils/shell/` | `utils/shell.rs` | ~100 | shell 检测, 环境初始化 |
| P14A.5 | utils/messages | `utils/messages/` | 扩展 `utils/messages.rs` | ~150 | 消息格式化, 截断, 计数 |
| P14A.6 | keybindings | `keybindings/` (14 文件) | `ui/keybindings.rs` | ~200 | 快捷键注册, 自定义绑定 |
| P14A.7 | vim 模式 | `vim/` (5 文件) | `ui/vim.rs` | ~300 | hjkl 导航, 模式切换 |
| P14A.8 | 迁移系统 | `migrations/` (11 文件) | `session/migrations.rs` | ~150 | 会话数据格式迁移 |
| P14A.9 | 任务子系统 | `tasks/` (12 文件) | `tasks/` | ~400 | LocalShellTask, LocalAgentTask |
| P14A.10 | 内存系统 | `memdir/` (8 文件) | 扩展 `session/` | ~200 | CLAUDE.md 记忆读写 |

### Phase 14B: 命令系统补全

TS 有 85+ 命令，当前 Rust 仅 5 个。按使用频率分批实现。

**第一批 — 高频核心命令 (无网络依赖)**

| # | 命令 | 对应 TS | 预估行数 | 说明 |
|---|------|---------|---------|------|
| P14B.1 | /exit | `commands/exit/` | ~20 | 退出 REPL |
| P14B.2 | /version | `commands/version.ts` | ~20 | 版本号 |
| P14B.3 | /model | `commands/model/` | ~60 | 切换模型 |
| P14B.4 | /cost | `commands/cost/` | ~40 | 显示 token 用量 |
| P14B.5 | /session | `commands/session/` | ~80 | 会话列表/切换 |
| P14B.6 | /resume | `commands/resume/` | ~60 | 恢复会话 |
| P14B.7 | /files | `commands/files/` | ~40 | 列出引用文件 |
| P14B.8 | /context | `commands/context/` | ~80 | 上下文管理 |
| P14B.9 | /permissions | `commands/permissions/` | ~60 | 权限查看/修改 |
| P14B.10 | /hooks | `commands/hooks/` | ~60 | hook 管理 |

**第二批 — 中频功能命令**

| # | 命令 | 对应 TS | 预估行数 | 说明 |
|---|------|---------|---------|------|
| P14B.11 | /commit | `commands/commit.ts` | ~80 | git commit |
| P14B.12 | /review | `commands/review.ts` | ~100 | 代码审查 |
| P14B.13 | /branch | `commands/branch/` | ~40 | 分支管理 |
| P14B.14 | /export | `commands/export/` | ~60 | 导出对话 |
| P14B.15 | /rename | `commands/rename/` | ~40 | 重命名会话 |
| P14B.16 | /stats | `commands/stats/` | ~60 | 使用统计 |
| P14B.17 | /effort | `commands/effort/` | ~40 | 思考力度 |
| P14B.18 | /fast | `commands/fast/` | ~30 | 快速模式 |
| P14B.19 | /memory | `commands/memory/` | ~60 | 记忆管理 |
| P14B.20 | /plan | `commands/plan/` | ~50 | 计划模式 |

**第三批 — 低频/高级命令 (可推迟)**

```
/add-dir, /agents, /bridge, /chrome, /color, /copy, /desktop,
/doctor, /feedback, /ide, /init, /install, /keybindings,
/login, /logout, /mcp, /mobile, /plugin, /privacy-settings,
/release-notes, /remote-env, /remote-setup, /rewind,
/sandbox-toggle, /skills, /status, /stickers, /tag, /tasks,
/terminalSetup, /theme, /thinkback, /upgrade, /usage, /vim, /voice
```

### Phase 14C: 缺失工具补全

| # | 工具 | 对应 TS | 预估行数 | 优先级 | 说明 |
|---|------|---------|---------|--------|------|
| P14C.1 | SendMessage | `SendMessageTool/` | ~80 | 高 | Agent 间通信 |
| P14C.2 | LSP | `LSPTool/` | ~200 | 中 | 需 lsp feature |
| P14C.3 | MCP | `MCPTool/` | ~100 | 中 | 需 mcp feature |
| P14C.4 | PowerShell | `PowerShellTool/` | ~100 | 中 | Windows 支持 |
| P14C.5 | Sleep | `SleepTool/` | ~20 | 低 | 简单 |
| P14C.6 | Brief | `BriefTool/` | ~30 | 低 | 输出简化 |
| P14C.7 | Config | `ConfigTool/` | ~50 | 低 | 设置修改 |
| P14C.8 | RemoteTrigger | `RemoteTriggerTool/` | ~80 | 低 | 需 network |
| P14C.9 | ScheduleCron | `ScheduleCronTool/` | ~80 | 低 | 需 network |
| P14C.10 | REPL | `REPLTool/` | ~100 | 低 | 嵌入式 REPL |

### Phase 14D: services 子模块补全 (中优先级)

| # | 模块 | 对应 TS | 预估行数 | 说明 |
|---|------|---------|---------|------|
| P14D.1 | LSP 服务 | `services/lsp/` | ~300 | 代码导航, 定义跳转 |
| P14D.2 | 插件系统 | `services/plugins/` + `plugins/` | ~200 | 插件加载/管理 |
| P14D.3 | 技能系统 | `skills/` (23 文件) | ~300 | 技能发现/执行 |
| P14D.4 | SessionMemory | `services/SessionMemory/` | ~100 | 会话记忆服务 |
| P14D.5 | 提示建议 | `services/PromptSuggestion/` | ~80 | 输入补全 |
| P14D.6 | 工具摘要 | `services/toolUseSummary/` | ~60 | 工具使用统计 |

### Phase 14E: 网络/远程目录 (低优先级)

按计划这些降优先级，但列出以保持完整性。

| # | 模块 | 对应 TS | 文件数 | 说明 |
|---|------|---------|--------|------|
| P14E.1 | bridge | `bridge/` | 31 | 远程控制桥接 |
| P14E.2 | cli transports | `cli/transports/` | 6 | SSE, WebSocket, Worker |
| P14E.3 | coordinator | `coordinator/` | 1 | 多 Agent 协调 |
| P14E.4 | server | `server/` | 3 | 服务器模式 |
| P14E.5 | remote 扩展 | `remote/` | 4 | 云容器支持 |
| P14E.6 | OAuth 流程 | `services/oauth/` | ~5 | 完整 OAuth |
| P14E.7 | 远程设置同步 | `services/remoteManagedSettings/` + `services/settingsSync/` | ~5 | MDM + 同步 |
| P14E.8 | 遥测扩展 | `services/analytics/` | ~5 | 完整遥测管线 |

---

## 统计总览

```
实现完成度:

  Phase 0  类型基础       ██████████ 100% (6/6)
  Phase 1  状态机骨架     ██████████ 100% (11/11)
  Phase 2  本地工具       ███████░░░  70% (10/14 完整, 4 脚手架)
  Phase 3  权限与配置     ██████████ 100% (5/5)
  Phase 4  上下文管理     █████████░  89% (8/9 完整, 1 脚手架)
  Phase 5  终端 UI        ██████████ 100% (8/8)
  Phase 6  会话持久化     ██████████ 100% (3/3)
  Phase 7  命令系统       ████████░░  67% (4/6 完整, 2 脚手架)
  Phase 8  高级工具       ██░░░░░░░░   0% (0/4 完整, 4 脚手架)
  Phase 9  API 客户端     ███████░░░  75% (3/4 完整, 1 脚手架)
  Phase 10 认证           ██████░░░░  67% (2/3 完整, 1 脚手架)
  Phase 11 MCP            ███░░░░░░░  33% (1/3 完整, 2 脚手架)
  Phase 12 网络工具       ░░░░░░░░░░   0% (0/2, 2 脚手架)
  Phase 13 远程/遥测      ░░░░░░░░░░   0% (0/2, 2 脚手架)
  Phase 14 目录对齐       ░░░░░░░░░░   0% (新增)

  文件总数: 96 .rs 文件 (目标: ~200+)
  代码行数: ~16,461 行 (目标: ~30,000+)
  测试数量: 164 个 (全部通过)
  目录覆盖: 16/35 TS 顶级目录 (46%)
  命令覆盖: 5/85+ (6%)
  工具覆盖: 20/42 (48%, 含脚手架)
```

---

## 下一步优先级 (P1 = 紧急, P4 = 可推迟)

### P1 — 使系统端到端可用

> 详见 [`P1_EXECUTION_PLAN.md`](P1_EXECUTION_PLAN.md)

| 任务 | 文件 | 预估 | 依赖 |
|------|------|------|------|
| API 客户端接入真实 Anthropic API | `api/client.rs` | 250 行 | network feature |
| Hooks 真实执行 (子进程 + JSON) | `tools/hooks.rs` | 350 行 | 无 |
| tool_result_budget 完成 async I/O | `compact/tool_result_budget.rs` | 30 行 | 无 |
| /compact 命令接 API 压缩 | `commands/compact.rs` | 120 行 | P9.1 |

### P2 — 功能完整性 + 脚手架提升

| 任务 | 文件 | 预估 | 依赖 |
|------|------|------|------|
| Agent 工具 (子 QueryEngine 派生) | `tools/agent.rs` | 200 行 | 无 |
| AskUser 真实终端交互 | `tools/ask_user.rs` | 80 行 | 无 |
| ToolSearch 工具搜索 | `tools/tool_search.rs` | 50 行 | 无 |
| NotebookEdit 完整 ipynb | `tools/notebook_edit.rs` | 100 行 | 无 |
| CLAUDE.md 记忆注入到系统提示 | `engine/system_prompt.rs` | 100 行 | 无 |
| MCP 客户端实现 | `mcp/client.rs` | 300 行 | network |

### P3 — 目录结构对齐 (Phase 14A + 14B 第一批)

| 任务 | 文件 | 预估 | 依赖 |
|------|------|------|------|
| 常量模块 | `config/constants.rs` | 200 行 | 无 |
| utils/git 封装 | `utils/git.rs` | 200 行 | 无 |
| utils/bash 解析 | `utils/bash/` | 300 行 | 无 |
| utils/shell | `utils/shell.rs` | 100 行 | 无 |
| 高频命令 10 个 | `commands/*.rs` | 600 行 | 无 |
| 任务子系统 | `tasks/` | 400 行 | 无 |

### P4 — 网络/远程/遥测 + 其余

| 任务 | 文件 | 预估 | 依赖 |
|------|------|------|------|
| AWS Bedrock 提供商 | `api/providers.rs` | 150 行 | aws-sdk |
| GCP Vertex 提供商 | `api/providers.rs` | 150 行 | gcp_auth |
| OAuth 认证流程 | `auth/` | 300 行 | oauth2 |
| WebFetch / WebSearch | `tools/web_*.rs` | 200 行 | reqwest |
| 远程会话 | `remote/` | 200 行 | websocket |
| 遥测 | `analytics/` | 100 行 | 无 |
| bridge/coordinator/server | 新目录 | 1000+ 行 | network |
| 插件/技能系统 | `plugins/`, `skills/` | 500 行 | 无 |
| LSP 集成 | `tools/lsp.rs` + 服务 | 500 行 | lsp feature |

---

## Rust 与 TypeScript 的关键映射

| TypeScript 概念 | Rust 对应 | 所在文件 |
|----------------|-----------|---------|
| `AsyncGenerator<T>` | `impl Stream<Item = T>` | query/loop_impl.rs |
| `interface Tool` | `trait Tool` | types/tool.rs |
| `type Message = A \| B \| C` | `enum Message { A(..), B(..), C(..) }` | types/message.rs |
| `ToolUseContext` (大对象) | `struct ToolUseContext` (Arc 共享) | types/tool.rs |
| `DeepImmutable<AppState>` | `Arc<RwLock<AppState>>` | engine/lifecycle.rs |
| `AbortController` | `tokio::sync::watch<bool>` | utils/abort.rs |
| `feature('FLAG')` | `#[cfg(feature = "flag")]` | Cargo.toml |
| `z.infer<Schema>` (Zod) | `#[derive(Deserialize)]` struct | 各工具模块 |
| React/Ink (UI) | ratatui + crossterm | ui/ |
| `runPreToolUseHooks()` | `hooks::run_pre_tool_hooks()` | tools/hooks.rs |
| `hasPermissionsToUseTool()` | `decision::has_permissions_to_use_tool()` | permissions/decision.rs |
| `compactConversation()` | `compaction::build_post_compact_messages()` | compact/compaction.rs |
| `gracefulShutdown()` | `shutdown::graceful_shutdown()` | shutdown.rs |

## Rust 目录结构设计 (完整目标)

```
rust/src/
├── main.rs                      ← entrypoints/cli.tsx + main.tsx
├── shutdown.rs                  ← 新增
│
├── analytics/                   ← services/analytics/
├── api/                         ← services/api/
├── auth/                        ← services/oauth/ + utils/secureStorage/
├── commands/                    ← commands/ (需大量补充)
├── compact/                     ← services/compact/
├── config/                      ← utils/settings/ + constants/
│   ├── claude_md.rs
│   ├── constants.rs             ← 待新增
│   └── settings.rs
├── engine/                      ← QueryEngine.ts + query.ts
├── mcp/                         ← services/mcp/
├── permissions/                 ← utils/permissions/
├── query/                       ← query/
├── remote/                      ← remote/
├── session/                     ← memdir/ + 会话相关
│   └── migrations.rs            ← 待新增
├── tasks/                       ← 待新增: tasks/
├── tools/                       ← tools/ (需补充 ~10 个)
├── types/                       ← types/ + state/ + schemas/
├── ui/                          ← components/ + ink/ + screens/
│   ├── keybindings.rs           ← 待新增
│   └── vim.rs                   ← 待新增
├── utils/                       ← utils/ (需大量补充)
│   ├── bash/                    ← 待新增
│   ├── git.rs                   ← 待新增
│   ├── shell.rs                 ← 待新增
│   └── ...
├── skills/                      ← 待新增: skills/
└── plugins/                     ← 待新增: plugins/
```

## 开发原则

1. **本地优先**: Phase 0-8 + 14A/B 不依赖网络，core 状态机可完全离线运行
2. **可测试**: 164 个测试覆盖所有核心路径，QueryDeps trait 允许完整 mock
3. **增量构建**: 每个 Phase 可独立编译和测试
4. **Feature gates**: 网络功能通过 Cargo features 按需启用
5. **Generator → Stream**: `async_stream::stream!` 宏实现 TypeScript 的 yield 语义
6. **扁平化模块**: Rust 版本将 TS 的深层嵌套 (services/X/) 提升为顶级模块
7. **合并相关**: 功能相近的 TS 目录 (如 Enter/ExitPlanMode) 在 Rust 中合并为单文件
