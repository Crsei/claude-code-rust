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
| `task-management.mdx` | TodoWrite V1、Tasks V2、依赖和输出管理 | 任务管理 | 部分实现 |

## 实现映射表

| 工具族群 | Bun 主题 | cc-rust 入口 | 状态 | 说明 |
| --- | --- | --- | --- | --- |
| 工具系统 | 工具抽象、注册、schema、权限、结果 | `crates/cc-engine/src/types/tool.rs:192-269`，`crates/claude-code-rs/src/tools/registry.rs:41-83` | 已实现 | `Tool` trait 已提供 `name`、`input_json_schema`、`validate_input`、`check_permissions`、`call`、`prompt`、`max_result_size_chars` 等核心能力；registry 先聚合 `fs::tools()` 和 `exec::tools()`，再追加单工具和插件工具。 |
| 文件操作 | Read / Edit / Write | `crates/claude-code-rs/src/tools/fs/mod.rs:14-31`，`crates/claude-code-rs/src/tools/fs/file_read.rs:706-889`，`crates/claude-code-rs/src/tools/fs/file_edit.rs:329-673`，`crates/claude-code-rs/src/tools/fs/file_write.rs:41-219`，`crates/claude-code-rs/src/tools/fs/safe_write.rs:50-132` | 已实现 | Read 支持文本、图片、PDF、Notebook；Edit 强制先读后写并校验未被外部修改；Write 通过安全写入和 `FileChanged` hook 完成原子替换。 |
| 搜索导航 | Glob / Grep / ToolSearch / LSP | `crates/claude-code-rs/src/tools/fs/glob_tool.rs:13-202`，`crates/claude-code-rs/src/tools/fs/grep.rs:1-218`，`crates/claude-code-rs/src/tools/tool_search.rs:504-627`，`crates/claude-code-rs/src/tools/lsp.rs:1-435` | 已实现 | Glob、Grep、ToolSearch、LSP 都已接入 registry。Rust 额外提供 LSP code intelligence，属于 Bun 文档未单列的扩展能力。 |
| Shell 执行 | BashTool | `crates/claude-code-rs/src/tools/exec/mod.rs:14-30`，`crates/claude-code-rs/src/tools/exec/bash.rs:134-668` | 已实现 | Bash 具备命令解析、危险命令检测、sandbox 预检、超时、进程组控制和实时输出流。Rust 还额外暴露了 PowerShell、Repl、Sleep。 |
| 任务管理 | TodoWrite V1、Tasks V2 | `crates/claude-code-rs/src/tools/tasks.rs:3-9`，`crates/claude-code-rs/src/tools/tasks.rs:1123-1867`，`crates/claude-code-rs/src/tools/registry.rs:19-24,62-67` | 部分实现 | Rust 只实现了 V2 任务体系：`TaskCreate`、`TaskGet`、`TaskUpdate`、`TaskList`、`TaskStop`、`TaskOutput`。`TodoWrite` 没有独立工具入口；任务 ID 采用 UUID，而不是 Bun 文档里的简单递增编号。 |
| 网络工具差异 | WebSearch / WebFetch | `crates/claude-code-rs/src/tools/web_search/tool.rs:1-258`，`crates/claude-code-rs/src/tools/web_fetch.rs:1-723`，`crates/claude-code-rs/src/tools/registry.rs:22-24,56-57` | 已实现 | Rust 端已提供 WebSearch 和 WebFetch，但实现路径不同于 Bun：WebSearch 走 Tavily / Brave provider，WebFetch 走 sandbox 网络策略、URL 归一化、同源重定向限制和内存缓存。 |

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
- Glob 的核心实现位于 `crates/claude-code-rs/src/tools/fs/glob_tool.rs:13-202`，但它当前是按路径做稳定排序，而不是 Bun 文档里强调的按修改时间排序，见 `crates/claude-code-rs/src/tools/fs/glob_tool.rs:161-202`。
- Grep 先尝试外部 `rg`，失败后回退到内部 walker + regex，见 `crates/claude-code-rs/src/tools/fs/grep.rs:1-218`。
- ToolSearch 支持自然语言查询、`select:<tool-name>` 精确选择、来源过滤和 schema hydration，见 `crates/claude-code-rs/src/tools/tool_search.rs:504-627`、`crates/claude-code-rs/src/tools/tool_search.rs:763-781`。
- LSP 作为 Rust 额外扩展，提供 definition、reference、hover、symbol、completion、diagnostics 等能力，见 `crates/claude-code-rs/src/tools/lsp.rs:1-11`、`crates/claude-code-rs/src/tools/lsp.rs:236-435`。
- 状态：已实现。
- 结论：搜索与导航的主链路已经落地；若要严格对齐 Bun 文档，唯一明显差异是 Glob 的结果排序策略和 Rust 额外提供的 LSP 扩展。

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
- Rust 只提供任务 V2 体系，没有单独的 TodoWrite 工具入口，见 `crates/claude-code-rs/src/tools/registry.rs:19-24,62-67`。
- 任务存储默认落在 `~/.cc-rust/tasks` 或 `$CC_RUST_HOME/tasks`，见 `crates/claude-code-rs/src/tools/tasks.rs:3-9`。
- `TaskStore` / `TaskRepository` 负责持久化、恢复、输出保留和 schema 迁移，见 `crates/claude-code-rs/src/tools/tasks.rs:68-222`、`crates/claude-code-rs/src/tools/tasks.rs:501-886`。
- `TaskCreate`、`TaskGet`、`TaskUpdate`、`TaskList`、`TaskStop`、`TaskOutput` 的实现分别见 `crates/claude-code-rs/src/tools/tasks.rs:1123-1867`。
- 任务 ID 使用 `uuid::Uuid::new_v4()` 生成，而不是 Bun 文档里的递增编号，见 `crates/claude-code-rs/src/tools/tasks.rs:241-248`。
- `TaskUpdate` 在完成态时会发 `TaskCompleted` hook，`TaskCreate` 会发 `TaskCreated` hook，见 `crates/claude-code-rs/src/tools/tasks.rs:1318-1334`、`crates/claude-code-rs/src/tools/tasks.rs:1479-1499`。
- `TaskOutput` 支持阻塞等待、超时和 abort signal，见 `crates/claude-code-rs/src/tools/tasks.rs:1688-1867`。
- 状态：部分实现。
- 结论：V2 任务体系已经落地，但 Bun 文档里的 V1 `TodoWrite` 没有对应独立实现，因此不能把这章写成完全对齐。

### `网络工具差异`

- Bun 的 WebSearch 依赖 Anthropic `web_search_20250305` server tool；Rust 的 WebSearch 走本地 provider 抽象，支持 Tavily / Brave，见 `crates/claude-code-rs/src/tools/web_search/tool.rs:12-16`、`crates/claude-code-rs/src/tools/web_search/tool.rs:143-176`。
- Rust 的 WebSearch 还支持 `allowed_domains` / `blocked_domains`、缓存和结果格式化，见 `crates/claude-code-rs/src/tools/web_search/tool.rs:68-73`、`crates/claude-code-rs/src/tools/web_search/tool.rs:166-215`。
- Bun 的 WebFetch 文档强调网页抓取与内容提取；Rust 的 WebFetch 在此基础上加了 sandbox 网络策略检查、URL 归一化、同源重定向限制、响应缓存和二进制拒绝，见 `crates/claude-code-rs/src/tools/web_fetch.rs:199-326`、`crates/claude-code-rs/src/tools/web_fetch.rs:484-723`。
- 状态：已实现。
- 结论：网络工具在 Rust 里不是照搬 Bun 的后端调用方式，而是换成了本地 provider + sandbox policy 的实现路径；能力已具备，但实现细节不同。

## 已实现汇总

- 工具系统已经具备统一 trait、统一 registry 和插件扩展入口。
- 文件操作已经覆盖 Read、Edit、Write，并带有安全写入和 hook。
- 搜索导航已经覆盖 Glob、Grep、ToolSearch、LSP。
- Shell 执行已经覆盖 Bash，并附带 PowerShell、Repl、Sleep。
- 网络工具已经覆盖 WebSearch 和 WebFetch。
- 任务管理已经覆盖 V2 任务链路和输出留存。

## 未实现 / 部分实现 / 待确认

### 未实现

- 未发现需要在本文中单独标成“未实现”的核心工具族群。

### 部分实现

- 任务管理只实现了 V2 Tasks，Bun 文档里的 V1 `TodoWrite` 没有独立工具入口，见 `crates/claude-code-rs/src/tools/registry.rs:19-24,62-67` 与 `crates/claude-code-rs/src/tools/tasks.rs:1123-1867`。

### 待确认

- 若后续要做 Bun 级别逐项对齐，需要再确认 `Glob` 的排序策略是否要从当前“按路径稳定排序”改成文档里描述的“按修改时间排序”，见 `crates/claude-code-rs/src/tools/fs/glob_tool.rs:161-202`。

### 故意裁剪

- 当前未发现可以确认的“故意裁剪”项。

## 后续动作

1. 如果要继续做文档级对齐，优先补一张 `Glob` 排序差异表，把 Bun 的修改时间排序和 Rust 的路径排序分开写清楚。
2. 如果要补齐任务管理文档，建议先决定是补一个 `TodoWrite` 兼容层，还是在文档里明确声明只保留 V2 Tasks。
3. 如果要继续细化网络工具差异，建议把 Bun 的 Anthropic WebSearch 路径和 Rust 的 Tavily / Brave 路径单独拆成对照表。
