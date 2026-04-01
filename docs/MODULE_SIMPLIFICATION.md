# Rust 重写模块简化分析

> 生成时间: 2026-04-01
> TypeScript 源码: `cc/src/` (~800+ 文件, ~225,000 行)
> Rust 源码: `cc/rust/src/` (151 文件, ~41,278 行, 占 TS 的 ~18%)

本文档对比 TypeScript 原始实现与 Rust 重写的各模块，分析简化程度、保留的核心逻辑和省略的功能。

---

## 1. 总体对比

| 维度 | TypeScript | Rust | 比例 |
|---|---|---|---|
| 总行数 | ~225,000 | ~41,278 | 18% |
| 文件数 | ~800+ | 151 | ~19% |
| 工具数 | 40+ | 21 | 53% |
| 顶级目录 | 36 | 22 | 61% |

简化主要来自三方面：
1. **砍掉非核心功能** — bridge、voice、computerUse、chrome 集成、native installer 等网络/平台特性完全未移植
2. **框架差异** — React/Ink UI → ratatui 减少大量 boilerplate；React hooks/state → 普通 Rust struct
3. **工具实现精简** — 大多数工具只保留核心调用逻辑，省略复杂的错误恢复、边缘 case 处理、telemetry、permission UI 等

---

## 2. 模块级简化详情

### 2.1 极大简化 (>80% 缩减)

#### utils/ — 97% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | 90,813 | 2,857 |
| 文件数 | 299 | 9 |

**Rust 保留：**
- `bash.rs` (704) — 命令构建与执行
- `git.rs` (687) — git 操作工具函数
- `messages.rs` (497) — 消息处理
- `shell.rs` (323) — shell 环境检测
- `file_state_cache.rs` (193) — 文件状态缓存
- `abort.rs` (173) — 中止信号
- `tokens.rs` (170) — token 计数
- `cwd.rs` (102) — 工作目录管理

**TS 独有（未移植）：**
| 子模块 | 行数 | 说明 |
|---|---|---|
| plugins/ | 20,521 | 插件系统管理（Rust 有独立 plugins/ 模块 931 行） |
| bash/ | 12,093 | Bash 辅助工具（Rust 精简为 704 行） |
| permissions/ | 9,409 | 权限检查（Rust 有独立 permissions/ 模块 959 行） |
| swarm/ | 4,486 | 多代理 swarm 系统 |
| telemetry/ | 4,044 | 遥测/分析 |
| settings/ | 4,035 | 设置管理（Rust 在 config/ 中） |
| hooks/ | 3,721 | Hook 工具 |
| shell/ | 3,069 | Shell 工具 |
| nativeInstaller/ | 3,018 | 原生安装器 |
| model/ | 2,710 | 模型相关 |
| claudeInChrome/ | 2,356 | Chrome 集成 |
| powershell/ | 2,305 | PowerShell 工具 |
| computerUse/ | 2,161 | 计算机控制自动化 |
| processUserInput/ | 1,765 | 用户输入处理 |
| deepLink/ | 1,388 | 深度链接 |
| task/ | 1,223 | 任务工具 |
| suggestions/ | 1,213 | 建议系统 |
| git/ | 1,075 | Git 工具（Rust 精简为 687 行） |
| sandbox/ | 997 | 沙箱 |
| teleport/ | 955 | 传送功能 |
| secureStorage/ | 629 | 安全存储 |
| ultraplan/ | 476 | 规划系统 |
| filePersistence/ | 413 | 文件持久化 |

---

#### UI (components + ink + hooks) — 94% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | 54,049 | 3,165 |
| 文件数 | 240 | 11 |

**TypeScript 使用 React/Ink 渲染终端 UI：**
- `components/` — 113 文件, 24,266 行 (PromptInput 5,161 行、messages 5,509 行、tasks 3,938 行、MCP UI 3,872 行等)
- `ink/` — 44 文件, 13,307 行 (Ink 终端 UI 库封装)
- `hooks/` — 83 文件, 16,476 行 (React hooks)

**Rust 使用 ratatui：**
- `app.rs` (409) — 主应用循环
- `keybindings.rs` (425) — 键绑定
- `vim.rs` (847) — Vim 模式
- `messages.rs` (404) — 消息渲染
- `markdown.rs` (259) — Markdown 渲染
- `prompt_input.rs` (250) — 输入框
- `permissions.rs` (244) — 权限对话框
- `theme.rs` (116) — 主题
- `diff.rs` (96) — Diff 显示
- `spinner.rs` (95) — 加载动画

---

#### state/ — 99% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~58,000 | 832 |
| 文件数 | 6 | 7 |

TS 使用 React 不可变状态管理 (`DeepImmutable<AppState>`)，包含大量类型和变更订阅逻辑。Rust 简化为普通 struct + `Arc<RwLock<AppState>>`，通过闭包 (`get_app_state` / `set_app_state`) 传递。

---

#### permissions/ — 90% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | 9,409 | 959 |
| 文件数 | 24 | 4 |

**Rust 保留：**
- `decision.rs` (458) — 权限决策核心逻辑
- `rules.rs` (274) — 规则定义
- `dangerous.rs` (217) — 危险命令检测

**省略：** 细粒度 per-tool UI、复杂的权限模式切换、规则持久化等。

---

#### skills/ — 98% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~43,000 | 989 |
| 文件数 | 3 | 3 |

TS 的 `loadSkillsDir.ts` 单文件 34K 行，包含复杂的目录扫描、frontmatter 解析、MCP skill builder 等。Rust 保留了核心的 registry + loader + bundled 骨架。

---

### 2.2 大幅简化 (50-80% 缩减)

#### tools/ — 80% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~45,000+ | 8,851 |
| 文件数 | 40+ 工具目录 | 22 文件 |
| 工具数 | 40+ | 18 |

**单工具对比（简化最显著的）：**

| 工具 | TS 行数 | Rust 行数 | 缩减率 | 简化说明 |
|---|---|---|---|---|
| BashTool | 12,411 (18 文件) | 430 | 97% | ✅ 输出截断; 省略 PowerShell/沙箱/进程管理 |
| AgentTool | 6,072 (14 文件) | 789 | 87% | ✅ worktree 隔离; 省略多后端 spawn、团队上下文 |
| FileEditTool | 1,812 (6 文件) | 386 | 79% | ✅ fuzzy 匹配; 省略 diff 渲染、冲突检测 |
| FileReadTool | 1,602 (5 文件) | 743 | 54% | ✅ PDF/图片/ipynb; 省略编码检测 |
| GrepTool | 795 (3 文件) | 371 | 53% | ✅ rg 子进程 + multiline + offset |
| FileWriteTool | 856 (3 文件) | 157 | 82% | 省略安全检查、备份逻辑 |
| ToolSearchTool | 593 (3 文件) | 254 | 57% | 省略模糊搜索算法 |
| SkillTool | 1,477 (4 文件) | 454 | 69% | 保留核心 lookup→expand→inject |
| PlanMode | 934 (8 文件) | 432 | 54% | 省略 auto-mode 集成、classifier gate |
| TaskTools | 1,561 (15 文件) | 648 | 58% | 合并 Create/Get/List/Update/Stop/Output 为一文件 |
| AskUserQuestion | 309 (2 文件) | 174 | 44% | 基本完整 |

**接近完整实现的工具：**

| 工具 | TS 行数 | Rust 行数 | 缩减率 |
|---|---|---|---|
| WebSearchTool | 569 | 529 | 7% |
| NotebookEditTool | 587 | 530 | 10% |
| WebFetchTool | 1,131 | 553 | 51% |
| LSPTool | 2,005 | 877 | 56% |
| Worktree | 563 | 724 | +29% (Rust 更详细) |

---

#### api/services — 54% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | 4,906+ | 2,254 |
| 文件数 | 16 | 7 |

**Rust 保留：**
- `client.rs` (871) — 核心 API 客户端
- `google_provider.rs` (443) — Google Vertex AI
- `openai_compat.rs` (409) — OpenAI 兼容层
- `providers.rs` (311) — 多 provider 抽象
- `streaming.rs` (112) — 流式响应
- `retry.rs` (99) — 重试逻辑

**TS 独有：** voice (560)、voiceStreamSTT、tokenEstimation、claudeAiLimits、diagnosticTracking、rateLimitMessages、VCR、preventSleep 等。

---

### 2.3 适度简化 (<50% 缩减)

#### commands/ — 30% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | 5,586 | 3,894 |
| 文件数 | 15 | 28 |

Rust 拆分粒度更细（28 个文件 vs 15 个），每个命令实现更精简。TS 有 `insights.ts` (114K)、`ultraplan.tsx` (66K) 等超大命令，Rust 未移植。

---

#### compact/ — 40% 缩减
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~3,000+ | 1,793 |
| 文件数 | — | 8 |

保留完整的 compaction pipeline：自动压缩检测、消息处理、microcompact、tool result 预算、snip 逻辑。

---

#### mcp/ — 较完整
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~2,000+ | 1,767 |
| 文件数 | — | 4 |

`client.rs` (1,008 行) 为核心 MCP 客户端，加上 tools、discovery 模块。

---

#### session/ — 较完整
| | TypeScript | Rust |
|---|---|---|
| 行数 | 1,736 | 1,266 |
| 文件数 | 8 | 6 |

保留 memdir、storage、migrations、transcript、resume。

---

#### engine/ + query/ — Rust 反而更大
| | TypeScript | Rust |
|---|---|---|
| 行数 | ~3,000+ | 4,194 |
| 文件数 | — | 12 |

核心状态机和生命周期管理在 Rust 中实现更详细：
- `lifecycle.rs` (1,393) — 完整生命周期
- `system_prompt.rs` (640) — 系统提示词构建
- `loop_impl.rs` (1,024) — 查询主循环

---

## 3. 完全未移植的 TS 模块

| 模块 | TS 行数 | 功能说明 |
|---|---|---|
| **bridge/** | 12,613 (31 文件) | 远程控制/桥接模式 |
| **coordinator/** | 369 + swarm 4,486 | 多代理协调（仅有 AGENT_TEAMS_SPEC.md） |
| **PowerShellTool** | 8,959 (14 文件) | Windows PowerShell 执行 |
| **BriefTool** | 610 (5 文件) | 摘要生成 |
| **ConfigTool** | 809 (5 文件) | 运行时配置修改 |
| **TodoWriteTool** | 300 (3 文件) | Todo 列表管理 |
| **ScheduleCronTool** | 543 (5 文件) | Cron 定时任务 |
| **MCPTool** (tool) | 1,086 (4 文件) | MCP 工具调用（MCP 客户端已实现） |
| **SendMessageTool** | 997 (4 文件) | 代理间消息发送 |
| **RemoteTriggerTool** | 192 (3 文件) | 远程触发 |
| **McpAuthTool** | 215 (1 文件) | MCP 认证 |
| **REPLTool** | 85 (2 文件) | REPL 交互 |
| **SleepTool** | 17 (1 文件) | 延时 |
| **voice/** | 大量 | 语音交互 |
| **computerUse/** | 2,161 | 计算机控制 |
| **claudeInChrome/** | 2,356 | Chrome 浏览器集成 |
| **nativeInstaller/** | 3,018 | 原生安装器 |
| **buddy/** | 1,298 | Buddy 子系统 |
| **screens/** | 5,977 | 屏幕组件 |
| **keybindings/** (TS) | 3,159 | 键绑定配置（Rust 在 ui/keybindings.rs 425 行） |

---

## 4. 简化模式总结

### 4.1 结构性简化
- **React → 无框架**：TS 的 hooks (16,476 行)、context (1,004 行)、components (24,266 行) 在 Rust 中不存在对等概念
- **Ink → ratatui**：终端 UI 从 React 声明式渲染简化为命令式绘制
- **Immutable state → RwLock**：`DeepImmutable<AppState>` 变为 `Arc<RwLock<AppState>>`

### 4.2 功能性简化
- **多平台分支**：PowerShell、沙箱、原生安装器等平台特定代码未移植
- **网络功能**：bridge、remote、voice、telemetry 等联网功能降低优先级
- **UI 丰富度**：复杂的权限对话框、任务面板、MCP 审批 UI 简化为基础交互
- **边缘 case**：fuzzy 匹配、冲突自动解决、rate limit 精细处理等省略

### 4.3 保持完整的核心
- **查询主循环** (engine/ + query/) — 反而比 TS 更详细
- **压缩管线** (compact/) — 完整实现
- **MCP 客户端** (mcp/) — 接近完整
- **会话管理** (session/) — 接近完整
- **权限决策** (permissions/) — 核心保留
- **工具执行管线** (tools/execution.rs + orchestration.rs + hooks.rs) — 完整
