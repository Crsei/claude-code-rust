# 已完成模块 — 完整实现

> 最后更新: 2026-04-02
> 此文档记录与 TypeScript 原版功能对等或接近完整的已完成模块。
> 大幅简化的模块见 [`COMPLETED_SIMPLIFIED.md`](COMPLETED_SIMPLIFIED.md)。
> 剩余工作见 [`REWRITE_PLAN.md`](../REWRITE_PLAN.md)。

---

## Phase 0: 类型基础

完整定义了所有核心类型，与 TS 功能对等。

| # | 模块 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P0.1 | Message 枚举 | `types/message.rs` | 283 | ContentBlock, Usage, 7 种消息类型, QueryYield |
| P0.2 | Tool trait | `types/tool.rs` | 204 | Tool trait 18 个方法, ToolUseContext, FileStateCache |
| P0.3 | 循环状态 | `types/state.rs` | 114 | QueryLoopState, AutoCompactTracking, BudgetTracker |
| P0.4 | 查询配置 | `types/config.rs` | 121 | QueryParams, QueryEngineConfig, ThinkingConfig |
| P0.5 | 应用状态 | `types/app_state.rs` | 57 | AppState, SettingsJson |
| P0.6 | 状态转换 | `types/transitions.rs` | 47 | Terminal (10 种), Continue (7 种) |

**小计: 6 个文件, 826 行**

---

## Phase 1: 状态机骨架

核心查询循环与引擎生命周期，Rust 实现比 TS 更详细。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P1.1 | query loop | `query/loop_impl.rs` | 1,024 | 4 | 完整 8 步循环 + 恢复路径 |
| P1.2 | 依赖注入 | `query/deps.rs` | 146 | 0 | QueryDeps trait (可 mock) |
| P1.3 | token 预算 | `query/token_budget.rs` | 64 | 0 | checkTokenBudget + diminishing returns |
| P1.4 | stop hooks | `query/stop_hooks.rs` | 163 | 4 | has_tool_use, extract_tool_uses |
| P1.5 | QueryEngine | `engine/lifecycle.rs` | 1,393 | 8 | Phase A-E 完整分发 + QueryEngineDeps |
| P1.6 | SDK 类型 | `engine/sdk_types.rs` | 142 | 0 | SdkMessage 7 种变体 |
| P1.7 | 输入处理 | `engine/input_processing.rs` | 155 | 5 | 斜杠命令解析 + UserMessage 构建 |
| P1.8 | 系统提示 | `engine/system_prompt.rs` | 640 | 7 | 7 静态段落 + 动态段落 + 缓存边界 |
| P1.9 | 结果判定 | `engine/result.rs` | 233 | 6 | isResultSuccessful + extractTextResult |
| P1.10 | 提示段落 | `engine/prompt_sections.rs` | 210 | — | 段落组装 + 工具 prompt |
| P1.11 | CLI 入口 | `main.rs` | ~400 | 0 | clap CLI + 快速路径 + TUI 集成 + print mode |
| P1.12 | 关闭清理 | `shutdown.rs` | 129 | 0 | SIGINT handler + abort + transcript flush |

**小计: 12 个文件, ~4,700 行, 34 测试**

---

## Phase 2: 工具基础设施

工具注册、执行管线和 Hook 系统完整实现。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P2.1 | 工具注册 | `tools/registry.rs` | 115 | 4 | get_all_tools + find_tool_by_name, 18 工具 |
| P2.2 | 并发编排 | `tools/orchestration.rs` | 535 | — | partitionToolCalls + 并行/串行批次 |
| P2.3 | 执行管线 | `tools/execution.rs` | 608 | — | run_tool_use() 8 步管线 |
| P2.4 | Hook 系统 | `tools/hooks.rs` | 855 | — | 完整子进程执行 + JSON 解析 + matcher |

**小计: 4 个文件, 2,113 行**

---

## Phase 3: 权限与配置

权限决策与设置加载核心逻辑完整。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P3.1 | 规则引擎 | `permissions/rules.rs` | 274 | 7 | deny→allow→ask 优先级 + glob 匹配 |
| P3.2 | 决策状态机 | `permissions/decision.rs` | 458 | 7 | 模式匹配 + denial tracker |
| P3.3 | 危险检测 | `permissions/dangerous.rs` | 217 | 11 | 16 种危险模式正则 |
| P3.4 | 设置加载 | `config/settings.rs` | 294 | 3 | 3 层合并 (global → project → env) |
| P3.5 | CLAUDE.md | `config/claude_md.rs` | 129 | 0 | 文件发现 + 上下文注入 |

**小计: 5 个文件, 1,372 行, 28 测试**

---

## Phase 4: 上下文管理 (压缩管线)

完整实现 snip → micro → autocompact 管线，与 TS 功能对等。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P4.1 | 消息工具 | `compact/messages.rs` | 306 | 4 | normalizeForAPI + 交替模式 |
| P4.2 | 微压缩 | `compact/microcompact.rs` | 261 | 2 | 阈值裁剪 + 最近 N 结果保护 |
| P4.3 | 历史裁剪 | `compact/snip.rs` | 217 | 2 | turn 识别 + 边界消息 |
| P4.4 | 结果预算 | `compact/tool_result_budget.rs` | 224 | 2 | 磁盘持久化 + async I/O + 预览生成 |
| P4.5 | 管线编排 | `compact/pipeline.rs` | 300 | 3 | snip → micro → autocompact 编排 |
| P4.6 | 压缩决策 | `compact/auto_compact.rs` | 52 | 3 | 80% 阈值判定 |
| P4.7 | 全量压缩 | `compact/compaction.rs` | 426 | 8 | 决策 + 跟踪 + prompt + boundary |
| P4.8 | token 估算 | `utils/tokens.rs` | 170 | 5 | 4 chars/token 启发式 |
| P4.9 | 文件缓存 | `utils/file_state_cache.rs` | 193 | 0 | LRU 缓存 + hash/timestamp |

**小计: 9 个文件, 2,149 行, 29 测试**

---

## Phase 6: 会话持久化

| # | 模块 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P6.1 | 会话存储 | `session/storage.rs` | 328 | JSON 持久化 + NDJSON 序列化 |
| P6.2 | 对话记录 | `session/transcript.rs` | 187 | NDJSON append + sync |
| P6.3 | 会话恢复 | `session/resume.rs` | 53 | cwd 匹配 + 消息加载 |

**小计: 3 个文件, 568 行**

---

## Phase 8: 高级本地工具

PlanMode、Worktree、Skill 完整实现真实逻辑。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P8.1 | PlanMode | `tools/plan_mode.rs` | 432 | 8 | 真实状态转换: save/restore pre_plan_mode, 验证 agent 上下文 |
| P8.2 | Worktree | `tools/worktree.rs` | 724 | 8 | 真实 git worktree: 创建/清理, 变更检测, 失败关闭安全 |
| P8.3 | Skill | `tools/skill.rs` | 454 | 7 | 技能查找/扩展/注入: 注册表查询, 参数替换, new_messages |

**小计: 3 个文件, 1,610 行, 23 测试**

---

## Phase 9: API 客户端 (活跃提供商)

| # | 模块 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P9.1 | 客户端 | `api/client.rs` | 871 | SSE 字节流解析 + 重试 |
| P9.2 | 流解析 | `api/streaming.rs` | 112 | SSE 解析 + StreamAccumulator |
| P9.3 | 重试 | `api/retry.rs` | 99 | 错误分类 + 指数退避 |
| P9.4 | Anthropic 提供商 | `api/providers.rs` | 311 | AnthropicProvider 完整实现 |

**小计: 4 个文件, 1,393 行**

> 注: Bedrock/Vertex 仅保留函数签名 (`unimplemented!`)，见简化文档。

---

## Phase 11: MCP 协议

| # | 模块 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P11.1 | 客户端 | `mcp/client.rs` | 1,008 | stdio 传输 + JSON-RPC 2.0 + McpManager |
| P11.2 | 发现 | `mcp/discovery.rs` | 47 | JSON 配置加载 |
| P11.3 | 工具 | `mcp/tools.rs` | 296 | McpToolWrapper 委托真实 McpClient |

**小计: 3 个文件, 1,351 行**

---

## Phase 12: 网络工具 (接近完整)

| # | 模块 | 文件 | Rust 行数 | TS 行数 | 缩减 | 说明 |
|---|------|------|-----------|---------|------|------|
| P12.1 | WebFetch | `tools/web_fetch.rs` | 553 | 1,131 | 51% | HTML→text, 缓存, URL 验证, 截断 |
| P12.2 | WebSearch | `tools/web_search.rs` | 529 | 569 | 7% | Brave Search API, 域名过滤 |
| P12.3 | LSP 工具 | `tools/lsp.rs` | 877 | 2,005 | 56% | 9 种操作, 回退文本分析 |
| P12.4 | NotebookEdit | `tools/notebook_edit.rs` | 530 | 587 | 10% | replace/insert/delete 完整实现 |
| P12.5 | 技能系统 | `skills/` (3 文件) | 989 | — | — | 加载/注册/5个内置技能 |
| P12.6 | 插件系统 | `plugins/` (3 文件) | 931 | — | — | manifest/loader/注册 |

**小计: 8 个文件, 4,409 行**

---

## Phase 13: 终端 UI (完整 TUI)

基于 ratatui + crossterm 的全屏终端界面，已与 QueryEngine 完整集成。

| # | 模块 | 文件 | 行数 | 测试 | 说明 |
|---|------|------|------|------|------|
| P13.1 | TUI 集成 | `ui/tui.rs` | ~300 | 0 | 异步事件循环, tokio::spawn 引擎查询, mpsc 通道通信, 终端状态守卫 |
| P13.2 | App 主框架 | `ui/app.rs` | ~470 | 0 | 消息历史, 输入状态, 权限对话框, 滚动, 输入历史, 状态栏, 费用跟踪 |
| P13.3 | 消息渲染 | `ui/messages.rs` | 404 | 0 | User/Assistant/System/Progress/Attachment 5 种消息渲染 |
| P13.4 | Markdown 渲染 | `ui/markdown.rs` | 259 | 0 | pulldown-cmark: 标题, 粗体, 斜体, 代码块, 列表, 链接, 引用, 分隔线 |
| P13.5 | 输入框 | `ui/prompt_input.rs` | 250 | 0 | 光标, 水平滚动, UTF-8 安全, Ctrl 快捷键 |
| P13.6 | 权限对话框 | `ui/permissions.rs` | 244 | 0 | 居中覆盖层, Allow/Deny/AlwaysAllow, 快捷键 Y/N/A |
| P13.7 | 主题系统 | `ui/theme.rs` | 116 | 0 | 23 种预定义 RGB 样式 |
| P13.8 | 旋转动画 | `ui/spinner.rs` | 95 | 0 | 10 帧 Braille 点阵动画 |
| P13.9 | Diff 渲染 | `ui/diff.rs` | 96 | 0 | 增/删/上下文着色, similar crate |

**小计: 9 个文件, ~2,234 行**

---

## Phase 14A: 本地补充模块

| # | 模块 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P14A.1 | 常量定义 | `config/constants.rs` | 454 | 模型 ID, API 版本, token 限制 |
| P14A.2 | utils/bash | `utils/bash.rs` | 704 | 命令解析, shell 转义, heredoc 检测 |
| P14A.3 | utils/git | `utils/git.rs` | 687 | git2: status/diff/log/branch/shallow |
| P14A.4 | utils/shell | `utils/shell.rs` | 323 | shell 检测, 环境初始化, Git Bash |
| P14A.5 | utils/messages | `utils/messages.rs` | 497 | 消息格式化, 截断, 摘要, 统计 |
| P14A.6 | keybindings | `ui/keybindings.rs` | 425 | 快捷键注册, 上下文解析, 自定义绑定 |
| P14A.7 | vim 模式 | `ui/vim.rs` | 847 | Normal/Insert/Visual, hjkl, dd/yy/p/w/b |
| P14A.8 | 迁移系统 | `session/migrations.rs` | 300 | 版本检测, v1→v2→v3 迁移链 |
| P14A.9 | 任务子系统 | `tools/tasks.rs` | 648 | TaskStore + 6 工具 (Create/Get/Update/List/Stop/Output) |
| P14A.10 | 内存系统 | `session/memdir.rs` | 385 | CRUD + 搜索 + 上下文注入 |

**小计: 10 个文件, 5,270 行**

---

## Phase 14B: 命令系统 (第一、二批)

**第一批 — 高频核心命令**

| # | 命令 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P14B.1 | /exit | `commands/exit.rs` | 63 | 退出 REPL |
| P14B.2 | /version | `commands/version.rs` | 48 | 版本号 |
| P14B.3 | /model | `commands/model.rs` | 132 | 切换模型 |
| P14B.4 | /cost | `commands/cost.rs` | 195 | token 用量 |
| P14B.5 | /session | `commands/session.rs` | 151 | 会话列表/切换 |
| P14B.6 | /resume | `commands/resume.rs` | 146 | 恢复会话 |
| P14B.7 | /files | `commands/files.rs` | 175 | 引用文件列表 |
| P14B.8 | /context | `commands/context.rs` | 198 | 上下文管理 |
| P14B.9 | /permissions | `commands/permissions_cmd.rs` | 300 | 权限查看/修改 |
| P14B.10 | /hooks | `commands/hooks_cmd.rs` | 184 | hook 管理 |

**第二批 — 中频功能命令**

| # | 命令 | 文件 | 行数 | 说明 |
|---|------|------|------|------|
| P14B.11 | /commit | `commands/commit.rs` | 95 | git commit + 模型辅助 |
| P14B.12 | /review | `commands/review.rs` | 81 | 代码审查 |
| P14B.13 | /branch | `commands/branch.rs` | 97 | 分支管理 |
| P14B.14 | /export | `commands/export.rs` | 154 | JSON/Markdown 导出 |
| P14B.15 | /rename | `commands/rename.rs` | 41 | 重命名会话 |
| P14B.16 | /stats | `commands/stats.rs` | 85 | 消息/token 统计 |
| P14B.17 | /effort | `commands/effort.rs` | 51 | 思考力度设置 |
| P14B.18 | /fast | `commands/fast.rs` | 256 | 快速模式 + 状态 |
| P14B.19 | /memory | `commands/memory.rs` | 85 | CLAUDE.md 管理 |
| P14B.20 | /plan | `commands/plan.rs` | 88 | 计划模式切换 |
| — | 注册表 | `commands/mod.rs` | 379 | 27 命令注册 + 别名 + 参数 |

**小计: 21 个文件, 2,905 行**

---

## 完整实现统计

| 类别 | 文件数 | 行数 | 测试数 |
|------|--------|------|--------|
| 类型基础 (Phase 0) | 6 | 826 | — |
| 状态机骨架 (Phase 1) | 12 | 4,772 | 34 |
| 工具基础设施 (Phase 2 基础) | 4 | 2,113 | 4 |
| 权限与配置 (Phase 3) | 5 | 1,372 | 28 |
| 上下文管理 (Phase 4) | 9 | 2,149 | 29 |
| 会话持久化 (Phase 6) | 3 | 568 | — |
| 高级工具 (Phase 8) | 3 | 1,610 | 23 |
| API 客户端 (Phase 9) | 4 | 1,393 | — |
| MCP 协议 (Phase 11) | 3 | 1,351 | — |
| 网络工具 + 技能/插件 (Phase 12) | 8 | 4,409 | — |
| 本地补充 (Phase 14A) | 10 | 5,270 | — |
| 命令系统 (Phase 14B) | 21 | 2,905 | — |
| **合计** | **88** | **~28,738** | **118+** |
