# 已完成模块 — 大幅简化实现

> 最后更新: 2026-04-02 (TUI 集成完成: ui/tui.rs 异步事件循环 + App 增强)
> 此文档记录已标记 ✅ 但相比 TypeScript 原版有显著功能缩减的模块。
> 完整实现见 [`COMPLETED_FULL.md`](COMPLETED_FULL.md)。
> 详细简化率分析见 [`MODULE_SIMPLIFICATION.md`](MODULE_SIMPLIFICATION.md)。
> 剩余工作见 [`REWRITE_PLAN.md`](../REWRITE_PLAN.md)。

---

## 简化分级

| 级别 | 缩减率 | 含义 |
|------|--------|------|
| **S** | >80% | 仅保留核心骨架，大量功能未移植 |
| **A** | 50-80% | 核心功能完整，高级功能缺失 |
| **B** | 30-50% | 较完整，部分边缘 case 省略 |

---

## 1. S 级简化 (>80% 缩减)

### 1.1 BashTool — 97% 缩减 (输出截断已补全)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 12,411 (18 文件) | 430 (1 文件) |
| 文件 | — | `tools/bash.rs` |

**Rust 保留：**
- 基础进程执行 (`tokio::process::Command`)
- timeout 支持
- stdout/stderr 输出捕获
- ✅ **输出截断策略** (head 200 行 + tail 100 行 + 中间省略, 行边界感知)

**TS 独有（未移植）：**
- PowerShell 分支 (8,959 行的 PowerShellTool)
- 沙箱执行环境 (sandbox/)
- 复杂超时逻辑 (分层 kill: SIGTERM → SIGKILL)
- 进程组管理
- 命令拒绝列表与危险命令分析
- heredoc 验证
- 终端大小感知
- Git 操作跟踪 (gitOperationTracking)

---

### 1.2 AgentTool — 87% 缩减 (worktree 隔离已补全)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 6,072 (14 文件) | 789 (1 文件) |
| 文件 | — | `tools/agent.rs` |

**Rust 保留：**
- 子 QueryEngine 派生
- 递归深度限制
- agent_id/agent_type 传递
- 基础输入验证 (prompt 必填)
- 模型覆盖 (model alias 解析)
- ✅ **worktree 隔离执行** (`isolation: "worktree"`, 创建临时 worktree → 运行 agent → 变更检测 → 自动清理/保留, fail-closed 安全)

**TS 独有（未移植）：**
- 多后端 spawn (in-process / tmux / iTerm2)
- 团队上下文与 agent teams 集成
- `spawnMultiAgent.ts` (1,093 行)
- 工具白名单过滤
- background 模式
- 工具定义去重

---

### 1.3 FileEditTool — 79% 缩减 (fuzzy 匹配已补全)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,812 (6 文件) | 386 (1 文件) |
| 文件 | — | `tools/file_edit.rs` |

**Rust 保留：**
- 精确字符串匹配替换
- `replace_all` 选项
- 路径验证
- ✅ **Fuzzy 匹配** (基于 `similar::TextDiff` 的滑动窗口相似度搜索, >60% 阈值建议最佳匹配, 显示行号范围和相似百分比)

**TS 独有（未移植）：**
- Diff 渲染 (用户预览)
- 冲突检测与解决
- 文件锁定检查
- 编辑历史追踪
- 缩进自动检测与修正

---

### 1.4 FileReadTool — 54% 缩减 (PDF/图片/ipynb 已补全)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,602 (5 文件) | 743 (1 文件) |
| 文件 | — | `tools/file_read.rs` |

**Rust 保留：**
- 文本文件读取 + 行号
- 二进制文件检测
- offset/limit 分页
- ✅ **图片文件** (.png/.jpg/.gif/.bmp/.webp → base64 编码, .svg → 文本读取)
- ✅ **PDF 文件** (pages 参数, 通过 `pdftotext` 子进程提取文本)
- ✅ **Jupyter notebook** (.ipynb JSON 解析, cell 类型/源码/输出提取)

**TS 独有（未移植）：**
- 符号链接解析
- 大文件智能分页策略
- 文件编码检测

---

### 1.5 GrepTool — 53% 缩减 (ripgrep + multiline 已补全)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 795 (3 文件) | 371 (1 文件) |
| 文件 | — | `tools/grep.rs` |

**Rust 保留：**
- 正则搜索 (regex crate 内置 + rg 子进程)
- 上下文行 (-A/-B/-C)
- output_mode: content/files_with_matches/count
- ✅ **ripgrep 子进程调用** (自动检测 `rg`, 构建完整参数, 失败时回退内置 regex)
- ✅ **多行匹配模式** (`multiline: true` → rg `-U --multiline-dotall`)
- ✅ **offset 分页** (跳过前 N 条结果)

**TS 独有（未移植）：**
- (核心功能已基本对齐)

---

### 1.6 FileWriteTool — 82% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 856 (3 文件) | 157 (1 文件) |
| 文件 | — | `tools/file_write.rs` |

**Rust 保留：**
- 路径验证 + 内容写入
- 父目录自动创建

**TS 独有（未移植）：**
- 安全文件写入 (先写临时 → rename)
- 文件备份/恢复
- 二进制内容检查
- 文件大小限制
- 权限保持

---

## 2. A 级简化 (50-80% 缩减)

### 2.1 SkillTool — 69% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,477 (4 文件) | 454 (1 文件) |
| 文件 | — | `tools/skill.rs` |

**Rust 保留：**
- 技能查找 (registry 查询)
- 参数替换 ($ARGUMENTS, ${NAME})
- new_messages 注入 (inline 上下文)
- fork 上下文 fallback

**TS 独有（未移植）：**
- MCP skill builder (mcpSkillBuilders.ts)
- 技能依赖解析
- 技能热重载
- 技能版本管理
- 复杂 frontmatter 验证

---

### 2.2 TaskTools — 58% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,561 (15 文件, 6 工具) | 648 (1 文件) |
| 文件 | — | `tools/tasks.rs` |

**Rust 保留：**
- TaskStore (内存 HashMap)
- 6 个操作: Create/Get/Update/List/Stop/Output

**TS 独有（未移植）：**
- 任务持久化 (磁盘存储)
- 任务进度 UI 渲染 (React 组件)
- 任务依赖图
- 后台任务管理
- 任务超时

---

### 2.3 ToolSearchTool — 57% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 593 (3 文件) | 254 (1 文件) |
| 文件 | — | `tools/tool_search.rs` |

**Rust 保留：**
- `select:` 精确选择模式
- 关键字模糊搜索
- 结果限制 (max_results)

**TS 独有（未移植）：**
- TF-IDF 排名算法
- 工具描述全文索引
- deferred tool schema 加载

---

### 2.4 PlanMode — 54% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 934 (8 文件) | 432 (1 文件) |
| 文件 | — | `tools/plan_mode.rs` |

**Rust 保留：**
- 完整状态转换 (save/restore pre_plan_mode)
- agent 上下文阻止
- 重复进入检测
- 用户确认退出

**TS 独有（未移植）：**
- auto-mode 集成 (classifier gate)
- 团队审批工作流
- 计划文件持久化
- 计划与实现关联跟踪

---

### 2.5 LSP 工具 + 服务 — 56% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 2,005 (6 文件) | 877 + 293 (2 文件) |
| 文件 | — | `tools/lsp.rs` + `lsp_service/mod.rs` |

**Rust 保留：**
- 9 种 LSP 操作
- 回退文本分析 (无 LSP 时)
- LSP 服务器配置/状态

**TS 独有（未移植）：**
- 真实 LSP 服务器生命周期管理
- 多语言 LSP 服务器自动检测
- 增量文档同步
- 补全建议

---

### 2.6 WebFetchTool — 51% 缩减

| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,131 (5 文件) | 553 (1 文件) |
| 文件 | — | `tools/web_fetch.rs` |

**Rust 保留：**
- HTTP GET 请求 (reqwest)
- HTML → text 转换
- URL 验证
- 响应截断
- 缓存

**TS 独有（未移植）：**
- JavaScript 渲染 (headless browser)
- Cookie 管理
- 代理支持
- 重定向限制
- Content-Type 智能处理

---

## 3. B 级简化 (30-50% 缩减)

### 3.1 终端 UI — 94% 缩减 (框架差异)

| | TypeScript | Rust |
|---|---|---|
| 行数 | 54,049 (components + ink + hooks) | 3,165 (ui/) |

> 注: 大部分缩减来自 React/Ink → ratatui 的框架差异，非功能遗漏。

**Rust 保留：**
- app.rs (409) — 主应用循环
- keybindings.rs (425) — 键绑定
- vim.rs (847) — Vim 模式
- messages.rs (404) — 消息渲染
- markdown.rs (259) — Markdown 渲染
- prompt_input.rs (250) — 输入框
- permissions.rs (244) — 权限对话框
- theme/diff/spinner

**TS 独有（框架差异，无法直接对比）：**
- React 组件树 (113 个组件)
- Ink 终端渲染引擎 (44 文件)
- React Hooks (83 个)
- 任务面板 UI
- MCP 审批对话框
- 团队管理面板
- 设置向导

---

### 3.2 认证 — 部分实现

| | TypeScript | Rust |
|---|---|---|
| 文件 | — | `auth/mod.rs` + `api_key.rs` + `token.rs` |

**Rust 保留 (活跃路径)：**
- API Key 验证/存储/加载 (`ANTHROPIC_API_KEY`)
- External Token (`ANTHROPIC_AUTH_TOKEN`)
- keyring 集成 (feature-gated)

**接口保留但未实现 (`#[allow(dead_code)]`, `bail!`)：**
- OAuth 登录流程
- Token 刷新
- Token 持久化

---

### 3.3 API 提供商 — 部分实现

**活跃实现：**
- Anthropic Direct (完整)

**接口保留但未实现 (`unimplemented!`)：**
- AWS Bedrock
- GCP Vertex AI

---

### 3.4 遥测/远程 — 仅接口

| 模块 | 文件 | 行数 | 状态 |
|---|---|---|---|
| 遥测 | `analytics/mod.rs` | 54 | 本地日志接口，无网络发送 |
| 远程会话 | `remote/session.rs` | 27 | 接口存根 (`bail!`) |
| LSP 服务 | `lsp_service/mod.rs` | 293 | 配置/状态框架，API 占位 |

---

## 4. 简化总结

### 按缩减率排序

| 模块 | TS 行数 | Rust 行数 | 缩减率 | 级别 |
|------|---------|-----------|--------|------|
| state (→ types) | ~58,000 | 832 | 99% | S |
| skills/ | ~43,000 | 989 | 98% | S |
| BashTool | 12,411 | 430 | 97% | S (截断已补全) |
| utils/ | 90,813 | 2,857 | 97% | S |
| AgentTool | 6,072 | 789 | 87% | S (worktree 已补全) |
| UI (全部) | 54,049 | 3,165 | 94% | S (框架) |
| permissions/ | 9,409 | 959 | 90% | S |
| FileEditTool | 1,812 | 386 | 79% | A (fuzzy 已补全) |
| FileReadTool | 1,602 | 743 | 54% | A (PDF/图片/ipynb 已补全) |
| FileWriteTool | 856 | 157 | 82% | S |
| GrepTool | 795 | 371 | 53% | A (rg+multiline 已补全) |
| SkillTool | 1,477 | 454 | 69% | A |
| TaskTools | 1,561 | 648 | 58% | A |
| ToolSearchTool | 593 | 254 | 57% | A |
| LSP | 2,005 | 1,170 | 42% | B |
| WebFetchTool | 1,131 | 553 | 51% | A |
| PlanMode | 934 | 432 | 54% | A |

### 主要简化原因分布

| 原因 | 涉及模块 | 影响 |
|------|---------|------|
| **框架差异** (React→无框架) | UI, state, hooks | ~110,000 行差异 |
| **平台功能未移植** | PowerShell, sandbox, native installer | ~14,000 行 |
| **网络功能降优先** | bridge, voice, telemetry, remote | ~20,000 行 |
| **高级特性省略** | JS render, OAuth (fuzzy match 已补全) | ~10,000 行 |
| **边缘 case 精简** | 各工具的错误恢复/重试/UI | 分散 |

### 优先补全建议

~~以下简化项已补全：~~ (2026-04-01)

1. ~~**FileEditTool fuzzy 匹配**~~ ✅ 已补全 — 230→386 行, `similar::TextDiff` 滑动窗口
2. ~~**BashTool 输出截断**~~ ✅ 已补全 — 199→430 行, head+tail 行级截断
3. ~~**FileReadTool PDF/图片**~~ ✅ 已补全 — 236→743 行, 图片 base64 + PDF pdftotext + ipynb JSON
4. ~~**GrepTool ripgrep 调用**~~ ✅ 已补全 — 185→371 行, rg 子进程 + multiline + offset
5. ~~**AgentTool worktree 隔离**~~ ✅ 已补全 — 322→789 行, 临时 worktree + 变更检测 + 自动清理

**剩余补全建议 (低优先级)：**

1. **FileWriteTool** — 安全写入 (先写临时 → rename)、备份
2. **ToolSearchTool** — TF-IDF 排名、全文索引
3. **API 提供商** — Bedrock/Vertex 填充
4. **认证** — OAuth 登录流程
