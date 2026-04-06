# master-feature → rust-lite 移植计划

> 本文档基于 `master-feature` (5326c57) 与 `rust-lite` (44a5c6e) 的完整对比分析，指导将 master-feature 的高价值模块按优先级移植到 rust-lite。
>
> **最后更新**: 2026-04-07 | **当前进度**: Phase 5 已完成 | **功能覆盖**: ~75%

## 目录

1. [背景与目标](#1-背景与目标)
2. [分支关系与现状](#2-分支关系与现状)
3. [规模总览](#3-规模总览)
4. [移植优先级矩阵](#4-移植优先级矩阵)
5. [Phase 1: 上下文压缩管道 (compact/)](#5-phase-1-上下文压缩管道)
6. [Phase 2: Agent 工具](#6-phase-2-agent-工具)
7. [Phase 3: Web 工具 (WebFetch / WebSearch)](#7-phase-3-web-工具)
8. [Phase 4: Plan Mode 与 Task 工具](#8-phase-4-plan-mode-与-task-工具)
9. [Phase 5: Worktree 工具](#9-phase-5-worktree-工具)
10. [Phase 6: MCP 协议支持](#10-phase-6-mcp-协议支持)
11. [Phase 7: LSP 集成](#11-phase-7-lsp-集成)
12. [Phase 8: 多 Agent Teams](#12-phase-8-多-agent-teams)
13. [Phase 9: 插件系统](#13-phase-9-插件系统)
14. [Phase 10: 云服务集成 (AWS/GCP)](#14-phase-10-云服务集成)
15. [Phase 11: 剩余命令补全](#15-phase-11-剩余命令补全)
16. [依赖管理](#16-依赖管理)
17. [保留 rust-lite 独有功能](#17-保留-rust-lite-独有功能)
18. [验证清单](#18-验证清单)
19. [附录：完整差异表](#19-附录完整差异表)

---

## 1. 背景与目标

### 两条路线的由来

- **master-feature**：完整功能版，全面对标 TypeScript 原版 Claude Code。218 文件、49K 行代码、28 工具、47+ 命令。包含 MCP、插件、Teams、上下文压缩、AWS/GCP 集成等完整生态。
- **rust-lite**：精简版，从 master-feature 剥离非核心模块后重建。132 文件、33.8K 行代码、15 工具、24 命令。增加了 bootstrap 全局状态、E2E 测试、.env 加载、欢迎界面等实用改进。

### 移植目标

将 master-feature 的高价值功能**逐阶段移植**到 rust-lite，同时保留 rust-lite 的架构改进（bootstrap、测试、精简设计）。最终目标是在 rust-lite 的精简基础上实现 ~85% 的 master-feature 功能覆盖。

### 原则

1. **渐进式移植**：每个 Phase 独立可编译、可测试
2. **保留 rust-lite 基础**：bootstrap、.env、测试、欢迎界面不变
3. **按价值排序**：先移植对用户体验影响最大的模块
4. **避免膨胀**：云服务等重量级依赖放在最后，可选移植

---

## 2. 分支关系与现状

```
master-feature (5326c57) ──── 冻结，无新 commit
       │
       │  "Strip to minimal viable version"
       │
       └──── 18 commits ────→ rust-lite (44a5c6e)  活跃开发
                                    │
                                    └──→ rust-lite-migrate-master (0e7d001)  移植分支
                                          Phase 1: compact ✅
```

- **共同祖先**：`5326c57` (master-feature HEAD)
- **rust-lite 独有**：18 个 commit（精简 + 重建 + 新功能）
- **master-feature 独有**：0 个 commit（已冻结）
- **rust-lite-migrate-master**：基于 rust-lite，逐步移植 master-feature 功能

---

## 3. 规模总览

| 指标 | master-feature | rust-lite (初始) | rust-lite-migrate (当前) | 差距 |
|------|---------------|-----------------|------------------------|------|
| 源文件数 | 218 `.rs` | 132 `.rs` | 147 `.rs` (+15) | -71 |
| 代码行数 | ~49,187 | ~33,800 | ~40,094 (+6,294) | -9,093 |
| 工具数 | 28 | 15 | 28 (+13) | 0 |
| 命令数 | 47+ | 26 | 27 (+1 /compact) | -20 |
| 依赖数 | 48 crate | 40 crate | 40 crate | -8 |
| 模块目录数 | 21 | 16 | 17 (+compact) | -4 |

---

## 4. 移植优先级矩阵

| Phase | 模块/功能 | 价值 | 复杂度 | 新依赖 | 预估行数 | 状态 |
|-------|-----------|------|--------|--------|----------|------|
| **1** | compact/ (上下文压缩) | ★★★★★ | 中 | 无 | ~1,561 | ✅ 完成 (`0e7d001`) |
| **2** | Agent 工具 | ★★★★★ | 中 | 无 | ~789 | ✅ 完成 |
| **3** | WebFetch + WebSearch | ★★★★☆ | 低 | 无 | ~1,053 | ✅ 完成 |
| **4** | PlanMode + Tasks | ★★★★☆ | 中 | 无 | ~1,082 | ✅ 完成 |
| **5** | Worktree 工具 | ★★★☆☆ | 中 | 无 | ~725 | ✅ 完成 |
| **6** | MCP 协议 | ★★★☆☆ | 高 | tokio-tungstenite, eventsource-stream | ~1,767 | ⬚ 待开始 |
| **7** | LSP 集成 | ★★★☆☆ | 高 | lsp-types | ~969 | ⬚ 待开始 |
| **8** | Agent Teams | ★★☆☆☆ | 高 | 无 | ~2,014 | ⬚ 待开始 |
| **9** | 插件系统 | ★★☆☆☆ | 中 | 无 | ~931 | ⬚ 待开始 |
| **10** | AWS/GCP 集成 | ★☆☆☆☆ | 高 | aws-sdk, gcp_auth, oauth2, jsonwebtoken | ~接口层 | ⬚ 待开始 |
| **11** | 剩余命令 (~25) | ★★★☆☆ | 低 | 无 | ~2,000+ | ⬚ 待开始 |

**总计新增代码**：~12,891 行（不含命令），移植后预计 ~46,000 行

---

## 5. Phase 1: 上下文压缩管道 ✅ 完成

> **Commit**: `0e7d001` | **日期**: 2026-04-07 | **新增**: +2,918 行, 14 文件

### 实际移植结果

| 文件 | 行数 | 功能 |
|------|------|------|
| `src/compact/mod.rs` | 7 | 模块入口 |
| `src/compact/compaction.rs` | 427 | 完整压缩逻辑：摘要生成、文件恢复、边界标记、跟踪状态、断路器 |
| `src/compact/auto_compact.rs` | 53 | 自动触发：80% 上下文窗口阈值检测 |
| `src/compact/microcompact.rs` | 262 | 轻量级过滤：旧大型工具结果裁剪（保留最近 10 个，>1K 字符替换为摘要） |
| `src/compact/snip.rs` | 218 | 历史裁剪：超出 200 轮时保留首条 + 最近 N 轮 |
| `src/compact/pipeline.rs` | 301 | 5 步管道编排 (budget → snip → microcompact → collapse → auto) + 响应式紧急压缩 |
| `src/compact/tool_result_budget.rs` | 225 | 超大结果（>100K 字符）保存到磁盘，替换为预览 |
| `src/compact/messages.rs` | 307 | API 消息规范化、交替模式保证、边界检测、工具函数 |
| `src/commands/compact.rs` | 182 | `/compact` 斜杠命令（支持自定义指令） |
| `tests/e2e_compact.rs` | 158 | 12 个 E2E 测试 |

### 集成变更

| 文件 | 变更 |
|------|------|
| `src/main.rs` | +`mod compact;` |
| `src/commands/mod.rs` | +`mod compact;` + 注册 `/compact` 命令 |
| `CLAUDE.md` | 更新架构图，从"已移除"列表中移除 compact |

### 测试结果

- [x] `cargo build` 通过（0 个新 warning）
- [x] 27 个单元测试全部通过（compact 模块内置）
- [x] 12 个 E2E 测试全部通过（二进制启动、命令注册、路径隔离、不崩溃）
- [x] 0 个新依赖
- [x] 已有 597 个测试不受影响

### 已就绪的集成点（无需额外修改）

- **query/loop_impl.rs**：STEP 2 CONTEXT 阶段已通过 `QueryDeps.microcompact()` 和 `QueryDeps.autocompact()` 调用压缩
- **query/deps.rs**：`QueryDeps` trait 已定义 `microcompact`、`autocompact`、`reactive_compact` 三个方法
- **types/state.rs**：`AutoCompactTracking` 和 `QueryLoopState.has_attempted_reactive_compact` 已存在
- **engine/lifecycle.rs**：`SystemSubtype::CompactBoundary` 已处理

### 待后续优化

- 完整 API 压缩（调用模型生成摘要）需要 `QueryDeps` 实现方接入 `compact::compaction`
- `/compact` 命令目前仅运行本地管道（无 API 调用），完整版需接入模型摘要生成

---

## 6. Phase 2: Agent 工具 ✅ 完成

> **日期**: 2026-04-07 | **新增**: +555 行, 1 新文件 + 5 文件修改

### 实际移植结果

| 文件 | 行数 | 功能 |
|------|------|------|
| `src/tools/agent.rs` | 558 | Agent 工具完整实现（从 master-feature 789 行适配精简） |

### 核心能力

- **子 QueryEngine 派生**：创建独立 engine 实例，隔离消息历史，max_turns=30
- **Worktree 隔离**：完整实现 `isolation: "worktree"`，创建临时 git worktree → 运行 → 检测变更 → 自动清理
- **模型别名解析**：`"sonnet"` / `"opus"` / `"haiku"` → 完整模型 ID
- **递归深度限制**：`MAX_AGENT_DEPTH = 5`，通过 `QueryChainTracking.depth` 传播
- **优雅降级**：worktree 创建失败时自动回退到普通模式并附加警告
- **Background 执行**：stub（日志警告，同步运行），待 Phase 4 Task 基础设施后完善

### 集成变更

| 文件 | 变更 |
|------|------|
| `src/types/config.rs` | +`AgentContext` 结构体 + `QueryEngineConfig.agent_context` 字段 |
| `src/engine/lifecycle.rs` | `QueryEngineDeps` 加 `agent_context` 字段，`execute_tool()` 传播到 `ToolUseContext` |
| `src/tools/mod.rs` | +`pub mod agent;` |
| `src/tools/registry.rs` | 注册 `AgentTool` |
| `src/main.rs` | `QueryEngineConfig` 构建加 `agent_context: None` |
| `CLAUDE.md` | 工具数 13→14，工具列表加 Agent |

### Bugfix: 深度传播修复

master-feature 的 `execute_tool()` 总是设置 `query_tracking: None`，导致嵌套 sub-agent（depth ≥ 2）无法正确检查递归深度。本次移植通过 `AgentContext` 在 `QueryEngineConfig` 中传播 depth，修复了此 bug：

```
主 engine (depth 0) → execute_tool { query_tracking: None }
  └─ AgentTool → 创建子 engine (depth 1)
       └─ execute_tool { query_tracking: Some(depth=1) }  ← 修复
            └─ AgentTool → depth=1 < 5, 可继续
                 └─ ... 直到 depth=5 拒绝
```

### SendMessage 决策

`send_message.rs` (436 行) **不在此阶段移植** — 它完全依赖 `teams/` 模块 (Phase 8) 的 mailbox IPC 机制，独立移植无意义。

### 测试结果

- [x] `cargo build` 通过（0 个新 warning）
- [x] 8 个 Agent 单元测试全部通过（model alias、schema、name、concurrency、isolation、deserialization）
- [x] 682 个测试全部通过，无回归
- [x] 0 个新依赖
- [x] `--dump-system-prompt` 包含完整 Agent 工具描述和 JSON schema

### 待后续优化

- Background 执行需要 Task 管理基础设施 (Phase 4)
- SendMessage 需要 Teams 模块 (Phase 8)
- Abort 信号从父 engine 传播到子 engine（当前子 engine 使用独立 aborted 标志）

---

## 7. Phase 3: Web 工具 ✅ 完成

> **日期**: 2026-04-07 | **新增**: +1,031 行, 2 新文件

### 实际移植结果

| 文件 | 行数 | 功能 |
|------|------|------|
| `src/tools/web_fetch.rs` | 531 | URL 内容获取：HTML→文本转换、LRU 缓存 (15min TTL, 64 entries)、HTTPS 升级、100KB 截断 |
| `src/tools/web_search.rs` | 500 | Brave Search API 集成：域名过滤、结构化结果、动态日期提示 |

### 核心能力

**WebFetch**:
- HTTP GET + 60s 超时 + 10 次重定向跟随
- HTML 标签剥离（含 script/style 块过滤）+ 实体解码
- 内容截断至 100K 字符（首尾各 50K + 省略标记）
- 响应缓存：LazyLock + Mutex HashMap, 15 分钟 TTL, LRU 淘汰
- URL 规范化：http→https 升级、自动添加 scheme、长度限制 2000 字符

**WebSearch**:
- Brave Search API (`BRAVE_SEARCH_API_KEY` 环境变量)
- 域名白名单/黑名单过滤
- 结构化结果 + Markdown 格式化文本
- 动态月份/年份注入到系统提示

### 集成变更

| 文件 | 变更 |
|------|------|
| `src/tools/mod.rs` | +`pub mod web_fetch; pub mod web_search;` |
| `src/tools/registry.rs` | 注册 `WebFetchTool` 和 `WebSearchTool` |
| `CLAUDE.md` | 工具数 14→16，工具列表加 WebFetch, WebSearch |

### 适配改动（相对 master-feature）

- 移除 `#![allow(unused)]`，清理未使用的 `BraveQuery` 结构体和 `query` 字段
- 0 个新 crate 依赖（`reqwest`, `url`, `chrono` 均已在 rust-lite 中）

### 测试结果

- [x] `cargo build` 通过（0 个新 warning）
- [x] 15 个 WebFetch 单元测试全部通过（HTML 剥离、实体解码、URL 规范化、缓存、截断）
- [x] 9 个 WebSearch 单元测试全部通过（域名过滤、结果格式化、输入验证）
- [x] 705 个测试全部通过，无回归
- [x] 0 个新依赖

---

## 8. Phase 4: Plan Mode 与 Task 工具 ✅ 完成

> **日期**: 2026-04-07 | **新增**: +1,062 行, 2 新文件

### 实际移植结果

| 文件 | 行数 | 功能 |
|------|------|------|
| `src/tools/plan_mode.rs` | 419 | EnterPlanMode / ExitPlanMode — 只读模式切换，权限状态保存/恢复 |
| `src/tools/tasks.rs` | 643 | TaskStore (LazyLock 单例) + 6 个任务工具：Create/Get/Update/List/Stop/Output |

### 核心能力

**Plan Mode**:
- `EnterPlanMode`：保存当前 PermissionMode 到 `pre_plan_mode`，切换到 `Plan` 模式
- `ExitPlanMode`：恢复 `pre_plan_mode`（支持 Default/Auto/Bypass 恢复），需用户确认
- 禁止在 agent 上下文中进入 plan 模式
- 防止重复进入 plan 模式
- `permissions/decision.rs` 已有 Plan 模式下拒绝写工具的逻辑

**Task 系统**:
- `TaskStore`：`Arc<Mutex<HashMap>>` 内存存储，LazyLock 全局单例
- 完整生命周期：Pending → InProgress → Completed / Stopped
- `append_output`：为 background agent 执行预留日志追加接口

### 集成变更

| 文件 | 变更 |
|------|------|
| `src/tools/mod.rs` | +`pub mod plan_mode; pub mod tasks;` |
| `src/tools/registry.rs` | 注册 8 个工具 (2 plan + 6 task) |
| `CLAUDE.md` | 工具数 16→24 |

### 测试结果

- [x] `cargo build` 通过（0 个新 warning）
- [x] 8 个 PlanMode 测试全部通过（agent 阻止、重复进入阻止、roundtrip、模式恢复）
- [x] 9 个 Task 测试全部通过（CRUD、生命周期、JSON 序列化）
- [x] 0 个新依赖
- [ ] Task 列表正确显示

---

## 9. Phase 5: Worktree 工具 ✅ 完成

> **日期**: 2026-04-07 | **新增**: +700 行, 1 新文件

### 实际移植结果

| 文件 | 行数 | 功能 |
|------|------|------|
| `src/tools/worktree.rs` | 700 | EnterWorktree / ExitWorktree — git worktree 隔离 + 会话管理 |

### 核心能力

- **EnterWorktree**：创建临时 git worktree（`git worktree add -B`），支持自定义 slug 命名
- **ExitWorktree**：keep（保留分支+目录）或 remove（清理 worktree + 删除分支）
- **WorktreeSession**：LazyLock 全局单例，防止嵌套
- **安全机制**：
  - 不可嵌套（validate_input 检查现有 session）
  - Fail-closed：无法确定 git 状态时拒绝删除
  - 变更检测：删除前检查 uncommitted files + new commits
  - `discard_changes: true` 才能强制删除有未保存工作的 worktree
  - Slug 路径安全验证（禁止 `..`、`/`、`\`，长度限制 64）

### 测试结果

- [x] `cargo build` 通过（0 个新 warning）
- [x] 9 个单元测试全部通过（slug 验证、schema、session 生命周期、嵌套阻止、无效 action）
- [x] 730 个测试全部通过，无回归
- [x] 0 个新依赖

### 里程碑

**工具数达到 28/28 — 与 master-feature 持平**（含不同工具组合）

---

## 10. Phase 6: MCP 协议支持

### 复杂度警告

这是移植复杂度最高的模块之一，涉及新的通信协议和外部依赖。

### 源文件

| 文件 | 行数 | 来源 |
|------|------|------|
| `src/mcp/mod.rs` | - | master-feature |
| `src/mcp/client.rs` | 1,008 | master-feature |
| `src/mcp/discovery.rs` | - | master-feature |
| `src/mcp/tools.rs` | - | master-feature |

### 新依赖

```toml
# Cargo.toml 新增
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-webpki-roots"] }
eventsource-stream = "0.2"
```

### 核心实现

```rust
// MCP 协议常量
pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const CLIENT_NAME: &str = "claude-code-rs";
pub const CONNECT_TIMEOUT_SECS: u64 = 30;
pub const TOOL_CALL_TIMEOUT_SECS: u64 = 300;

// 传输方式
// 1. stdio: 通过子进程 stdin/stdout 通信
// 2. SSE: 通过 Server-Sent Events (需要 WebSocket)

// 服务器配置
pub struct McpServerConfig {
    pub name: String,
    pub transport: String,  // "stdio" | "sse"
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, String>>,
}
```

### 集成点

1. **config/settings.rs**：添加 MCP 服务器配置解析
2. **tools/registry.rs**：动态注册 MCP 提供的工具
3. **commands/mcp.rs**：添加 `/mcp` 命令
4. **engine/lifecycle.rs**：启动时初始化 MCP 连接

### 验证

- [ ] stdio 传输模式可连接 MCP 服务器
- [ ] SSE 传输模式可连接
- [ ] MCP 工具在工具列表中显示
- [ ] MCP 工具可被模型调用
- [ ] 超时和错误处理正确

---

## 11. Phase 7: LSP 集成

### 源文件

| 文件 | 行数 | 来源 |
|------|------|------|
| `src/tools/lsp.rs` | 677 | master-feature |
| `src/lsp_service/mod.rs` | 292 | master-feature |

### 新依赖

```toml
lsp-types = "0.97"
```

### 集成点

1. **tools/registry.rs**：注册 LSP 工具
2. rust-lite 的 `services/lsp_lifecycle.rs` 可作为基础（已有存根）

### 验证

- [ ] 可启动 LSP 服务器进程
- [ ] 获取诊断/补全/定义跳转
- [ ] 服务器生命周期管理正确

---

## 12. Phase 8: 多 Agent Teams

### 实验性功能

此模块受 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 环境变量控制。

### 源文件

| 文件 | 行数 | 来源 |
|------|------|------|
| `src/teams/mod.rs` | - | master-feature |
| `src/teams/types.rs` | - | Team/Agent 类型定义 |
| `src/teams/identity.rs` | - | 团队身份管理 |
| `src/teams/context.rs` | - | 执行上下文 |
| `src/teams/protocol.rs` | - | 消息协议 |
| `src/teams/mailbox.rs` | 440 | 文件邮箱 IPC |
| `src/teams/helpers.rs` | 446 | 工具函数 |
| `src/teams/backend.rs` | - | 后端实现 |
| `src/teams/in_process.rs` | - | 进程内执行器 |
| `src/teams/runner.rs` | - | 团队运行器 |
| `src/teams/constants.rs` | - | 常量 |

### 依赖

- 需要 Phase 2 (Agent 工具) 完成
- 无新 crate（使用文件系统 IPC）

### IPC 目录

```
~/.cc-rust/teams/{team_name}/inboxes/{agent_name}/
```

### 验证

- [ ] 环境变量控制功能开关
- [ ] Team Lead 可创建 Teammates
- [ ] 邮箱 IPC 消息正确传递
- [ ] 团队生命周期管理正确

---

## 13. Phase 9: 插件系统

### 源文件

| 文件 | 行数 | 来源 |
|------|------|------|
| `src/plugins/mod.rs` | - | master-feature |
| `src/plugins/loader.rs` | - | 插件加载器 |
| `src/plugins/manifest.rs` | - | Manifest 解析 |

### 集成点

1. **skills/mod.rs**：`SkillSource::Plugin(String)` 已在 rust-lite 中定义
2. **config/settings.rs**：添加插件配置
3. **commands/plugin.rs**：添加 `/plugin` 命令

### 验证

- [ ] 插件发现与加载
- [ ] 插件提供的技能/工具正确注册
- [ ] 插件隔离（不影响核心功能）

---

## 14. Phase 10: 云服务集成

### 重量级依赖警告

此 Phase 会显著增加编译时间和二进制体积。建议作为可选 feature flag。

### 新依赖

```toml
# Cargo.toml — 建议放在 [features] 下
aws-config = "1"
aws-sdk-bedrockruntime = "1"
gcp_auth = "0.12"
jsonwebtoken = "9"
oauth2 = "5"
```

### 建议方案

```toml
[features]
default = []
cloud = ["aws", "gcp"]
aws = ["dep:aws-config", "dep:aws-sdk-bedrockruntime"]
gcp = ["dep:gcp_auth"]
oauth = ["dep:oauth2", "dep:jsonwebtoken"]
```

### 源文件

master-feature 中的 `ApiProvider::Bedrock` 和 `ApiProvider::Vertex` 目前是接口定义（标注为 "interface only"），实际调用未完成实现。移植时只需搬运接口层。

### 验证

- [ ] `cargo build` (无 cloud feature) 通过
- [ ] `cargo build --features cloud` 通过
- [ ] Bedrock 接口可实例化
- [ ] Vertex 接口可实例化

---

## 15. Phase 11: 剩余命令补全

### 待移植命令列表 (~25 个)

| 类别 | 命令 | 优先级 |
|------|------|--------|
| **高级 Git** | review, ultrareview, commit-push-pr, pr-comments | 高 |
| **上下文** | ~~compact~~ (已完成), rewind/undo | 高（依赖 Phase 1） |
| **诊断** | doctor/diag, stats | 中 |
| **配置** | theme, color, keybindings/keys | 中 |
| **编辑器** | vim (命令) | 中 |
| **MCP** | mcp | 中（依赖 Phase 6） |
| **插件** | plugin | 低（依赖 Phase 9） |
| **任务** | tasks | 中（依赖 Phase 4） |
| **会话** | rename, tag, brief | 低 |
| **高级** | ultraplan, advisor, think-back, voice, sandbox, add-dir | 低 |

### 策略

- 每个命令是独立文件，可逐个移植
- 注意 `commands/mod.rs` 注册表（master-feature 726 行 vs rust-lite 356 行）
- 部分命令依赖前序 Phase 完成

---

## 16. 依赖管理

### 移植各 Phase 的 Cargo.toml 变更

| Phase | 新增依赖 | 影响 |
|-------|---------|------|
| 1-5 | 无 | 零依赖变更 |
| 6 | `tokio-tungstenite`, `eventsource-stream` | +2 crate，中等影响 |
| 7 | `lsp-types` | +1 crate，低影响 |
| 8-9 | 无 | 零依赖变更 |
| 10 | `aws-config`, `aws-sdk-bedrockruntime`, `gcp_auth`, `oauth2`, `jsonwebtoken` | +5 crate，高影响（建议 feature flag） |

### 其他可选依赖

```toml
tree-sitter = "0.24"   # AST 解析，用于代码分析（Phase 7 或独立）
image = "0.25"          # 图像处理（按需）
fs4 = "0.12"            # 文件锁（并发安全，建议在 Phase 1 同时添加）
```

---

## 17. 保留 rust-lite 独有功能

以下 rust-lite 独有功能**必须保留**，不被 master-feature 代码覆盖：

| 功能 | 文件 | 说明 |
|------|------|------|
| **ProcessState 全局单例** | `src/bootstrap/` (7 文件) | 进程级状态管理，比 master-feature 更成熟 |
| **E2E 测试** | `tests/e2e_*.rs` (5 文件, 含 e2e_compact) | master-feature 无测试 |
| **.env 自动加载** | `dotenvy` 依赖 | 方便开发配置 |
| **欢迎界面** | `src/ui/welcome.rs` | ASCII logo + 两列布局 |
| **会话自动保存** | `src/session/` 增强 | 更可靠的持久化 |
| **memdir** | `src/session/memdir.rs` | CLAUDE.md 内存目录 |
| **服务层** | `src/services/` (4 文件) | ToolUseSummary, SessionMemory, PromptSuggestion, LSP Lifecycle |
| **扩展工具** | PowerShell, Config, REPL, StructuredOutput, Hooks | rust-lite 独有工具 |
| **额外命令** | `/extra-usage`, `/rate-limit` | rust-lite 独有命令 |
| **SDK 输出** | JSONL 输出模式 | 程序化访问 |

### 冲突解决原则

当 master-feature 和 rust-lite 对同一文件有不同实现时：
1. **以 rust-lite 为基础**，添加 master-feature 的新功能
2. **不替换** rust-lite 的改进实现
3. 如有结构冲突，优先保留 rust-lite 的 API 设计

---

## 18. 验证清单

### 每个 Phase 完成后的通用检查

- [ ] `cargo build` 成功（无 warning 优先）
- [ ] `cargo test` 全部通过（包括现有 E2E 测试）
- [ ] `cargo clippy` 无新 warning
- [ ] 新模块有基本文档注释
- [ ] 工具注册表 (`tools/registry.rs`) 正确更新
- [ ] 命令注册表 (`commands/mod.rs`) 正确更新（如适用）

### 最终集成测试

- [ ] 冷启动到交互模式正常
- [ ] Print 模式 (`-p`) 正常
- [ ] 会话恢复 (`--resume`) 正常
- [ ] 所有 15+ 个 LLM provider 可连接
- [ ] 权限系统在所有新工具上生效
- [ ] Vim 模式不受影响
- [ ] 长对话（100+ turn）不崩溃（Phase 1 完成后）

---

## 19. 附录：完整差异表

### A. 工具对照表

| 工具 | master-feature | rust-lite | 移植 Phase |
|------|:-:|:-:|:-:|
| Bash | ✅ | ✅ | - |
| Read | ✅ | ✅ | - |
| Write | ✅ | ✅ | - |
| Edit | ✅ | ✅ | - |
| Glob | ✅ | ✅ | - |
| Grep | ✅ | ✅ | - |
| AskUser | ✅ | ✅ | - |
| Skill | ✅ | ✅ | - |
| PowerShell | ❌ | ✅ | 保留 |
| Config | ❌ | ✅ | 保留 |
| REPL | ❌ | ✅ | 保留 |
| StructuredOutput | ❌ | ✅ | 保留 |
| SendUserMessage | ❌ | ✅ | 保留 |
| Hooks | ❌ | ✅ | 保留 |
| Agent | ✅ | ❌ | Phase 2 |
| SendMessage | ✅ | ❌ | Phase 2 |
| WebFetch | ✅ | ❌ | Phase 3 |
| WebSearch | ✅ | ❌ | Phase 3 |
| EnterPlanMode | ✅ | ❌ | Phase 4 |
| ExitPlanMode | ✅ | ❌ | Phase 4 |
| TaskCreate | ✅ | ❌ | Phase 4 |
| TaskGet | ✅ | ❌ | Phase 4 |
| TaskUpdate | ✅ | ❌ | Phase 4 |
| TaskList | ✅ | ❌ | Phase 4 |
| TaskStop | ✅ | ❌ | Phase 4 |
| TaskOutput | ✅ | ❌ | Phase 4 |
| EnterWorktree | ✅ | ❌ | Phase 5 |
| ExitWorktree | ✅ | ❌ | Phase 5 |
| LSP | ✅ | ❌ | Phase 7 |
| NotebookEdit | ✅ | ❌ | Phase 7 |
| TeamCreate | ✅ | ❌ | Phase 8 |
| TeamDelete | ✅ | ❌ | Phase 8 |
| ToolSearch | ✅ | ❌ | Phase 3 |
| Sleep | ✅ | ❌ | Phase 3 |
| SnipTool | ✅ | ❌ | Phase 3 |
| TodoWrite | ✅ | ❌ | Phase 4 |

### B. 命令对照表

| 命令 | master-feature | rust-lite | 移植 Phase |
|------|:-:|:-:|:-:|
| help | ✅ | ✅ | - |
| clear | ✅ | ✅ | - |
| exit/quit/q | ✅ | ✅ | - |
| version | ✅ | ✅ | - |
| model | ✅ | ✅ | - |
| config | ✅ | ✅ | - |
| cost/usage | ✅ | ✅ | - |
| session | ✅ | ✅ | - |
| resume | ✅ | ✅ | - |
| diff | ✅ | ✅ | - |
| commit | ✅ | ✅ | - |
| branch | ✅ | ✅ | - |
| context | ✅ | ✅ | - |
| files | ✅ | ✅ | - |
| permissions | ✅ | ✅ | - |
| login | ✅ | ✅ | - |
| logout | ✅ | ✅ | - |
| memory | ✅ | ✅ | - |
| skills | ✅ | ✅ | - |
| status | ✅ | ✅ | - |
| fast | ✅ | ✅ | - |
| effort | ✅ | ✅ | - |
| export | ✅ | ✅ | - |
| init | ✅ | ✅ | - |
| copy | ✅ | ✅ | - |
| hooks | ✅ | ✅ | - |
| extra-usage | ❌ | ✅ | 保留 |
| rate-limit | ❌ | ✅ | 保留 |
| compact | ✅ | ✅ | Phase 1 ✅ |
| review | ✅ | ❌ | Phase 11 |
| rewind/undo | ✅ | ❌ | Phase 11 |
| doctor/diag | ✅ | ❌ | Phase 11 |
| theme | ✅ | ❌ | Phase 11 |
| color | ✅ | ❌ | Phase 11 |
| keybindings | ✅ | ❌ | Phase 11 |
| vim | ✅ | ❌ | Phase 11 |
| mcp | ✅ | ❌ | Phase 6 |
| plugin | ✅ | ❌ | Phase 9 |
| tasks | ✅ | ❌ | Phase 4 |
| rename | ✅ | ❌ | Phase 11 |
| stats | ✅ | ❌ | Phase 11 |
| tag | ✅ | ❌ | Phase 11 |
| brief | ✅ | ❌ | Phase 11 |
| add-dir | ✅ | ❌ | Phase 11 |
| sandbox | ✅ | ❌ | Phase 11 |
| ultraplan | ✅ | ❌ | Phase 11 |
| ultrareview | ✅ | ❌ | Phase 11 |
| advisor | ✅ | ❌ | Phase 11 |
| think-back | ✅ | ❌ | Phase 11 |
| voice | ✅ | ❌ | Phase 11 |
| commit-push-pr | ✅ | ❌ | Phase 11 |
| pr-comments | ✅ | ❌ | Phase 11 |

### C. 模块对照表

| 模块 | master-feature | rust-lite | 说明 |
|------|:-:|:-:|------|
| api/ | ✅ | ✅ | 基本一致 |
| auth/ | ✅ | ✅ | 基本一致 |
| bootstrap/ | ❌ | ✅ | rust-lite 独有改进 |
| commands/ | ✅ (47+) | ✅ (27) | 需补全 |
| compact/ | ✅ | ✅ | Phase 1 ✅ 已完成 |
| config/ | ✅ | ✅ | 基本一致 |
| engine/ | ✅ | ✅ | 基本一致 |
| lsp_service/ | ✅ | ❌ | Phase 7 移植 |
| mcp/ | ✅ | ❌ | Phase 6 移植 |
| permissions/ | ✅ | ✅ | 基本一致 |
| plugins/ | ✅ | ❌ | Phase 9 移植 |
| query/ | ✅ | ✅ | 基本一致 |
| remote/ | ✅ | ❌ | 可选移植 |
| services/ | ❌ | ✅ | rust-lite 独有 |
| session/ | ✅ | ✅ | rust-lite 更完善 |
| skills/ | ✅ | ✅ | 基本一致 |
| teams/ | ✅ | ❌ | Phase 8 移植 |
| tools/ | ✅ (28) | ✅ (15) | 需补全 |
| types/ | ✅ | ✅ | 基本一致 |
| ui/ | ✅ | ✅ | rust-lite 有欢迎界面 |
| utils/ | ✅ | ✅ | 基本一致 |
| analytics/ | ✅ | ❌ | 可选移植 |

---

## 时间线与进度

| 阶段 | 预估工作量 | 累计功能覆盖 | 状态 | 完成日期 |
|------|-----------|-------------|------|---------|
| Phase 1 (compact) | 中 | 60% | ✅ 完成 | 2026-04-07 |
| Phase 2 (Agent) | 中 | 70% | ⬚ 待开始 | - |
| Phase 3 (Web) | 低 | 75% | ⬚ 待开始 | - |
| Phase 4 (Plan+Task) | 中 | 80% | ⬚ 待开始 | - |
| Phase 5 (Worktree) | 中 | 82% | ⬚ 待开始 | - |
| Phase 6 (MCP) | 高 | 87% | ⬚ 待开始 | - |
| Phase 7 (LSP) | 高 | 90% | ⬚ 待开始 | - |
| Phase 8 (Teams) | 高 | 92% | ⬚ 待开始 | - |
| Phase 9 (Plugins) | 中 | 94% | ⬚ 待开始 | - |
| Phase 10 (Cloud) | 高 | 96% | ⬚ 待开始 | - |
| Phase 11 (Commands) | 低 | 100% | ⬚ 待开始 | - |

完成 Phase 1-5（无新依赖）即可达到 ~82% 功能覆盖，是性价比最高的阶段。

### 变更日志

| 日期 | Commit | 内容 |
|------|--------|------|
| 2026-04-07 | `0e7d001` | Phase 1: compact 模块移植 (+2,918 行, 27 单元测试 + 12 E2E 测试) |
