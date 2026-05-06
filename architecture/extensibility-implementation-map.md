# Extensibility Implementation Map

## 范围

本文档只覆盖 `docs/bun-docs-documentation-plan.md` 中的 Extensibility 章节，对照 Bun 上游的以下五份文档，梳理 cc-rust 当前实现状态：

- `custom-agents.mdx`
- `hooks.mdx`
- `mcp-configuration.mdx`
- `mcp-protocol.mdx`
- `skills.mdx`

本文档只做实现映射，不展开源码设计细节；所有结论都基于只读检查到的文件路径与行号。状态只使用：`已实现`、`部分实现`、`未实现`、`待确认`、`故意裁剪`。

## 上游文档清单

| 上游文档 | 主要主题 | cc-rust 对应面 |
| --- | --- | --- |
| `custom-agents.mdx` | 自定义 agents 定义、编辑、运行 | `ipc/agent_settings.rs`、`engine/agent/*`、`ipc/builtin_agents.rs` |
| `hooks.mdx` | 生命周期 hooks、事件 hooks、权限联动 | `cc-config/src/settings.rs`、`tools/hooks/*`、`engine/lifecycle/deps.rs` |
| `mcp-configuration.mdx` | MCP 服务配置、发现、编辑、启停 | `cc-mcp/*`、`commands/mcp_cmd.rs`、`ipc/subsystem_handlers.rs` |
| `mcp-protocol.mdx` | MCP JSON-RPC 协议、握手、工具/资源 RPC | `cc-mcp/src/lib.rs`、`cc-mcp/src/client.rs` |
| `skills.mdx` | skills frontmatter、加载、注册、调用、命令入口 | `cc-skills/*`、`tools/skill.rs`、`commands/skills_cmd.rs` |

## 实现映射表

| 文档 | 配置层 | 运行时层 | 安全层 | 总评 |
| --- | --- | --- | --- | --- |
| `custom-agents.mdx` | 已实现 | 已实现 | 部分实现 | 自定义 agent 定义链路已接通，安全边界主要靠通用工具过滤与可编辑范围限制 |
| `hooks.mdx` | 已实现 | 已实现 | 已实现 | hooks 配置、执行与权限联动已经形成闭环 |
| `mcp-configuration.mdx` | 部分实现 | 部分实现 | 部分实现 | MCP 配置与管理可用；stdio、本地 loopback HTTP SSE、manager 级 connect/disconnect/reconnect、短指数退避重试和 channel notification 事件路由可运行，但远程 HTTPS / OAuth / 完整传输矩阵仍不完整 |
| `mcp-protocol.mdx` | 部分实现 | 部分实现 | 部分实现 | JSON-RPC 协议骨架、stdio 通道、本地 loopback SSE endpoint / POST 通道和 `notifications/claude/channel` 路由可用；远程 transport / auth 覆盖仍不完整 |
| `skills.mdx` | 已实现 | 已实现 | 已实现 | skills 的加载、调用、fork 执行与命令入口已形成完整链路 |

## 逐文档分析

### 1. `custom-agents.mdx`

#### 配置层

`ipc/agent_settings.rs` 已经支持自定义 agent 的解析、渲染和持久化：文件头注释直接列出支持的字段，包含 `name`、`description`、`tools`、`disallowedTools`、`model`、`permissionMode`、`memory`、`maxTurns`、`effort`、`background`、`isolation`、`initialPrompt` 等；解析与写回逻辑分别位于 `crates/claude-code-rs/src/ipc/agent_settings.rs:9-11`、`crates/claude-code-rs/src/ipc/agent_settings.rs:176-227`、`crates/claude-code-rs/src/ipc/agent_settings.rs:258-335`。

`list_all_agents()` 会把内置 agent 与目录中的用户/项目定义合并到同一列表，入口在 `crates/claude-code-rs/src/ipc/agent_settings.rs:134-169`。UI 层的 agent 类型也保留了 `skills`、`hooks`、`plugin` 等字段，见 `crates/claude-code-rs/src/ui/agents/types.rs:67-85`。

#### 运行时层

运行时已经可以按 `subagent_type` 选择 agent 定义并启动子 agent。`AgentTool` 在 `crates/claude-code-rs/src/engine/agent/tool_impl.rs:98-117` 读取 `subagent_type` 后，通过 `active_agent_definition()` 找到当前定义，再把定义中的颜色、模型等信息带入启动参数；背景执行与同步执行分支分别在 `crates/claude-code-rs/src/engine/agent/tool_impl.rs:157-234`。

真正的定义查找与工具裁剪逻辑在 `crates/claude-code-rs/src/engine/agent/mod.rs:299-307` 与 `crates/claude-code-rs/src/engine/agent/mod.rs:320-344`：先定位当前 agent 定义，再按 `disallowedTools` 和显式 `tools` 列表过滤子 agent 可见工具。内置 agent 也已经作为独立来源存在，见 `crates/claude-code-rs/src/ipc/builtin_agents.rs:1-184`。

#### 安全层

安全边界主要来自定义内的 `disallowedTools`、`permissionMode`、`isolation` 以及运行时工具过滤，而不是一套独立的 agent 安全子系统。相关证据在 `crates/claude-code-rs/src/ipc/agent_settings.rs:196-227`、`crates/claude-code-rs/src/engine/agent/mod.rs:320-344`、`crates/claude-code-rs/src/engine/agent/tool_impl.rs:157-234`。

结论：`custom-agents.mdx` 在配置和运行时上都已经可用，但安全策略更多是沿用通用工具/隔离机制，因此这里记为 `部分实现`。

### 2. `hooks.mdx`

#### 配置层

hooks 的持久化形态已经落在 `cc-config` 的 `RawSettings` 里：`hooks: Option<HashMap<String, Value>>` 定义在 `crates/cc-config/src/settings.rs:392-410`，合并逻辑在 `crates/cc-config/src/settings.rs:460-501`。`/hooks` 命令提供了只读浏览与定位入口，见 `crates/claude-code-rs/src/commands/hooks_cmd.rs:1-21`、`crates/claude-code-rs/src/commands/hooks_cmd.rs:103-185`、`crates/claude-code-rs/src/commands/hooks_cmd.rs:293-309`。

`cc-types::hooks` 已定义 hook 事件与运行器接口，见 `crates/cc-types/src/hooks.rs:1-8`、`crates/cc-types/src/hooks.rs:60-70`、`crates/cc-types/src/hooks.rs:131-188`。

#### 运行时层

`ShellHookRunner` 是实际执行层，位于 `crates/claude-code-rs/src/tools/hooks/mod.rs:84-154`。预工具、后工具、失败、Stop 与通用事件钩子分别实现于 `crates/claude-code-rs/src/tools/hooks/pre_tool.rs:22-108` 和 `crates/claude-code-rs/src/tools/hooks/post_tool.rs:18-274`。

生命周期里已经接上 hooks：启动时触发 `SessionStart`，见 `crates/claude-code-rs/src/main.rs:727-746`；工具执行主链路在 `crates/claude-code-rs/src/engine/lifecycle/deps.rs:488-1012`，其中包括 `PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`PermissionRequest`、`PermissionDenied` 等事件。

#### 安全层

hooks 的安全联动是当前实现里最完整的一块：`cc-permissions/src/decision.rs:6-25` 描述了 hook deny、deny rule、ask rule、allow rule 的优先级；`crates/cc-permissions/src/decision.rs:235-373` 实现了 hook 决策在权限判定中的插入点。`PreToolUse` 钩子还能返回 `permission_decision` 与 `updated_input`，见 `crates/cc-types/src/hooks.rs:89-125` 和 `crates/claude-code-rs/src/tools/hooks/pre_tool.rs:22-108`。

结论：`hooks.mdx` 对应的配置、执行、权限三层都已经接通，状态记为 `已实现`。

### 3. `mcp-configuration.mdx`

#### 配置层

MCP 的配置形态已经进入设置层与发现层。`McpServerConfig` 定义在 `crates/cc-mcp/src/lib.rs:133-164`，包含 `transport`、`command`、`args`、`url`、`headers`、`env`、`browser_mcp`、`disabled` 等字段。发现逻辑在 `crates/cc-mcp/src/discovery.rs:19-35`、`crates/cc-mcp/src/discovery.rs:120-211`，能从用户、项目、插件和 IDE 作用域收集配置。

管理命令也已存在：`/mcp` 支持 `list`、`status`、`add`、`edit`、`remove`、`approve`、`reject`、`connect`、`disconnect`、`reconnect`，见 `crates/claude-code-rs/src/commands/mcp_cmd.rs:1-14`、`crates/claude-code-rs/src/commands/mcp_cmd.rs:63-72`。写回设置与作用域映射在 `crates/claude-code-rs/src/ipc/subsystem_handlers.rs:708-757`、`crates/claude-code-rs/src/ipc/subsystem_handlers.rs:806-969`。

#### 运行时层

运行时已经能连接、初始化并管理 MCP 服务：`connect_all()`、`connect_server()`、`reconnect_server()` 与 `disconnect_server()` 位于 `crates/cc-mcp/src/manager.rs`，其中单服务 reconnect 会先断开旧 client，再按新配置连接，失败时不保留过期 client。`connect_server()` 现在会通过 `connect_ready_client_with_retries()` 对连接/初始化失败做 3 次短指数退避重试，退避上限 250ms，见 `crates/cc-mcp/src/manager.rs:94-112` 与 `crates/cc-mcp/src/manager.rs:219-224`。`McpClient::connect()`、`initialize()`、`list_tools()`、`call_tool()`、`list_resources()`、`read_resource()` 分别位于 `crates/cc-mcp/src/client.rs:93-239`、`crates/cc-mcp/src/client.rs:273-401`。

通知层现在会解析并路由 `notifications/claude/channel`：`transport.rs` 在 stdio 与 SSE message 路径中把 channel payload 转成 `McpSubsystemEvent::ChannelNotification`，`ipc/runtime.rs` 再适配为前端已有的 `McpEvent::ChannelNotification`。[`transport.rs`](F:/AIclassmanager/cc/rust/crates/cc-mcp/src/transport.rs#L242), [`lib.rs`](F:/AIclassmanager/cc/rust/crates/cc-mcp/src/lib.rs#L89), [`runtime.rs`](F:/AIclassmanager/cc/rust/crates/claude-code-rs/src/ipc/runtime.rs#L137)

工具包装层也已经把远端 MCP 工具暴露成本地工具，见 `crates/claude-code-rs/src/mcp/tools.rs:1-147`。

#### 安全层

这里的安全主要依赖作用域与显式禁用，而不是更细的策略引擎。`discover_mcp_servers_scoped()` 会保留来源作用域，见 `crates/cc-mcp/src/discovery.rs:143-191`；`/mcp` 只允许编辑可写作用域，`crates/claude-code-rs/src/commands/mcp_cmd.rs:272-306` 与 `crates/claude-code-rs/src/commands/mcp_cmd.rs:415-430` 会拒绝只读来源。运行时还会跳过 `disabled` 的服务，见 `crates/cc-mcp/src/manager.rs:39-43` 和 `crates/claude-code-rs/src/ipc/subsystem_handlers.rs:685-722`。

补充：SSE transport 已有一个无新增依赖的本地 loopback HTTP 运行时：`connect_sse()` 会打开 `http://localhost` / `127.0.0.1` / `::1` 的 SSE stream，读取 `endpoint` 事件，再通过 HTTP POST 发送 JSON-RPC；实现入口在 `crates/cc-mcp/src/client.rs` 与 `crates/cc-mcp/src/transport.rs`。manager 层已有真实 reconnect API 和短指数退避重试，transport 层已把 channel notification 路由到 IPC 事件，但上层 `/mcp reconnect` / IPC 运行时命令仍是排队语义；远程 `https://` SSE、OAuth / interactive auth 和非 SSE 的 HTTP / WS 传输仍未实现。

结论：`mcp-configuration.mdx` 的配置与管理能力可用，SSE 配置安全已有基础保护，但传输与策略覆盖还不完整，因此记为 `部分实现`。

### 4. `mcp-protocol.mdx`

#### 配置层

协议层的骨架已经存在于 `crates/cc-mcp/src/lib.rs:1-37` 与 `crates/cc-mcp/src/lib.rs:175-363`：这里定义了 JSON-RPC 消息、初始化结果、工具/资源返回值，以及 `McpServerConfig` 的协议相关字段。`McpClient` 的说明也明确了协议流程：连接、`initialize`、`tools/list`、`tools/call`、`resources/list`、`resources/read`，见 `crates/cc-mcp/src/client.rs:1-11`。

协议配置的形态声明了 `stdio` 与 `sse`。当前可用路径包括 stdio 以及本地 loopback HTTP SSE：SSE 连接会先校验 URL 与 headers，再读取 SSE `endpoint` 事件，并把 JSON-RPC 请求 POST 到该 endpoint；响应由 SSE `message` 事件回到 pending request，`notifications/claude/channel` 会转成 subsystem channel 事件，见 `crates/cc-mcp/src/client.rs`、`crates/cc-mcp/src/transport.rs` 与 `crates/claude-code-rs/src/ipc/runtime.rs`。远程 HTTPS、OAuth、Streamable HTTP、WebSocket 和 IDE 专用 transport 仍未落地。

#### 运行时层

协议运行时对 stdio 的 JSON-RPC 流程已经打通：`initialize()` 发送 `initialize` 并回 `notifications/initialized`，见 `crates/cc-mcp/src/client.rs:196-232`；工具与资源 RPC 分别在 `crates/cc-mcp/src/client.rs:273-401`。管理层通过 `crates/cc-mcp/src/manager.rs:50-67` 完成初始化后再拉取工具与资源。

#### 安全层

协议层看到了超时、断开清理、SSE 配置安全校验、本地 loopback SSE endpoint / message 分发、channel notification 事件路由，以及 manager 级短指数退避重试。请求超时与 pending 回收在 `crates/cc-mcp/src/client.rs`，断开逻辑在 `crates/cc-mcp/src/client.rs`，SSE URL / header 校验、HTTP GET / POST 与 endpoint 解析在 `crates/cc-mcp/src/client.rs`，SSE event 解析、JSON-RPC response 分发与 channel notification 路由在 `crates/cc-mcp/src/transport.rs`，退避重试在 `crates/cc-mcp/src/manager.rs:94-112`。由于远程 HTTPS/OAuth、自动长线重连与完整传输矩阵没有完成，这一层仍记为 `部分实现`。

结论：`mcp-protocol.mdx` 的核心 JSON-RPC 主路径已经存在，但完整度还不够，所以总体记为 `部分实现`。

### 5. `skills.mdx`

#### 配置层

skills 的数据模型与加载器已经比较完整。`SkillSource`、`SkillContext`、`SkillFrontmatter`、`SkillDefinition` 分别定义在 `crates/cc-skills/src/lib.rs:29-123`；其中 frontmatter 支持 `user_invocable`、`disable_model_invocation`、`context`、`agent`、`allowed_tools`、`dependencies`、`entry_docs` 等字段，见 `crates/cc-skills/src/lib.rs:73-123`。

加载器支持目录扫描、旧命令兼容与前置诊断，见 `crates/cc-skills/src/loader.rs:168-234`、`crates/cc-skills/src/loader.rs:324-393`。内置 skills 也已编译进二进制，见 `crates/cc-skills/src/bundled.rs:5-137`。

#### 运行时层

`/skills` 命令提供列表、详情、诊断和热重载入口，见 `crates/claude-code-rs/src/commands/skills_cmd.rs:18-24`、`crates/claude-code-rs/src/commands/skills_cmd.rs:75-112`。真正调用 skills 的工具是 `crates/claude-code-rs/src/tools/skill.rs:1-286`：它会展开 prompt、检查是否允许模型调用，并按 `SkillContext::Inline` / `SkillContext::Fork` 分流；fork 分支通过 `crates/claude-code-rs/src/tools/skill.rs:209-286` 进入 `engine::agent::fork`。

另外，`/simplify` 已经把 bundled skill 当成单独命令使用，见 `crates/claude-code-rs/src/commands/simplify.rs:152-179`。

#### 安全层

skills 的安全约束已经体现在加载、注册和调用三个阶段。`is_user_invocable()` 与 `is_model_invocable()` 分别在 `crates/cc-skills/src/lib.rs:130-136` 提供调用门禁；注册时采用 first-registration-wins，见 `crates/cc-skills/src/lib.rs:328-366`；调用侧只接受可模型调用的 skill，见 `crates/claude-code-rs/src/tools/skill.rs:114-155`。

fork 执行还会继承 skill 的 `allowed_tools` 和 `model` 配置，见 `crates/claude-code-rs/src/tools/skill.rs:179-286`，这使得 skill 的运行隔离比单纯 prompt 展开更强。

结论：`skills.mdx` 在 cc-rust 里已经形成完整闭环，状态记为 `已实现`。

## 已实现汇总

已实现的能力如下：

- `hooks.mdx`
- `skills.mdx`
- `custom-agents.mdx` 的配置与运行时主路径

这些能力都能在源码里找到明确入口，且能从配置层走到执行层，不只是文档或占位符。

## 未实现 / 部分实现 / 待确认

| 文档 | 当前状态 | 主要原因 |
| --- | --- | --- |
| `custom-agents.mdx` | 部分实现 | 定义、编辑、调用链路已通，但安全边界主要依赖通用工具过滤与隔离，没有看到独立的 agent 安全子系统 |
| `mcp-configuration.mdx` | 部分实现 | 发现、编辑、连接已具备；stdio、本地 loopback HTTP SSE、manager 级 reconnect、短指数退避重试和 channel notification 事件路由可运行，但远程 HTTPS / OAuth / HTTP / WS 等更完整配置矩阵未落地 |
| `mcp-protocol.mdx` | 部分实现 | stdio JSON-RPC 主路径、本地 loopback SSE endpoint / POST 通道、channel notification 路由和 manager 级短退避重试可用；认证、远程 transport runtime 和完整传输覆盖仍未完成 |

`未实现` 与 `故意裁剪` 在这次核查里没有找到可直接落表的明确项。

## 后续动作

1. 如果要继续补齐 Extensibility 章节，优先把 manager 级 reconnect 接到 `/mcp reconnect` / IPC 会话生命周期，再实现远程 HTTPS SSE / OAuth；当前只完成了 stdio、本地 loopback HTTP SSE、channel notification 路由、manager API 与短退避重试主路径。
2. 如果后续发现 custom agents 还要补更细的安全约束，再补一轮 `engine/agent/*` 与 `ipc/agent_settings.rs` 的交叉核查。
3. 其余三项（hooks、skills、MCP 配置）已经可以直接作为文档基线使用。
