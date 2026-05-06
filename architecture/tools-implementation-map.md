# Bun Tools 实现映射

## 范围

本文只覆盖 Bun 上游 `docs/tools` 下的五篇文档，以及它们在 cc-rust 中对应的工具族群实现。本文只记录当前代码证据，不讨论源码重构，也不推断未来计划。

## 上游文档清单

| Bun 文档 | 主题 | cc-rust 族群 | 当前状态 |
| --- | --- | --- | --- |
| `what-are-tools.mdx` | 工具系统、Tool trait、注册和权限链路 | 工具系统 | 已实现 |
| `file-operations.mdx` | Read / Edit / Write、读写安全、历史保护 | 文件操作 | 已实现 |
| `search-and-navigation.mdx` | Glob、Grep、ToolSearch、WebSearch、WebFetch | 搜索导航、网络工具差异 | 已实现 |
| `shell-execution.mdx` | BashTool、权限、超时、后台化、输出流 | Shell 执行 | 已实现 |
| `task-management.mdx` | TodoWrite V1、Tasks V2、依赖和输出管理 | 任务管理 | 已实现 |

## 实现映射表

| 工具族群 | Bun 主题 | cc-rust 入口 | 状态 | 说明 |
| --- | --- | --- | --- | --- |
| 工具系统 | 工具抽象、注册、schema、权限、结果 | `crates/cc-engine/src/types/tool.rs:192-269`，`crates/claude-code-rs/src/tools/registry.rs:41-83` | 已实现 | `Tool` trait 已提供 `name`、`input_json_schema`、`validate_input`、`check_permissions`、`call`、`prompt`、`max_result_size_chars` 等核心能力；registry 先聚合 `fs::tools()` 和 `exec::tools()`，再追加单工具和插件工具。 |
| 文件操作 | Read / Edit / Write | `crates/claude-code-rs/src/tools/fs/mod.rs:14-31`，`crates/claude-code-rs/src/tools/fs/file_read.rs:706-889`，`crates/claude-code-rs/src/tools/fs/file_edit.rs:329-673`，`crates/claude-code-rs/src/tools/fs/file_write.rs:41-219`，`crates/claude-code-rs/src/tools/fs/safe_write.rs:50-132` | 已实现 | Read 支持文本、图片、PDF、Notebook；Edit 强制先读后写并校验未被外部修改；Write 通过安全写入和 `FileChanged` hook 完成原子替换。 |
| 搜索导航 | Glob / Grep / ToolSearch / LSP | `crates/claude-code-rs/src/tools/fs/glob_tool.rs`，`crates/claude-code-rs/src/tools/fs/grep.rs:1-218`，`crates/claude-code-rs/src/tools/tool_search.rs:504-627`，`crates/claude-code-rs/src/tools/lsp.rs:1-435` | 已实现 | Glob、Grep、ToolSearch、LSP 都已接入 registry；Glob 已按修改时间倒序返回，路径作为稳定兜底。Rust 额外提供 LSP code intelligence，属于 Bun 文档未单列的扩展能力。 |
| Shell 执行 | BashTool | `crates/claude-code-rs/src/tools/exec/mod.rs:14-30`，`crates/claude-code-rs/src/tools/exec/bash.rs:134-668` | 已实现 | Bash 具备命令解析、危险命令检测、sandbox 预检、超时、进程组控制和实时输出流。Rust 还额外暴露了 PowerShell、Repl、Sleep。 |
| 任务管理 | TodoWrite V1、Tasks V2 | `crates/claude-code-rs/src/tools/tasks.rs`，`crates/claude-code-rs/src/tools/registry.rs` | 已实现 | Rust 已提供 `TodoWrite` V1 兼容入口，以及 V2 任务体系：`TaskCreate`、`TaskGet`、`TaskUpdate`、`TaskList`、`TaskStop`、`TaskOutput`。新任务已使用递增 ID，并通过 `.highwatermark` 防止删除后复用；依赖真源仍是 `depends_on`，但已提供 Bun 兼容的 `blocked_by` / `blockedBy` 输入别名、`blocks` 反向输出、owner claim / agent-busy 检查、task-list 存储隔离、任务列表级 `.lock`、`activeForm` / `metadata` 和 teammate 退出后的 owner 释放。 |
| 网络工具差异 | WebSearch / WebFetch | `crates/claude-code-rs/src/tools/web_search/tool.rs:1-258`，`crates/claude-code-rs/src/tools/web_fetch.rs:1-723`，`crates/claude-code-rs/src/tools/registry.rs:22-24,56-57` | 已实现 | Rust 端已提供 WebSearch 和 WebFetch，但实现路径不同于 Bun：WebSearch 走 Tavily / Brave provider，WebFetch 走 sandbox 网络策略、URL 归一化、同源重定向限制和内存缓存；provider 差异见 `architecture/web-tools-provider-diff.md`。 |

## 逐文档分析

### `what-are-tools.mdx`

- 上游主题是“什么是工具、如何注册、如何定义输入 schema、如何走权限与执行链路”。
- cc-rust 对应实现集中在 `Tool` trait 和工具注册器。
- `Tool` trait 直接定义了工具的核心面：`name`、`input_json_schema`、`validate_input`、`check_permissions`、`call`、`prompt`、`max_result_size_chars` 等，见 `crates/cc-engine/src/types/tool.rs:192-269`。
- registry 先拼装文件系统和执行子域，再加入任务、LSP、Web、ToolSearch 等单工具，见 `crates/claude-code-rs/src/tools/registry.rs:41-83`。
- 证据还包括 `fs::tools()` 和 `exec::tools()` 的域内聚合，见 `crates/claude-code-rs/src/tools/fs/mod.rs:14-31`、`crates/claude-code-rs/src/tools/exec/mod.rs:14-30`。
- 状态：已实现。
- 结论：Bun 文档里关于工具系统的核心抽象，cc-rust 已经有对应实现；渲染职责已从 `Tool` trait 分离到 UI 层，这一点在 `crates/cc-engine/src/types/tool.rs:192-269` 的注释里已经说明。

### `file-operations.mdx`

- 上游主题是 Read、Edit、Write 三类文件工具，以及读写安全、历史保护和原子写入。
- cc-rust 的 Read 实现已经覆盖文本、图片、PDF、Notebook 四条路径，见 `crates/claude-code-rs/src/tools/fs/file_read.rs:135-175`、`crates/claude-code-rs/src/tools/fs/file_read.rs:354-419`、`crates/claude-code-rs/src/tools/fs/file_read.rs:789-860`。
- Read 具备只读与并发安全标记，见 `crates/claude-code-rs/src/tools/fs/file_read.rs:706-889`。
- Edit 强制先读后写，并在写入前校验缓存内容与磁盘内容一致，见 `crates/claude-code-rs/src/tools/fs/file_edit.rs:93-105`、`crates/claude-code-rs/src/tools/fs/file_edit.rs:382-492`、`crates/claude-code-rs/src/tools/fs/file_edit.rs:570-623`。
- Write 使用 `validate_write_request` 和 `safe_write_text` 完成安全写入，并在成功后发出 `FileChanged` hook，见 `crates/claude-code-rs/src/tools/fs/file_write.rs:85-198`。
- `safe_write.rs` 提供原子替换、备份和权限保留能力，见 `crates/claude-code-rs/src/tools/fs/safe_write.rs:50-132`、`crates/claude-code-rs/src/tools/fs/safe_write.rs:250-332`。
- 状态：已实现。
- 结论：文件操作这章在 Rust 里已经有完整的可用路径，且比 Bun 文档里的基础文本读写更宽，额外支持图片、PDF 和 Notebook 读取。

### `search-and-navigation.mdx`

- 上游主题是 Glob、Grep、ToolSearch，并把 WebSearch / WebFetch 作为同章中的网络检索能力。
- Glob 和 Grep 都已经进入文件系统子域，见 `crates/claude-code-rs/src/tools/fs/mod.rs:14-31`。
- Glob 的核心实现位于 `crates/claude-code-rs/src/tools/fs/glob_tool.rs`；结果会按修改时间倒序排序，并用路径升序作为稳定兜底。
- Grep 先尝试外部 `rg`，失败后回退到内部 walker + regex，见 `crates/claude-code-rs/src/tools/fs/grep.rs:1-218`。
- ToolSearch 支持自然语言查询、`select:<tool-name>` 精确选择、来源过滤和 schema hydration，见 `crates/claude-code-rs/src/tools/tool_search.rs:504-627`、`crates/claude-code-rs/src/tools/tool_search.rs:763-781`。
- LSP 作为 Rust 额外扩展，提供 definition、reference、hover、symbol、completion、diagnostics 等能力，见 `crates/claude-code-rs/src/tools/lsp.rs:1-11`、`crates/claude-code-rs/src/tools/lsp.rs:236-435`。
- 状态：已实现。
- 结论：搜索与导航的主链路已经落地；Glob 的结果排序语义已补齐到 Bun 文档描述，Rust 额外提供 LSP 扩展。

### `shell-execution.mdx`

- 上游主题是 BashTool 的安全执行链路：输入校验、危险命令检测、超时、后台化、输出截断和进度回调。
- Rust 的执行子域入口见 `crates/claude-code-rs/src/tools/exec/mod.rs:14-30`，其中包含 Bash、PowerShell、Repl、Sleep。
- BashTool 本体位于 `crates/claude-code-rs/src/tools/exec/bash.rs:134-668`。
- 它在 `validate_input` 中检查可解析性、未闭合引号和 heredoc，见 `crates/claude-code-rs/src/tools/exec/bash.rs:206-246`。
- 它在 `check_permissions` 中执行危险命令检测、前缀拒绝规则和 `dangerouslyDisableSandbox` 门禁，见 `crates/claude-code-rs/src/tools/exec/bash.rs:247-299`。
- 它在 `call` 中完成 sandbox 预检、进程组控制、超时处理和实时输出流，见 `crates/claude-code-rs/src/tools/exec/bash.rs:303-668`。
- 状态：已实现。
- 结论：BashTool 的核心安全设计已经在 Rust 端复现，同时 Rust 还补了 PowerShell、Repl、Sleep 这几个执行子能力。

### `task-management.mdx`

- 上游主题同时包含 TodoWrite V1 和 Tasks V2。
- Rust 现在同时提供 `TodoWrite` V1 兼容入口和任务 V2 体系，见 `crates/claude-code-rs/src/tools/tasks.rs` 与 `crates/claude-code-rs/src/tools/registry.rs`。
- `TodoWrite` 采用 Bun V1 的全量替换语义：输入 `todos[]` 覆盖当前 session / agent todo 列表，全部 `completed` 时清空列表，并在 3 个以上任务全部完成但缺少验证项时返回验证提示。
- 任务存储默认落在 `~/.cc-rust/tasks/<task-list-id>/` 或 `$CC_RUST_HOME/tasks/<task-list-id>/`，默认列表为 `tasklist`；首次打开默认列表时会非破坏性复制旧 flat 目录任务，见 `crates/claude-code-rs/src/tools/tasks.rs`。
- `TaskStore` / `TaskRepository` 负责持久化、恢复、输出保留和 schema 迁移，见 `crates/claude-code-rs/src/tools/tasks.rs:76-536`、`crates/claude-code-rs/src/tools/tasks.rs:550-931`。
- `TaskCreate` 仍以 `depends_on` 为内部依赖真源，但接受 Bun 兼容的 `blocked_by` / `blockedBy` 输入别名；`task_to_json()` 会输出 `depends_on`、`blocked_by`、`blockedBy`，并计算反向 `blocks` 列表。
- `TaskCreate` / `TaskUpdate` 已补齐 Tasks V2 的 `activeForm` 和通用 `metadata`；`TaskUpdate` 支持 subject、description、owner、metadata merge/null-delete、`addBlocks`、`addBlockedBy` 和 `deleted`。
- `TaskCreate`、`TaskGet`、`TaskUpdate`、`TaskList`、`TaskStop`、`TaskOutput` 的实现分别见 `crates/claude-code-rs/src/tools/tasks.rs:1640-2442`。
- 任务 ID 已改为目录内递增编号，`TaskStore::create_with_options()` 通过 `reserve_next_task_id()` 写入 `.highwatermark`，删除任务后不会复用旧 ID；高水位文件的创建受 `.highwatermark.lock` 保护，见 `crates/claude-code-rs/src/tools/tasks.rs:351-364`、`crates/claude-code-rs/src/tools/tasks.rs:809-823`、`crates/claude-code-rs/src/tools/tasks.rs:981-1022`。
- V2 任务记录已持久化 `owner`，`TaskUpdate(status="in_progress")` 会走 `claim_task()`：先获取任务列表 `.lock`，再重读磁盘最新任务并检查任务是否存在、是否已完成、是否被其他 owner 认领、依赖是否未完成，以及 `check_agent_busy` / `checkAgentBusy` 下同 owner 是否已有其它未完成任务；锁不可用时返回 `lock_unavailable` 结构化原因。
- teammate 正常 shutdown、runner error、`team kill` 和 `team delete` 会调用 `unassign_teammate_tasks()`，把该 teammate id/name 持有的非 terminal 任务重置为 `pending` 并清空 `owner`；completed / failed / cancelled / interrupted / stopped 等 terminal 任务保持归属不变。
- `TaskUpdate` 在完成态时会发 `TaskCompleted` hook，`TaskCreate` 会发 `TaskCreated` hook，见 `crates/claude-code-rs/src/tools/tasks.rs:2040-2056`、`crates/claude-code-rs/src/tools/tasks.rs:1846-1858`。
- `TaskOutput` 支持阻塞等待、超时和 abort signal，见 `crates/claude-code-rs/src/tools/tasks.rs:2202-2442`。
- 状态：已实现。
- 结论：V1 `TodoWrite` 入口已经补齐，V2 任务体系也已落地；依赖字段已有 Bun 兼容别名与反向输出，ID 分配、高水位语义、任务列表级文件锁、owner claim / agent-busy 检查、task-list-id 解析、按列表存储隔离、`activeForm`、通用 `metadata`、核心更新语义和 teammate 退出后的 owner 释放已对齐到 Bun 文档主路径。

### `网络工具差异`

- Bun 的 WebSearch 依赖 Anthropic `web_search_20250305` server tool；Rust 的 WebSearch 走本地 provider 抽象，支持 Tavily / Brave，见 `crates/claude-code-rs/src/tools/web_search/tool.rs:12-16`、`crates/claude-code-rs/src/tools/web_search/tool.rs:143-176`。
- Rust 的 WebSearch 还支持 `allowed_domains` / `blocked_domains`、缓存和结果格式化，见 `crates/claude-code-rs/src/tools/web_search/tool.rs:68-73`、`crates/claude-code-rs/src/tools/web_search/tool.rs:166-215`。
- Bun 的 WebFetch 文档强调网页抓取与内容提取；Rust 的 WebFetch 在此基础上加了 sandbox 网络策略检查、URL 归一化、同源重定向限制、响应缓存和二进制拒绝，见 `crates/claude-code-rs/src/tools/web_fetch.rs:199-326`、`crates/claude-code-rs/src/tools/web_fetch.rs:484-723`。
- 状态：已实现。
- 结论：网络工具在 Rust 里不是照搬 Bun 的后端调用方式，而是换成了本地 provider + sandbox policy 的实现路径；能力已具备，但实现细节不同。provider/runtime 对照已收口到 `architecture/web-tools-provider-diff.md`。

## 已实现汇总

- 工具系统已经具备统一 trait、统一 registry 和插件扩展入口。
- 文件操作已经覆盖 Read、Edit、Write，并带有安全写入和 hook。
- 搜索导航已经覆盖 Glob、Grep、ToolSearch、LSP。
- Shell 执行已经覆盖 Bash，并附带 PowerShell、Repl、Sleep。
- 网络工具已经覆盖 WebSearch 和 WebFetch。
- 任务管理已经覆盖 `TodoWrite` V1 兼容入口、V2 任务链路、task-list-id 解析与存储隔离、递增 ID / 高水位、任务列表级文件锁、owner claim / agent-busy 检查、`activeForm` / `metadata` schema、核心更新语义、teammate 退出 owner 释放和输出留存。

## 未实现 / 部分实现 / 待确认

### 未实现

- 未发现需要在本文中单独标成“未实现”的核心工具族群。

### 部分实现

- 当前未发现需要在本文中继续标成“部分实现”的核心工具族群。

### 待确认

- 当前未发现需要在本文中继续标成“待确认”的核心工具差异。

### 故意裁剪

- 当前未发现可以确认的“故意裁剪”项。

## 后续动作

- 当前工具实现地图中没有剩余的工具族群级后续动作；如产品要求 Anthropic server-tool WebSearch parity，应另开 provider 决策项。

## 实现推进记录

- 2026-05-06：Phase 0 已完成基线锁定。新增 `docs/archive/tools-phase0-baseline-2026-05-06.md` 记录决策；`crates/claude-code-rs/src/tools/tasks.rs` 增加 3 个 `#[ignore]` 缺口测试，分别锁定跨 store claim 竞争、Tasks V2 `activeForm` / `metadata` schema parity、task-list-id 与 teammate unassign 后续集成点。验证：`cargo test -p claude-code-rs tools::tasks` 通过，44 passed / 3 ignored。
- 2026-05-06：Phase 1 已完成 task-list-id 与存储边界。`TaskCreate` / `TaskGet` / `TaskUpdate` / `TaskList` / `TaskStop` / `TaskOutput` 现在按 `CC_RUST_TASK_LIST_ID`、兼容 env、in-process teammate team、AppState team、legacy team env、session id、默认 `tasklist` 的优先级选择 store；默认列表落在 `$CC_RUST_HOME/tasks/tasklist/`，并会非破坏性复制旧 flat 目录任务。新增 `docs/archive/tools-phase1-task-list-storage-2026-05-06.md`。验证：`cargo test -p claude-code-rs tools::tasks` 通过，49 passed / 3 ignored。
- 2026-05-06：Phase 2 已完成任务列表级文件锁与原子 claim。新增 `.lock` 任务列表锁；create/update/stop/delete/claim 在写路径上持锁，claim 持锁后 live refresh 磁盘任务并原子检查 owner、terminal、blocked dependency、agent busy；list/get 使用 live refresh 避免 stale store；delete 会清理其它任务对被删任务的依赖引用。新增 `docs/archive/tools-phase2-task-list-lock-2026-05-06.md`。验证：`cargo test -p claude-code-rs tools::tasks` 通过，54 passed / 2 ignored；`rustfmt --edition 2021 --check crates/claude-code-rs/src/tools/tasks.rs` 通过。
- 2026-05-06：Phase 3 已完成 Tasks V2 schema 与更新语义。`TaskEntry` / 持久化 schema 增加 `activeForm` 和通用 `metadata`；`TaskCreate` 支持写入二者；`TaskUpdate` 支持 subject、description、activeForm、owner、metadata merge/null-delete、`addBlocks`、`addBlockedBy` 和 `deleted`，并继续复用 Phase 2 的任务列表锁。新增 `docs/archive/tools-phase3-task-v2-schema-2026-05-06.md`。验证：`cargo test -p claude-code-rs tools::tasks` 通过，57 passed / 1 ignored；`rustfmt --edition 2021 --check crates/claude-code-rs/src/tools/tasks.rs` 通过。
- 2026-05-06：Phase 4 已完成 teammate 退出 owner 释放。新增 `unassign_teammate_tasks()`，在 teammate shutdown、runner error、`team kill` 和 `team delete` 路径释放该 teammate id/name 持有的非 terminal 任务，重置为 `pending` 且清空 `owner`，并返回包含任务 ID / subject 的通知；terminal 任务保持归属不变。新增 `docs/archive/tools-phase4-teammate-unassign-2026-05-06.md`。验证：`cargo test -p claude-code-rs tools::tasks` 通过，58 passed；`cargo test -p claude-code-rs team_cmd` 通过，7 passed；`cargo test -p claude-code-rs teams::in_process` 通过，8 passed；`rustfmt --edition 2021 --check crates/claude-code-rs/src/tools/tasks.rs crates/claude-code-rs/src/teams/runner.rs crates/claude-code-rs/src/commands/team_cmd.rs` 通过。
- 2026-05-06：Phase 5 已完成网络工具 provider 差异文档收口。新增 `architecture/web-tools-provider-diff.md` 和 `docs/archive/tools-phase5-web-provider-diff-2026-05-06.md`，明确 Bun WebSearch 依赖 Anthropic `web_search_20250305` server tool，而 cc-rust WebSearch 依赖 Tavily / Brave 本地 provider；WebFetch 保持工具能力但走本地 sandbox policy、URL 归一化、同源重定向限制、缓存和二进制拒绝。验证：`git diff --check -- architecture/tools-implementation-map.md architecture/web-tools-provider-diff.md docs/archive/tools-phase5-web-provider-diff-2026-05-06.md` 通过。
