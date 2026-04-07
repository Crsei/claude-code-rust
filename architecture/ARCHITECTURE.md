# cc-rust 完整架构文档

> 176 个源文件 · ~49,883 行 Rust 代码 · 28 工具 · 32 命令 · 5 内置技能  
> 16 API Provider · 多 Agent 协调 · MCP/Plugin 生态

---

## 目录

1. [全局架构总览](#1-全局架构总览)
2. [模块依赖 DAG](#2-模块依赖-dag)
3. [生命周期状态机](#3-生命周期状态机)
4. [核心类型系统](#4-核心类型系统)
5. [QueryEngine 与查询循环](#5-queryengine-与查询循环)
6. [工具系统](#6-工具系统)
7. [API 客户端与多 Provider](#7-api-客户端与多-provider)
8. [认证系统](#8-认证系统)
9. [权限系统](#9-权限系统)
10. [技能系统](#10-技能系统)
11. [命令系统](#11-命令系统)
12. [上下文压缩管道](#12-上下文压缩管道)
13. [会话持久化](#13-会话持久化)
14. [IPC 与 Headless 模式](#14-ipc-与-headless-模式)
15. [TUI 渲染层](#15-tui-渲染层)
16. [MCP 服务器集成](#16-mcp-服务器集成)
17. [插件系统](#17-插件系统)
18. [多 Agent Teams](#18-多-agent-teams)
19. [Bootstrap 与进程状态](#19-bootstrap-与进程状态)
20. [服务层](#20-服务层)
21. [工具函数](#21-工具函数)
22. [并发模型与线程安全](#22-并发模型与线程安全)
23. [关键设计模式](#23-关键设计模式)
24. [数据流全景图](#24-数据流全景图)
25. [构建与依赖](#25-构建与依赖)

---

## 1. 全局架构总览

cc-rust 是 Claude Code CLI 的 Rust 精简版实现，保留核心对话引擎 + 工具系统 + 技能系统，移除了 analytics 和 remote 模块。

```
┌─────────────────────────────────────────────────────────┐
│                     用户界面层                            │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ TUI (ratatui) │  │ ink-terminal │  │  Print/JSON   │  │
│  │  + crossterm  │  │  (React 19)  │  │  非交互输出    │  │
│  └──────┬───────┘  └──────┬───────┘  └───────┬───────┘  │
│         │ 直接调用          │ IPC/JSONL         │ Stream   │
├─────────┼─────────────────┼──────────────────┼──────────┤
│         └────────────┐    │    ┌──────────────┘          │
│                      ▼    ▼    ▼                         │
│              ┌────────────────────┐                      │
│              │   QueryEngine      │                      │
│              │  (会话生命周期管理)  │                      │
│              └────────┬───────────┘                      │
│                       │                                  │
│         ┌─────────────┼─────────────┐                    │
│         ▼             ▼             ▼                    │
│  ┌────────────┐ ┌──────────┐ ┌──────────┐               │
│  │ query loop │ │  tools/  │ │ compact/ │               │
│  │(流式查询)   │ │ (28工具)  │ │(上下文压缩)│              │
│  └─────┬──────┘ └─────┬────┘ └──────────┘               │
│        │              │                                  │
│        ▼              ▼                                  │
│  ┌──────────┐  ┌──────────────┐                          │
│  │ api/     │  │ permissions/ │                          │
│  │(多Provider)│ │  (权限控制)   │                          │
│  └──────────┘  └──────────────┘                          │
├──────────────────────────────────────────────────────────┤
│                     基础设施层                            │
│  bootstrap/ · config/ · auth/ · session/ · utils/       │
│  skills/ · commands/ · mcp/ · plugins/ · teams/         │
└──────────────────────────────────────────────────────────┘
```

### 运行模式

| 模式 | 入口 | 说明 |
|------|------|------|
| 交互式 TUI | `cargo run` | ratatui + crossterm 终端 UI |
| ink-terminal | `run.sh` | React 19 前端 + Rust `--headless` 后端 |
| Print 模式 | `--print` / `-p` | 提取最终文本输出 |
| JSON 模式 | `--output-format json` | JSONL 流式 SDK 消息 |
| Headless | `--headless` | IPC 模式，等待外部 UI 连接 |

---

## 2. 模块依赖 DAG

```
main.rs
├── bootstrap/         ← 叶节点，不依赖任何上层模块
│   ├── state.rs       ProcessState 全局单例
│   ├── ids.rs         SessionId 品牌类型
│   └── signal.rs      Signal<T> 响应式原语
│
├── types/             ← 核心类型，被所有模块引用
│   ├── message.rs     Message 联合类型 + ContentBlock
│   ├── tool.rs        Tool trait + ToolUseContext
│   ├── state.rs       QueryLoopState + BudgetTracker
│   ├── config.rs      QueryEngineConfig + QueryParams
│   ├── app_state.rs   AppState + SettingsJson
│   └── transitions.rs Terminal + Continue 枚举
│
├── config/            设置加载与合并
├── auth/              认证链 (env → keychain)
├── permissions/       权限决策引擎
│
├── engine/            QueryEngine 生命周期
│   ├── lifecycle.rs   主控：submit_message() → Stream<SdkMessage>
│   ├── system_prompt.rs  系统提示词构建
│   ├── input_processing.rs  斜杠命令/用户输入处理
│   ├── result.rs      结果提取与成功判定
│   ├── sdk_types.rs   SdkMessage 输出类型
│   └── prompt_sections.rs  提示词分段缓存
│
├── query/             异步流式查询循环
│   ├── loop_impl.rs   8 步状态机 (核心)
│   ├── deps.rs        QueryDeps trait (DI)
│   ├── token_budget.rs  Token 预算管理
│   └── stop_hooks.rs  停止钩子
│
├── tools/             28 个工具实现
│   ├── registry.rs    工具注册表
│   ├── execution.rs   工具执行引擎
│   ├── hooks.rs       Hook 系统
│   ├── orchestration.rs  工具编排
│   └── (24 个工具文件)
│
├── compact/           上下文压缩 5 阶段管道
├── commands/          32 个斜杠命令
├── skills/            技能系统 (内置 + 用户自定义)
├── session/           会话持久化 + 导出 + 审计
├── api/               多 Provider API 客户端
├── mcp/               MCP 服务器集成
├── plugins/           插件系统
├── teams/             多 Agent 协调
├── services/          后台服务 (摘要/记忆/建议/LSP)
├── lsp_service/       LSP 服务器管理
├── ipc/               IPC 协议 + Headless 模式
├── ui/                TUI 渲染 (ratatui)
├── utils/             工具函数
└── shutdown.rs        优雅关闭
```

**依赖规则：** `bootstrap` 是纯叶节点 → `types` → `config/auth/permissions` → `engine/query/tools` → `ui/ipc`

---

## 3. 生命周期状态机

### Phase A: 快速路径检测

```
main() → tokio::Runtime::block_on
  ├─ 加载 .env (优先级: ~/.cc-rust/.env → exe-dir → cwd)
  ├─ 解析 CLI 参数 (clap)
  ├─ --version → 打印版本 → 退出
  └─ --dump-system-prompt → 构建并打印提示词 → 退出
```

### Phase B: 完整初始化

```
run_full_init(cli)
  ├─ B.1 加载配置: GlobalConfig + ProjectConfig → MergedConfig
  ├─ B.2 解析权限模式: CLI > config > Default
  ├─ B.3 注册工具: get_all_tools() → 28 工具
  ├─ B.3c MCP 发现: discover_mcp_servers() → McpManager
  ├─ B.4 创建 AppState (Arc<RwLock<AppState>>)
  ├─ B.5 --init-only 快速退出
  ├─ B.6 构建 QueryEngineConfig (不可变快照)
  ├─ B.7 创建 QueryEngine
  ├─ B.7.1 初始化 ProcessState 全局单例
  ├─ B.8 --resume 恢复会话
  ├─ B.9 确定输出模式 (TUI / Print / JSON / Headless)
  └─ B.10 进入主循环
```

### REPL 循环

```
for each user_input:
    stream = engine.submit_message(prompt, QuerySource::ReplMainThread)
    for msg in stream:
        ui.render(msg)
        if msg.is_result(): break
```

### Phase I: 优雅关闭

```
graceful_shutdown(engine)
  ├─ 1. engine.abort() → 传播 abort_signal 到所有运行中工具
  ├─ 2. transcript::flush_transcript() → 写入审计日志
  ├─ 3. session::storage::save_session() → 持久化消息
  ├─ 4. 重置终端 (显示光标, 关闭 raw mode)
  └─ 5. 打印使用量摘要 (tokens, cost)
```

### CLI 参数 (Cli struct)

```rust
pub struct Cli {
    version: bool,                          // --version
    dump_system_prompt: bool,              // --dump-system-prompt
    init_only: bool,                       // --init-only
    resume: bool,                          // --resume
    continue_session: Option<String>,      // --continue <id>
    cwd: Option<String>,                   // -C/--cwd
    model: Option<String>,                 // -m/--model
    system_prompt: Option<String>,         // --system-prompt
    append_system_prompt: Option<String>,  // --append-system-prompt
    permission_mode: Option<String>,       // --permission-mode
    print: bool,                           // -p/--print
    output_format: Option<String>,         // --output-format (json/text)
    headless: bool,                        // --headless
    max_turns: Option<usize>,              // --max-turns
    max_budget: Option<f64>,               // --max-budget (USD)
    verbose: bool,                         // -v/--verbose
    prompt: Vec<String>,                   // 位置参数 (内联提示词)
}
```

---

## 4. 核心类型系统

### 4.1 消息类型 (`types/message.rs`)

```rust
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    System(SystemMessage),
    Progress(ProgressMessage),
    Attachment(AttachmentMessage),
}

pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: ToolResultContent, is_error: bool },
    Thinking { thinking: String, signature: Option<String> },
    RedactedThinking { data: String },
    Image { source: ImageSource },
}
```

**消息变体详情：**

| 变体 | 关键字段 | 用途 |
|------|---------|------|
| `UserMessage` | uuid, content, is_meta, source_tool_assistant_uuid | 用户输入 (is_meta=true 表示系统注入) |
| `AssistantMessage` | content: Vec<ContentBlock>, usage, stop_reason, cost_usd | 模型输出 + 用量追踪 |
| `SystemMessage` | subtype: SystemSubtype | 压缩边界、API 错误、信息提示 |
| `ProgressMessage` | tool_use_id, data: Value | 工具实时进度 |
| `AttachmentMessage` | attachment: Attachment | 文件变更、排队命令、技能发现 |

**SystemSubtype：**
- `CompactBoundary { compact_metadata }` — 压缩边界标记
- `ApiError { retry_attempt, max_retries, retry_in_ms, error }` — API 重试
- `Informational { level: InfoLevel }` — 信息提示
- `LocalCommand { content }` — 本地命令结果
- `Warning` — 警告

**Attachment：**
- `EditedTextFile { path }` — 文件编辑记录
- `QueuedCommand { prompt, source_uuid }` — 排队命令
- `MaxTurnsReached { max_turns, turn_count }` — 达到最大轮数
- `StructuredOutput { data }` — 结构化输出
- `SkillDiscovery { skills }` — 技能发现
- `NestedMemory { path, content }` — 嵌套记忆

### 4.2 Tool trait (`types/tool.rs`)

```rust
pub trait Tool: Send + Sync {
    // 身份标识
    fn name(&self) -> &str;
    async fn description(&self, input: &Value) -> String;
    fn input_json_schema(&self) -> Value;
    
    // 内省
    fn is_enabled(&self) -> bool { true }
    fn is_concurrency_safe(&self, _input: &Value) -> bool { false }
    fn is_read_only(&self, _input: &Value) -> bool { false }
    fn is_destructive(&self, _input: &Value) -> bool { false }
    
    // 生命周期钩子
    async fn validate_input(&self, input: &Value, ctx: &ToolUseContext) -> ValidationResult;
    async fn check_permissions(&self, input: &Value, ctx: &ToolUseContext) -> PermissionResult;
    
    // 核心执行
    async fn call(
        &self, input: Value, ctx: &ToolUseContext,
        parent_message: &AssistantMessage,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult>;
    
    // 配置
    async fn prompt(&self) -> String;
    fn user_facing_name(&self, input: Option<&Value>) -> String;
    fn max_result_size_chars(&self) -> usize { 100_000 }
    fn get_path(&self, _input: &Value) -> Option<String> { None }
    fn interrupt_behavior(&self) -> InterruptBehavior { InterruptBehavior::Block }
}
```

### 4.3 ToolUseContext

```rust
pub struct ToolUseContext {
    pub options: ToolUseOptions,                    // 不可变配置
    pub abort_signal: watch::Receiver<bool>,       // 取消信号
    pub read_file_state: FileStateCache,           // 文件追踪 (LRU)
    pub get_app_state: Arc<dyn Fn() -> AppState>,  // 闭包读取状态
    pub set_app_state: Arc<dyn Fn(Box<dyn FnOnce(AppState) -> AppState>)>,
    pub messages: Vec<Message>,                    // 完整对话历史
    pub agent_id: Option<String>,                  // 子 Agent 上下文
    pub agent_type: Option<String>,
    pub query_tracking: Option<QueryChainTracking>, // 递归深度追踪
}
```

### 4.4 权限类型

```rust
pub enum PermissionMode { Default, Auto, Bypass, Plan }

pub enum PermissionResult {
    Allow { updated_input: Value },
    Deny { message: String },
    Ask { message: String },
}

pub struct ToolPermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: HashMap<String, AdditionalWorkingDirectory>,
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
    pub pre_plan_mode: Option<PermissionMode>,
}
```

### 4.5 配置类型

```rust
pub struct QueryEngineConfig {
    pub cwd: String,
    pub tools: Tools,                          // Vec<Arc<dyn Tool>>
    pub custom_system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub user_specified_model: Option<String>,
    pub max_turns: Option<usize>,
    pub max_budget_usd: Option<f64>,
    pub thinking_config: Option<ThinkingConfig>,
    pub json_schema: Option<Value>,            // 结构化输出
    pub persist_session: bool,
    pub auto_save_session: bool,
    pub agent_context: Option<AgentContext>,
    // ...
}

pub enum ThinkingConfig {
    Disabled,
    Adaptive,
    Enabled { budget_tokens: Option<usize> },
}
```

### 4.6 查询循环状态

```rust
pub struct QueryLoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition: Option<Continue>,
    pub max_output_tokens_override: Option<usize>,
    pub max_output_tokens_recovery_count: usize,
    pub auto_compact_tracking: Option<AutoCompactTracking>,
    pub has_attempted_reactive_compact: bool,
    pub pending_tool_use_summary: Option<String>,
    pub stop_hook_active: Option<bool>,
}

pub enum Terminal {
    Completed, AbortedStreaming, AbortedTools, BlockingLimit,
    PromptTooLong, ImageError, ModelError { error: String },
    HookStopped, StopHookPrevented, MaxTurns { turn_count: usize },
}

pub enum Continue {
    NextTurn, CollapseDrainRetry { committed: usize },
    ReactiveCompactRetry, MaxOutputTokensEscalate,
    MaxOutputTokensRecovery { attempt: usize },
    StopHookBlocking, TokenBudgetContinuation,
}
```

---

## 5. QueryEngine 与查询循环

### 5.1 QueryEngine (`engine/lifecycle.rs`)

```rust
pub struct QueryEngine {
    pub session_id: SessionId,
    config: QueryEngineConfig,
    
    // 跨轮可变状态 (Arc 包装)
    mutable_messages: Arc<RwLock<Vec<Message>>>,
    abort_reason: Arc<Mutex<Option<AbortReason>>>,
    aborted: Arc<AtomicBool>,
    usage: Arc<Mutex<UsageTracking>>,
    permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    total_turn_count: Arc<Mutex<usize>>,
    app_state: Arc<RwLock<AppState>>,
    tools: Arc<RwLock<Tools>>,
    
    // 会话级去重
    discovered_skill_names: Arc<Mutex<HashSet<String>>>,
    loaded_nested_memory_paths: Arc<Mutex<HashSet<String>>>,
}
```

**核心方法：**

```rust
pub fn submit_message(&self, prompt: &str, query_source: QuerySource) 
    -> Pin<Box<dyn Stream<Item = SdkMessage> + Send>>
```

**submit_message 5 阶段：**

| 阶段 | 动作 | 产出 |
|------|------|------|
| A | 输入处理 (斜杠命令/纯文本) | ProcessedInput |
| B | 系统提示词构建 | (system_prompt, user_context, system_context) |
| C | 预查询设置 | yield SystemInit; 本地命令快速路径 |
| D | 查询循环分发 | 消费 loop_impl::query() 的 Stream |
| E | 结果生成 | yield SdkResult (唯一终结信号) |

### 5.2 SDK 输出类型 (`engine/sdk_types.rs`)

```rust
pub enum SdkMessage {
    SystemInit(SystemInitMessage),       // 初始化信息
    Assistant(SdkAssistantMessage),      // 完整助手消息
    UserReplay(SdkUserReplay),           // 用户消息回放
    StreamEvent(SdkStreamEvent),         // 实时流事件
    CompactBoundary(SdkCompactBoundary), // 压缩边界
    ApiRetry(SdkApiRetry),               // API 重试通知
    ToolUseSummary(SdkToolUseSummary),   // 工具使用摘要
    Result(SdkResult),                   // 终结信号 (每次 submit 恰好一个)
}

pub struct SdkResult {
    pub subtype: ResultSubtype,  // Success | ErrorDuringExecution | ErrorMaxTurns | ...
    pub is_error: bool,
    pub duration_ms: u64,
    pub num_turns: usize,
    pub result: String,
    pub total_cost_usd: f64,
    pub usage: UsageTracking,
    pub permission_denials: Vec<PermissionDenial>,
    // ...
}
```

### 5.3 查询循环 8 步状态机 (`query/loop_impl.rs`)

```
┌─────────────────────────────────────────────────────────┐
│                    query() 主循环                        │
│                                                         │
│  STEP 1: SETUP                                          │
│    └─ 检查 abort → 若已取消则退出                          │
│                                                         │
│  STEP 2: CONTEXT — 压缩管道                              │
│    ├─ microcompact() — 裁剪超大工具结果                    │
│    └─ autocompact() — 紧急压缩 (>80% 窗口)               │
│                                                         │
│  STEP 3: API CALL — 流式调用模型                          │
│    ├─ yield RequestStart                                │
│    ├─ call_model_streaming() → Stream<StreamEvent>      │
│    ├─ 逐事件 yield + 累积到 StreamAccumulator            │
│    └─ 错误恢复: prompt_too_long → reactive_compact       │
│                                                         │
│  STEP 4: POST-STREAMING                                 │
│    ├─ 检查 abort                                        │
│    ├─ 注入 pending_tool_use_summary                     │
│    └─ yield AssistantMessage → 推入历史                   │
│                                                         │
│  STEP 5: TERMINAL CHECK (无工具调用时)                    │
│    ├─ 5a: max_output_tokens → 升级到 64k → 恢复尝试      │
│    ├─ 5b: stop_hooks → AllowStop / PreventStop          │
│    └─ 5c: token_budget → Continue / Stop                │
│    └─ BREAK                                             │
│                                                         │
│  STEP 6: TOOL EXECUTION (有工具调用时)                    │
│    ├─ 按 is_concurrency_safe() 分区                      │
│    ├─ 并发批次: tokio::spawn → await all                 │
│    ├─ 串行批次: 逐个执行                                  │
│    └─ yield UserMessage (工具结果)                        │
│                                                         │
│  STEP 7: ATTACHMENTS                                    │
│    └─ 文件变更检测, 技能发现, 记忆注入                      │
│                                                         │
│  STEP 8: CONTINUE                                       │
│    ├─ 检查 max_turns 限制                                │
│    ├─ refresh_tools() (MCP 可能变化)                     │
│    ├─ 递增 turn_count                                   │
│    └─ CONTINUE 主循环                                    │
└─────────────────────────────────────────────────────────┘
```

### 5.4 依赖注入 (`query/deps.rs`)

```rust
#[async_trait]
pub trait QueryDeps: Send + Sync {
    async fn call_model(params: ModelCallParams) -> Result<ModelResponse>;
    async fn call_model_streaming(params: ModelCallParams) 
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
    async fn microcompact(messages: Vec<Message>) -> Result<Vec<Message>>;
    async fn autocompact(messages, tracking) -> Result<Option<CompactionResult>>;
    async fn reactive_compact(messages) -> Result<Option<CompactionResult>>;
    async fn execute_tool(request, tools, parent, progress) -> Result<ToolExecResult>;
    fn get_app_state() -> AppState;
    fn is_aborted() -> bool;
    fn get_tools() -> Tools;
    async fn refresh_tools() -> Result<Tools>;
}
```

### 5.5 Token 预算管理 (`query/token_budget.rs`)

```
check_token_budget(tracker, agent_id, budget, global_turn_tokens)
  │
  ├─ 子 Agent / 无预算 → Stop
  ├─ turn_tokens < budget * 90% → Continue (注入进度提示)
  ├─ continuation_count >= 3 AND delta < 500 → Stop (收益递减)
  └─ 否则 → Continue
```

### 5.6 系统提示词构建 (`engine/system_prompt.rs`)

```
build_system_prompt(custom, append, tools, model, cwd)
  │
  ├─ 静态段 (可缓存, DYNAMIC_BOUNDARY 之前):
  │   ├─ intro       — 角色与能力
  │   ├─ system      — 系统规则
  │   ├─ doing_tasks — 任务执行指导
  │   ├─ actions     — 操作安全规则
  │   ├─ tools       — 工具使用指导
  │   ├─ tone        — 语调风格
  │   └─ efficiency  — 输出效率
  │
  ├─ __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__
  │
  └─ 动态段 (每轮重算):
      ├─ env_info              — 环境信息 (OS, shell, git)
      ├─ summarize_tool_results — 工具结果摘要规则
      ├─ mcp_instructions      — MCP 服务器指令
      └─ CLAUDE.md 上下文注入
```

---

## 6. 工具系统

### 6.1 工具注册表 (`tools/registry.rs`)

```rust
pub fn get_all_tools() -> Tools {
    vec![
        Arc::new(BashTool::new()),       Arc::new(FileReadTool::new()),
        Arc::new(FileWriteTool::new()),  Arc::new(FileEditTool::new()),
        Arc::new(GlobTool::new()),       Arc::new(GrepTool),
        Arc::new(AskUserQuestionTool),   Arc::new(AgentTool),
        Arc::new(SkillTool),             Arc::new(PowerShellTool),
        Arc::new(ConfigTool),            Arc::new(ReplTool),
        Arc::new(StructuredOutputTool),  Arc::new(SendUserMessageTool),
        Arc::new(WebFetchTool),          Arc::new(WebSearchTool),
        Arc::new(EnterPlanModeTool),     Arc::new(ExitPlanModeTool),
        Arc::new(EnterWorktreeTool),     Arc::new(ExitWorktreeTool),
        Arc::new(TaskCreateTool),        Arc::new(TaskGetTool),
        Arc::new(TaskUpdateTool),        Arc::new(TaskListTool),
        Arc::new(TaskStopTool),          Arc::new(TaskOutputTool),
        Arc::new(LspTool),              Arc::new(SendMessageTool),
    ]
    .into_iter().filter(|t| t.is_enabled()).collect()
}
```

### 6.2 工具分类总表

| 类别 | 工具 | 并发安全 | 只读 | 可破坏 |
|------|------|---------|------|--------|
| **文件系统** | Bash | ✗ | ✗ | ✗ |
| | Read | ✓ | ✓ | ✗ |
| | Write | ✗ | ✗ | ✓ |
| | Edit | ✗ | ✗ | ✓ |
| | Glob | ✓ | ✓ | ✗ |
| **搜索** | Grep | ✓ | ✓ | ✗ |
| | LSP | ✓ | ✓ | ✗ |
| **用户交互** | AskUser | ✗ | ✓ | ✗ |
| | SendUserMessage | ✓ | ✓ | ✗ |
| **代码执行** | PowerShell | ✗ | ✗ | ✓ |
| | REPL | ✗ | ✗ | ✓ |
| | Config | 视操作 | 视操作 | ✗ |
| **Agent/技能** | Agent | ✗ | ✗ | ✗ |
| | Skill | ✗ | ✗ | ✗ |
| **Web** | WebFetch | ✓ | ✓ | ✗ |
| | WebSearch | ✓ | ✓ | ✗ |
| **规划** | EnterPlanMode | ✗ | ✗ | ✗ |
| | ExitPlanMode | ✗ | ✗ | ✗ |
| **Worktree** | EnterWorktree | ✗ | ✗ | ✗ |
| | ExitWorktree | ✗ | ✗ | ✓ |
| **任务** | TaskCreate/Get/Update/List/Stop/Output | ✓ | 视操作 | ✗ |
| **数据** | StructuredOutput | ✓ | ✓ | ✗ |
| **Teams** | SendMessage | ✓ | ✗ | ✗ |

### 6.3 关键工具实现细节

**Bash** (`bash.rs`, 431 行):
- `tokio::process::Command::new("sh")` (Unix) / `"bash"` (Windows)
- 默认超时 120s, 最大 600s
- 输出截断策略: >100KB → 前 200 行 + 后 100 行 + 省略计数

**FileEdit** (`file_edit.rs`, 387 行):
- 精确字符串替换 + 模糊匹配 (similar::TextDiff)
- 如果 old_string 出现多次且未启用 replace_all → 报错
- 相似度 >60% 时提供模糊匹配建议

**Agent** (`agent.rs`):
- 创建子 QueryEngine 继承配置
- 模型别名解析: sonnet→claude-sonnet-4, opus→claude-opus-4, haiku→claude-haiku-4-5
- Worktree 隔离: git worktree add → 执行 → 统计变更 → 清理
- 深度限制: MAX_AGENT_DEPTH = 5

**WebFetch** (`web_fetch.rs`):
- LRU 缓存 (64 条, 15 分钟 TTL)
- HTTP → HTTPS 自动升级
- HTML → 纯文本 (去除 `<script>`, `<style>`)
- 最大 10MB, 截断到 100KB

**WebSearch** (`web_search.rs`):
- Brave Search API (`BRAVE_SEARCH_API_KEY` 环境变量)
- 域名过滤 (allow/block 列表)
- 默认 5 条结果, 最多 20 条

### 6.4 工具执行流程

```
QueryLoop STEP 6
  │
  ├─ 提取 tool_uses 列表 (从 AssistantMessage)
  │
  ├─ 分区: is_concurrency_safe() → 并发批次 / 串行批次
  │
  ├─ 每个工具:
  │   ├─ validate_input() → ValidationResult
  │   ├─ check_permissions() → Allow / Deny / Ask
  │   ├─ call(input, ctx, parent, on_progress) → ToolResult
  │   └─ 结果: { data: Value, new_messages: Vec<Message> }
  │
  └─ 汇总: Vec<ToolExecResult> → UserMessage (is_meta=true)
```

---

## 7. API 客户端与多 Provider

### 7.1 Provider 架构 (`api/providers.rs`)

三种协议:

| 协议 | SSE 格式 | Provider |
|------|---------|----------|
| `Anthropic` | content_block_start/delta/stop | Anthropic 原生 |
| `OpenAiCompat` | choices/delta | Azure, OpenAI, DeepSeek, Zhipu, Qwen, 等 12 家 |
| `Google` | gemini 累积文本 | Google Gemini |

**支持的 16 个 Provider：**

| Provider | 默认模型 | 协议 | 环境变量 |
|----------|---------|------|---------|
| Anthropic | claude-sonnet-4-20250514 | Anthropic | ANTHROPIC_API_KEY |
| Azure | gpt-4o | OpenAiCompat | AZURE_API_KEY |
| OpenAI | gpt-4o | OpenAiCompat | OPENAI_API_KEY |
| Google Gemini | gemini-2.0-flash | Google | GOOGLE_API_KEY |
| Groq | llama-3.3-70b | OpenAiCompat | GROQ_API_KEY |
| OpenRouter | claude-sonnet-4 | OpenAiCompat | OPENROUTER_API_KEY |
| DeepSeek | deepseek-chat | OpenAiCompat | DEEPSEEK_API_KEY |
| 智谱 GLM | glm-4-flash | OpenAiCompat | ZHIPU_API_KEY |
| 通义千问 | qwen-plus | OpenAiCompat | DASHSCOPE_API_KEY |
| Moonshot | moonshot-v1-8k | OpenAiCompat | MOONSHOT_API_KEY |
| 百川 | Baichuan4-Air | OpenAiCompat | BAICHUAN_API_KEY |
| MiniMax | MiniMax-Text-01 | OpenAiCompat | MINIMAX_API_KEY |
| 零一万物 | yi-lightning | OpenAiCompat | YI_API_KEY |
| SiliconFlow | DeepSeek-V3 | OpenAiCompat | SILICONFLOW_API_KEY |
| 阶跃星辰 | step-2-16k | OpenAiCompat | STEPFUN_API_KEY |
| 讯飞星火 | generalv3.5 | OpenAiCompat | SPARK_API_KEY |

### 7.2 API 客户端 (`api/client.rs`)

```rust
pub struct ApiClient {
    provider: ApiProvider,
    http: reqwest::Client,
    default_model: String,
}

impl ApiClient {
    pub fn from_env() -> Result<Self>;           // 自动检测 Provider
    pub fn from_auth() -> Result<Self>;          // 完整认证链
    pub async fn messages_stream(request) -> Stream<StreamEvent>;
    pub async fn messages_stream_with_retry(request, config) -> Stream<StreamEvent>;
}
```

### 7.3 流式解析 (`api/streaming.rs`)

```rust
pub enum StreamEvent {
    MessageStart { usage: Usage },
    ContentBlockStart { index: usize, content_block: ContentBlock },
    ContentBlockDelta { index: usize, delta: ContentBlockDelta },
    ContentBlockStop { index: usize },
    MessageDelta { delta: MessageDelta, usage: Usage },
    MessageStop,
}

pub struct StreamAccumulator {
    content_blocks: Vec<ContentBlock>,
    usage: Usage,
    stop_reason: Option<String>,
}
```

### 7.4 重试策略 (`api/retry.rs`)

```rust
pub struct RetryConfig {
    pub max_retries: usize,            // 默认 3
    pub initial_delay_ms: u64,         // 默认 1000
    pub max_delay_ms: u64,             // 默认 30000
    pub backoff_multiplier: f64,       // 默认 2.0
    pub retryable_status_codes: Vec<u16>,  // [429, 500, 502, 503, 529]
}

// 重试延迟 = initial * backoff^attempt * (0.8~1.2 抖动)
```

### 7.5 协议适配

**OpenAI 兼容** (`api/openai_compat.rs`):
- Anthropic 消息格式 → OpenAI chat/completions 格式
- tool_use blocks → OpenAI tool_calls
- ToolResult → `{"role": "tool"}` 消息
- Token 限制参数: Azure/OpenAI → `max_completion_tokens`, 其他 → `max_tokens`

**Google Gemini** (`api/google_provider.rs`):
- Anthropic → Gemini 格式 (assistant→model, 合并连续同角色消息)
- Gemini 返回累积文本 → 差分计算产生增量 delta
- URL: `{base}/models/{model}:streamGenerateContent?key={key}&alt=sse`

---

## 8. 认证系统

### 认证链 (`auth/mod.rs`)

```
resolve_auth()
  │
  ├─ 1. ANTHROPIC_API_KEY 环境变量 (必须通过 validate_api_key)
  ├─ 2. ANTHROPIC_AUTH_TOKEN 环境变量
  ├─ 3. 系统 Keychain (keyring, 服务名 "cc-rust")
  └─ 4. AuthMethod::None
```

```rust
pub enum AuthMethod {
    ApiKey(String),           // sk-ant-* 前缀, >20 字符
    ExternalToken(String),    // Bearer token
    None,
}
```

**Keychain 存储** (`auth/api_key.rs`):
- 服务名: `"cc-rust"` (与原版 `"claude-code"` 隔离)
- 账户名: `"api-key"`
- 使用 `keyring` crate 操作系统级凭据存储

**OAuth** (`auth/token.rs`):
- Token 持久化: `~/.cc-rust/credentials.json`
- 接口已定义, 实现尚未完成

---

## 9. 权限系统

### 9.1 权限模式

| 模式 | 行为 | 用途 |
|------|------|------|
| `Default` | 每次询问用户 | 生产使用 |
| `Auto` | 自动批准 (安全分类器) | 自动化场景 |
| `Bypass` | 跳过所有检查 | 开发模式 |
| `Plan` | 只读, 拒绝写入 | 规划阶段 |

### 9.2 决策流程 (`permissions/decision.rs`)

```
has_permissions_to_use_tool(tool_name, input, ctx, denial_tracker)
  │
  ├─ Phase 1a: 无条件规则 (仅按工具名)
  │   ├─ deny rules → Deny (最高优先级)
  │   ├─ allow rules → Allow
  │   └─ ask rules → Ask
  │
  ├─ Phase 1b: 模式匹配规则 (按输入内容)
  │   ├─ Bash: 命令首词 (如 "git")
  │   ├─ Read/Write/Edit: file_path
  │   └─ Glob: pattern
  │   ├─ allow patterns → Allow
  │   └─ deny patterns → Deny
  │
  ├─ Phase 2: Hook 拦截 (预留)
  │
  └─ Phase 3: 模式回退
      ├─ Bypass → Allow
      ├─ Auto → Allow (denial_tracker 回退时 → Ask)
      ├─ Plan → Ask
      └─ Default → Ask
```

### 9.3 危险命令检测 (`permissions/dangerous.rs`)

8 类危险模式 (编译时正则):

| 类别 | 示例 | 原因 |
|------|------|------|
| 递归删除 | `rm -rf /` | 破坏性文件操作 |
| Git 强推 | `git push --force` | 覆盖远程历史 |
| 磁盘操作 | `dd if=...`, `mkfs` | 直接磁盘写入 |
| 权限炸弹 | `chmod -R 777 /` | 安全性破坏 |
| Fork 炸弹 | `:(){ :|:& };:` | 耗尽资源 |
| 设备破坏 | `> /dev/sda` | 覆盖块设备 |
| 管道注入 | `curl ... | sh` | 远程代码执行 |
| 系统文件 | `> /etc/passwd` | 破坏认证 |

### 9.4 路径校验 (`permissions/path_validation.rs`)

- 路径遏制检查: 文件是否在 CWD 或额外允许目录内
- 路径遍历攻击检测: `../` 超过 5 层, `/etc/passwd`, `%2e%2e`
- 符号链接解析 + 规范化

---

## 10. 技能系统

### 10.1 技能来源

| 来源 | 路径 | 优先级 |
|------|------|--------|
| Bundled | 编译时内置 | 最先注册 |
| User | `~/.cc-rust/skills/` | 其次 |
| Project | `.cc-rust/skills/` | 最后 |
| Plugin | 插件贡献 | 动态 |
| MCP | MCP 服务器 | 动态 |

### 10.2 技能定义 (`skills/mod.rs`)

```yaml
# SKILL.md frontmatter
---
name: Custom Name
description: 技能描述
when-to-use: 模型使用指导
allowed-tools: Read, Grep, Edit, Bash
argument-hint: "需要什么输入"
arguments: NAME, LANG
model: claude-opus-4-20250514
user-invocable: true
disable-model-invocation: true
context: inline    # 或 "fork"
agent: code-simplifier
---

技能正文 (Markdown)
支持变量: ${CLAUDE_SKILL_DIR}, ${CLAUDE_SESSION_ID}, $ARGUMENTS, ${ARG_NAME}
```

### 10.3 内置技能

| 技能 | 上下文 | 允许工具 | 用途 |
|------|--------|---------|------|
| `simplify` | fork | Read, Grep, Glob, Edit, Write, Bash | 代码简化 |
| `remember` | inline | Read, Edit, Write | 保存到 CLAUDE.md |
| `debug` | inline | Read, Grep, Glob, Bash | 诊断调试 |
| `stuck` | inline | 所有 | 卡住时求助 |
| `update-config` | inline | Read, Edit, Write | 修改设置 |

### 10.4 全局注册表

```rust
static REGISTRY: LazyLock<Mutex<Vec<SkillDefinition>>> = LazyLock::new(|| ...);

pub fn init_skills(project_dir: Option<&Path>) {
    // 1. 注册内置技能
    // 2. 加载用户技能 (~/.cc-rust/skills/)
    // 3. 加载项目技能 (.cc-rust/skills/)
}
```

---

## 11. 命令系统

### 32 个斜杠命令

| 类别 | 命令 | 别名 | 说明 |
|------|------|------|------|
| **会话** | /help | h, ? | 帮助 |
| | /clear | | 清除对话 |
| | /exit | quit, q | 退出 |
| | /status | | 会话状态 |
| | /session | | 会话信息/列表 |
| | /resume | | 恢复会话 |
| **配置** | /config | settings | 设置管理 |
| | /version | v | 版本号 |
| | /model | | 切换模型 |
| | /effort | | 思考深度 |
| | /fast | | 快速模式 |
| | /init | | 初始化项目 |
| | /permissions | perms | 权限管理 |
| **Git** | /diff | | Git diff |
| | /commit | | Git commit |
| | /branch | br | 分支管理 |
| **认证** | /login | | 登录 |
| | /logout | | 登出 |
| **分析** | /cost | usage | 费用统计 |
| | /context | ctx | Token 估算 |
| | /extra-usage | eu | 详细分析 |
| | /rate-limit-options | rlo | 限流指南 |
| **导出** | /export | | Markdown 导出 |
| | /audit-export | audit | 审计导出 (SHA-256 链) |
| | /session-export | sexport | JSON 数据包 |
| **其他** | /files | | 引用文件列表 |
| | /memory | mem | CLAUDE.md 查看 |
| | /skills | | 技能列表 |
| | /copy | cp | 复制到剪贴板 |
| | /compact | | 上下文压缩 |
| | /mcp | | MCP 管理 |

### 命令架构

```rust
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult>;
}

pub enum CommandResult {
    Output(String),        // 显示文本 (不发给模型)
    Query(Vec<Message>),   // 发给模型的消息
    Clear,                 // 清除对话
    Exit(String),          // 退出
    None,                  // 无输出
}
```

---

## 12. 上下文压缩管道

### 5 阶段管道 (`compact/pipeline.rs`)

```
run_context_pipeline(messages, tracking, model) → PipelineResult
  │
  ├─ Stage 1: tool_result_budget
  │   └─ >100K 字符的工具结果 → 持久化到磁盘, 替换为预览
  │      (前 500 字符 + ... + 后 200 字符)
  │
  ├─ Stage 2: snip_compact
  │   └─ >200 轮 → 保留首条消息 + CompactBoundary + 最后 N 轮
  │
  ├─ Stage 3: microcompact
  │   └─ 保留最近 10 个工具结果完整, 旧的 >1000 字符 → 截断摘要
  │
  ├─ Stage 4: context_collapse (Phase 2+)
  │   └─ 折叠旧段为摘要 (预留)
  │
  └─ Stage 5: auto_compact
      └─ >80% 上下文窗口 → 完整对话摘要化
         ├─ 调用模型生成摘要 (max 20K tokens)
         ├─ 恢复最近 5 个文件 (每个 max 5K tokens)
         ├─ 重注入技能指令 (25K budget, max 5 技能)
         └─ 生成 CompactBoundary 标记
```

**紧急压缩 (reactive_compact):**
- 触发条件: API 返回 `prompt_too_long` 错误
- 每次 submit_message 最多尝试一次

**关键常量:**

| 常量 | 值 | 用途 |
|------|-----|------|
| DEFAULT_SNIP_MAX_TURNS | 200 | 普通裁剪 |
| REACTIVE_SNIP_MAX_TURNS | 5 | 紧急裁剪 |
| KEEP_RECENT_TOOL_RESULTS | 10 | 保留最近结果数 |
| SIZE_THRESHOLD_CHARS | 1000 | 微压缩阈值 |
| MAX_OUTPUT_TOKENS_FOR_SUMMARY | 20000 | 摘要最大输出 |
| MAX_RECOVERED_FILES | 5 | 恢复文件数 |
| MAX_CONSECUTIVE_FAILURES | 3 | 断路器 |

---

## 13. 会话持久化

### 13.1 存储结构

```
~/.cc-rust/
├── sessions/                          会话存储
│   └── {session-id}.json             SessionFile (消息 + 元数据)
├── transcripts/                       审计日志
│   └── {session-id}.ndjson           NDJSON 追加式记录
├── exports/                           导出
│   ├── {session-id}.md               Markdown 导出
│   └── {session-id}.session.json     JSON 数据包
├── audits/                            审计
│   └── {session-id}.audit.json       SHA-256 链式审计记录
└── memory/                            全局记忆
    └── {key}.json                     MemoryEntry
```

### 13.2 会话文件格式

```rust
pub struct SessionFile {
    session_id: String,
    created_at: i64,
    last_modified: i64,
    cwd: String,
    messages: Vec<SerializableMessage>,
}

pub struct SerializableMessage {
    msg_type: String,     // "user" | "assistant" | "system" | ...
    uuid: Uuid,
    timestamp: i64,
    data: serde_json::Value,  // 序列化的消息内容
}
```

### 13.3 版本迁移

```
V1 → V2: 添加 metadata 对象 (cwd, created_at)
V2 → V3: 规范化消息内容为数组格式
```

### 13.4 审计导出 (SHA-256 链)

```rust
pub struct AuditRecord {
    format_version: String,
    metadata: AuditMetadata,
    entries: Vec<AuditEntry>,
    integrity: IntegrityInfo,
}

pub struct AuditEntry {
    sequence: usize,
    uuid: Uuid,
    timestamp: i64,
    msg_type: String,
    hash: String,           // SHA-256(data)
    chain_hash: String,     // SHA-256(prev_chain_hash || hash)
    data: Value,
}
```

### 13.5 记忆系统 (`session/memdir.rs`)

```rust
pub struct MemoryEntry {
    key: String,
    value: String,
    category: String,
    created_at: i64,
    updated_at: i64,
}

pub enum MemoryScope { Global, Project }

// 构建记忆上下文 → 注入系统提示词
pub fn build_memory_context(cwd) → Result<String>
// 输出:
// <memory-context>
// ## Project Memories
// - **key**: value
// </memory-context>
```

---

## 14. IPC 与 Headless 模式

### 14.1 协议 (`ipc/protocol.rs`)

**前端 → 后端：**

| 消息类型 | 字段 | 用途 |
|---------|------|------|
| `SubmitPrompt` | text, id | 提交查询 |
| `AbortQuery` | | 取消 |
| `PermissionResponse` | tool_use_id, decision | 权限应答 |
| `SlashCommand` | raw | 斜杠命令 |
| `Resize` | cols, rows | 终端尺寸 |
| `Quit` | | 退出 |

**后端 → 前端：**

| 消息类型 | 字段 | 用途 |
|---------|------|------|
| `Ready` | session_id, model, cwd | 初始化完成 |
| `StreamStart/Delta/End` | message_id, text | 流式文本 |
| `AssistantMessage` | id, content, cost_usd | 完整消息 |
| `ToolUse` | id, name, input | 工具调用 |
| `ToolResult` | tool_use_id, output, is_error | 工具结果 |
| `PermissionRequest` | tool_use_id, tool, command, options | 权限请求 |
| `SystemInfo` | text, level | 系统消息 |
| `UsageUpdate` | input_tokens, output_tokens, cost_usd | 用量更新 |
| `Suggestions` | items | 提示建议 |
| `Error` | message, recoverable | 错误 |

### 14.2 Headless 事件循环 (`ipc/headless.rs`)

```
run_headless(engine, model)
  ├─ 发送 Ready 消息
  ├─ 从 stdin 逐行读取 JSON
  ├─ SubmitPrompt → tokio::spawn 查询任务
  │   ├─ StreamEvent → BackendMessage 映射
  │   └─ 完成后发送 UsageUpdate + AssistantMessage
  ├─ AbortQuery → engine.abort()
  └─ Quit → break 事件循环
```

---

## 15. TUI 渲染层

### 15.1 架构 (`ui/`)

```
ui/
├── tui.rs           主事件循环 (crossterm 输入 + engine 输出 + tick)
├── app.rs           App 状态 + 渲染逻辑
├── theme.rs         主题 (ratatui::Style 集合)
├── keybindings.rs   键绑定 → Action 映射
├── virtual_scroll.rs  O(log n) 虚拟滚动 (前缀和 + 二分搜索)
├── messages.rs      消息渲染 (Markdown/diff/工具块)
├── markdown.rs      Markdown → ratatui 渲染
├── diff.rs          Diff 样式
├── prompt_input.rs  输入行编辑 (含 Vim 模式)
├── spinner.rs       动画 spinner
├── permissions.rs   权限对话框
├── vim.rs           Vim 键绑定模式
└── welcome.rs       欢迎屏幕
```

### 15.2 主循环

```rust
pub fn run_tui(engine, initial_prompt, model_name, shutdown_token) -> Result<()> {
    // 启用 raw mode + alternate screen
    // 创建 App 状态
    // 生成 OS 线程: crossterm::event::read() → mpsc 通道
    // 主循环: tokio::select!
    //   ├─ 终端事件 → handle_key_event() → AppAction
    //   ├─ 引擎事件 → 更新消息/流状态
    //   └─ tick (80ms) → 推进 spinner
    // 清理: TerminalGuard (RAII)
}
```

### 15.3 虚拟滚动

```rust
pub struct VirtualScroll {
    heights: Vec<usize>,    // 每条消息的渲染行数
    offsets: Vec<usize>,    // 前缀和: offsets[i] = sum(heights[0..i])
    valid_up_to: usize,     // 有效缓存范围
    cached_width: u16,      // 缓存时的宽度
}

// 可见范围: O(log n) 二分搜索
fn visible_range(scroll_offset, viewport_height) -> (start_msg, end_msg)

// 过度扫描: OVERSCAN = 40 行 (平滑滚动)
```

---

## 16. MCP 服务器集成

### 16.1 架构 (`mcp/`)

```
mcp/
├── protocol.rs    JSON-RPC 2.0 类型定义
├── client.rs      McpClient (单服务器连接)
├── manager.rs     McpManager (多服务器聚合)
├── discovery.rs   MCP 服务器发现
└── tools.rs       McpToolWrapper (Tool trait 适配)
```

### 16.2 连接生命周期

```
McpClient::connect()
  ├─ 启动子进程 (stdin/stdout 管道)
  ├─ 生成后台 reader_loop (读取 JSON-RPC 响应)
  └─ 请求多路复用: id → oneshot channel

McpClient::initialize()
  ├─ 发送 initialize 请求
  ├─ 接收 ServerCapabilities
  └─ 发送 notifications/initialized

McpClient::list_tools() → Vec<McpToolDef>
McpClient::call_tool(name, args) → CallToolResult (300s 超时)
McpClient::disconnect() → 优雅关闭
```

### 16.3 工具包装

```rust
pub struct McpToolWrapper {
    def: McpToolDef,
    manager: Arc<Mutex<McpManager>>,
}

impl Tool for McpToolWrapper {
    fn name(&self) -> &str { &self.def.name }  // "mcp__{server}__{tool}"
    async fn call(&self, input, ctx, parent, progress) -> Result<ToolResult> {
        let client = self.manager.find_client_for_tool(&self.def.name)?;
        let result = client.call_tool(&self.def.name, input).await?;
        // 格式化 ToolCallContent → ToolResult
    }
}
```

---

## 17. 插件系统

### 17.1 三层模型

```
意图层 (settings.json)
  → 物化层 (cache/ 磁盘)
    → 激活层 (内存注册表)
```

### 17.2 插件清单 (`plugin.json`)

```rust
pub struct PluginManifest {
    name: String,
    display_name: Option<String>,
    version: String,              // semver
    description: Option<String>,
    tools: Vec<ToolContribution>,
    skills: Vec<SkillContribution>,
    mcp_servers: Vec<McpServerContribution>,
    commands: Vec<CommandContribution>,
    dependencies: Option<Value>,
    configuration: Option<Value>,
}
```

### 17.3 存储结构

```
~/.cc-rust/plugins/
├── cache/{marketplace}/{plugin}/{version}/
│   └── plugin.json
├── marketplaces/{marketplace}/
│   └── marketplace.json
├── known_marketplaces.json
└── installed_plugins.json
```

---

## 18. 多 Agent Teams

### 18.1 架构概览

```
团队领导 (team-lead)
  ├─ 创建团队 → team_config.json
  ├─ 生成队友 → TeammateSpawnConfig
  │   ├─ InProcess: tokio::spawn + 子 QueryEngine
  │   ├─ Tmux: tmux split-window
  │   └─ iTerm2: iTerm2 pane
  ├─ 邮箱通信 → ~/.cc-rust/teams/{team}/inboxes/{agent}.json
  └─ 协议消息 → ShutdownRequest, PlanApproval, Permission, Idle, TaskAssignment
```

### 18.2 执行后端

```rust
pub trait TeammateExecutor: Send + Sync {
    fn spawn(config: TeammateSpawnConfig) -> TeammateSpawnResult;
    fn send_message(agent_id, team_name, message);
    fn terminate(agent_id, team_name, reason) -> bool;
    fn kill(agent_id);
    fn is_active(agent_id) -> bool;
}
```

| 后端 | 实现 | 隔离性 |
|------|------|--------|
| `InProcess` | tokio::spawn + 子 QueryEngine | 进程内, task_local 上下文 |
| `Tmux` | tmux split-window | 独立进程 + tmux 窗格 |
| `iTerm2` | iTerm2 API | 独立进程 + iTerm2 pane |

### 18.3 邮箱 IPC

```
文件路径: ~/.cc-rust/teams/{team_name}/inboxes/{agent_name}.json
格式: JSON 数组 [{ from, text, timestamp, read, color, summary }, ...]
锁定: 排他锁文件 ({inbox}.lock), 指数退避 (5-100ms, 10 次重试)
轮询: 500ms 间隔检查新消息
```

### 18.4 协议消息

| 消息 | 方向 | 用途 |
|------|------|------|
| `ShutdownRequest/Approved/Rejected` | Lead→Member / Member→Lead | 关闭协商 |
| `PlanApprovalRequest/Response` | Member→Lead / Lead→Member | 计划审批 |
| `PermissionRequest/Response` | Member→Lead / Lead→Member | 权限请求 |
| `IdleNotification` | Member→Lead | 空闲通知 |
| `TaskAssignment` | Lead→Member | 任务分配 |
| `TeamPermissionUpdate` | Lead→All | 权限更新广播 |

### 18.5 身份与上下文

```rust
// Agent ID 格式
fn format_agent_id(agent_name, team_name) -> String {
    format!("{agent_name}@{team_name}")
}

// task_local 上下文传播
tokio::task_local! {
    static TEAMMATE_CONTEXT: TeammateIdentity;
}

fn run_in_scope(identity: TeammateIdentity, future: F) -> F::Output {
    TEAMMATE_CONTEXT.scope(identity, future).await
}
```

---

## 19. Bootstrap 与进程状态

### ProcessState 全局单例

```rust
pub struct ProcessState {
    pub original_cwd: PathBuf,
    pub project_root: Option<PathBuf>,
    pub session_id: SessionId,
    pub parent_session_id: Option<SessionId>,
    pub total_cost_usd: AtomicF64,
    pub api_duration: DurationTracker,        // 无锁累积计时
    pub tool_duration: DurationTracker,
    pub main_loop_model_override: Option<String>,
    pub initial_main_loop_model: String,
    pub model_strings: ModelStrings,
    pub is_interactive: bool,
    pub error_log: ErrorLog,                  // 环形缓冲 (100 条)
    pub slow_operations: SlowOperationTracker, // >500ms 操作追踪 (50 条)
    pub invoked_skills: HashSet<String>,
}
```

**特点：**
- `LazyLock<RwLock<ProcessState>>` — 全局单例
- `DurationTracker` 使用 `AtomicU64` — 无锁累积
- 不依赖任何上层模块 (纯叶节点)
- 在 Phase B 初始化一次, 此后只读 + 递增更新

### Signal<T> 响应式原语

```rust
pub struct Signal<T> {
    tx: watch::Sender<T>,
    rx: watch::Receiver<T>,
}
// 用于: 值变更通知, 无轮询等待
```

---

## 20. 服务层

### 4 个后台服务 (`services/`)

| 服务 | 类型 | 用途 |
|------|------|------|
| `tool_use_summary` | 本地 (无 API) | 生成工具使用一行摘要 |
| `session_memory` | 磁盘持久化 | 提取并存储对话洞察 |
| `prompt_suggestion` | 本地 (启发式) | 基于工具的下一步提示建议 |
| `lsp_lifecycle` | 进程管理 | LSP 服务器生命周期管理 |

**tool_use_summary:**
```
"Used Bash to run a command (ls -la) → file list, Edit to edit a file (main.rs) → ok"
```

**prompt_suggestion:**
- 基于最近使用的工具生成建议
- Bash → "Run the tests to verify" (0.7 confidence)
- Edit → "Review the changes" (0.6)
- 限速: 30s 间隔

**lsp_lifecycle:**
- 支持: rust-analyzer, typescript-language-server, pylsp, gopls, clangd, jdtls
- 状态: NotStarted, Starting, Running, Stopped, Error

---

## 21. 工具函数

### `utils/` 模块

| 文件 | 用途 |
|------|------|
| `tokens.rs` | Token 估算 (4 字符/token 启发式) |
| `file_state_cache.rs` | LRU 文件状态缓存 (SipHash) |
| `abort.rs` | AbortController (协作取消, 父子层级) |
| `bash.rs` | Shell 命令解析, 危险模式检测, heredoc 校验 |
| `git.rs` | Git 操作 (branch, HEAD SHA, status) |
| `cwd.rs` | 工作目录管理 |
| `shell.rs` | Shell 检测与初始化 |
| `messages.rs` | 消息工具函数 |

**AbortController 层级取消:**
```rust
pub struct AbortController {
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
    reason: Mutex<Option<String>>,
}

// 父取消 → 子自动取消
pub fn child(&self) -> AbortController {
    // 子控制器订阅父信号, 父取消时自动传播
}
```

---

## 22. 并发模型与线程安全

### 共享状态模式

| 类型 | 包装 | 共享方 |
|------|------|--------|
| `AppState` | `Arc<RwLock<>>` | ToolUseContext, UI, Engine |
| `Messages` | `Arc<RwLock<Vec<>>>` | 查询循环, 会话存储 |
| `UsageTracking` | `Arc<Mutex<>>` | 查询循环, 结果生成 |
| `Tools` | `Arc<RwLock<>>` | 查询循环, 工具执行 |
| `AbortSignal` | `watch::Receiver<bool>` | 所有运行中工具 |

### 取消传播

```
CancellationToken (shutdown.rs)
  └─ TUI 主循环检查 is_cancelled()
      └─ 退出 → graceful_shutdown()
          └─ engine.abort()
              └─ abort_signal → watch::Sender::send(true)
                  └─ 所有工具的 abort_signal 收到通知
```

### 工具并发调度

```
工具分区 by is_concurrency_safe():
  ├─ [Read, Read, Grep] → 并发批次: tokio::spawn × 3 → join_all
  ├─ [Bash]             → 串行批次: 单独执行
  └─ [Read, Glob]       → 并发批次: tokio::spawn × 2 → join_all
```

---

## 23. 关键设计模式

### 1. 消息历史作为主状态
- 所有消息 (user/assistant/system/progress) 进入单一 `Vec<Message>`
- 每轮持久化 → 支持会话恢复和审计

### 2. Generator 风格流式 API
- `async_stream::stream!` 宏实现异步生成器
- `submit_message()` 返回 `Pin<Box<dyn Stream<Item = SdkMessage>>>`
- 消费者异步读取事件, 零阻塞

### 3. 闭包注入避免生命周期
- `ToolUseContext` 通过 `get_app_state` / `set_app_state` 闭包访问状态
- 避免 `&mut AppState` 引用导致的生命周期问题
- 原子更新: `FnOnce(AppState) -> AppState`

### 4. 静态/动态提示词分界
- `DYNAMIC_BOUNDARY` 标记: 之前可缓存, 之后每轮重算
- 缓存段: 角色/规则/工具指导
- 动态段: 环境信息/MCP 指令

### 5. 特征对象 + Arc 的工具多态
- `type Tools = Vec<Arc<dyn Tool>>`
- 所有工具共享 Tool trait 接口
- 注册表模式: `get_all_tools()` 返回全部

### 6. 查询循环状态机而非递归
- 循环产出 `Terminal` 或 `Continue` 而非直接递归
- 支持背压、用户中断和恢复逻辑
- `transition` 字段记录继续原因

### 7. 破坏性操作分类
- `is_read_only()` / `is_destructive()` / `is_concurrency_safe()`
- 权限系统根据分类强制安全规则

### 8. 路径隔离
- 所有持久化路径使用 `~/.cc-rust/` (区别于原版 `~/.claude/`)
- Keychain 服务名: `"cc-rust"` (区别于 `"claude-code"`)
- 项目配置: `.cc-rust/settings.json`

---

## 24. 数据流全景图

```
用户输入
  │
  ▼
main.rs: Cli 解析 + Phase B 初始化
  │
  ▼
QueryEngine::new(QueryEngineConfig)
  ├─ Arc<RwLock<AppState>>
  ├─ Arc<RwLock<Vec<Message>>>
  └─ Arc<Mutex<UsageTracking>>
  │
  ▼
QueryEngine::submit_message(prompt, QuerySource)
  │
  ├─ Phase A: process_user_input() → ProcessedInput
  ├─ Phase B: build_system_prompt() → (系统提示, 上下文)
  ├─ Phase C: yield SystemInit → 本地命令快速路径
  ├─ Phase D: query(params, deps) → 8 步循环
  │   │
  │   ├─ STEP 2: microcompact + autocompact
  │   ├─ STEP 3: call_model_streaming() → Stream<StreamEvent>
  │   │   └─ ApiClient → HTTP SSE → parse → StreamEvent
  │   │       ├─ Anthropic SSE (原生)
  │   │       ├─ OpenAI SSE → 适配
  │   │       └─ Gemini SSE → 累积差分
  │   ├─ STEP 4: yield AssistantMessage
  │   ├─ STEP 5: Terminal 检查 (无工具)
  │   │   ├─ max_tokens 恢复
  │   │   ├─ stop_hooks
  │   │   └─ token_budget
  │   ├─ STEP 6: 工具执行 (有工具)
  │   │   ├─ validate_input() → check_permissions()
  │   │   ├─ 并发/串行分区
  │   │   └─ call() → ToolResult { data, new_messages }
  │   └─ STEP 8: max_turns 检查 → CONTINUE
  │
  └─ Phase E: yield SdkResult (终结信号)
  │
  ▼
消费层
  ├─ TUI: app.render(msg) → ratatui 渲染
  ├─ Headless: BackendMessage → JSONL → ink-terminal
  ├─ Print: 提取文本 → stdout
  └─ JSON: SdkMessage → JSONL → SDK 消费者
  │
  ▼
Phase I: graceful_shutdown()
  ├─ abort → flush_transcript → save_session
  └─ 重置终端 → 打印用量
```

---

## 25. 构建与依赖

### Cargo.toml 依赖分类

| 类别 | 依赖 |
|------|------|
| **异步运行时** | tokio (full), futures, async-trait, async-stream, tokio-stream, tokio-util |
| **序列化** | serde + derive, serde_json |
| **错误处理** | anyhow, thiserror |
| **CLI** | clap (derive) |
| **文件搜索** | glob, ignore, walkdir, dirs |
| **正则** | regex |
| **Git** | git2 (https) |
| **文本** | similar, textwrap, unicode-width, strip-ansi-escapes, pulldown-cmark |
| **终端 UI** | ratatui, crossterm, syntect |
| **网络** | reqwest (json, stream, rustls-tls) |
| **认证** | keyring, dotenvy |
| **哈希** | sha2, hex |
| **编码** | base64, bytes, url, semver |
| **缓存** | lru |
| **日志** | tracing, tracing-subscriber (env-filter) |
| **UUID** | uuid (v4, serde) |
| **时间** | chrono |

### 构建命令

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# ink-terminal 前端
git submodule update --init --recursive
cd ui && bun install && bun run dev

# 运行测试
cargo test

# 统计
# 176 个 .rs 文件, ~49,883 行代码
```

---

## 附录 A: 文件清单

```
src/
├── main.rs                        入口 (Phase A/B/I)
├── shutdown.rs                    优雅关闭
│
├── bootstrap/                     进程级全局状态
│   ├── state.rs                   ProcessState 单例
│   ├── ids.rs                     SessionId 品牌类型
│   └── signal.rs                  Signal<T> 响应式原语
│
├── types/                         核心类型定义
│   ├── message.rs                 Message + ContentBlock
│   ├── tool.rs                    Tool trait + ToolUseContext
│   ├── state.rs                   QueryLoopState + BudgetTracker
│   ├── config.rs                  QueryEngineConfig + QueryParams
│   ├── app_state.rs               AppState + SettingsJson
│   └── transitions.rs             Terminal + Continue
│
├── engine/                        QueryEngine 生命周期
│   ├── lifecycle.rs               submit_message() 主控
│   ├── system_prompt.rs           系统提示词构建
│   ├── input_processing.rs        输入处理
│   ├── result.rs                  结果提取
│   ├── sdk_types.rs               SdkMessage 输出
│   └── prompt_sections.rs         分段缓存
│
├── query/                         异步查询循环
│   ├── loop_impl.rs               8 步状态机
│   ├── deps.rs                    QueryDeps DI trait
│   ├── token_budget.rs            Token 预算
│   └── stop_hooks.rs              停止钩子
│
├── tools/                         28 工具实现
│   ├── registry.rs                注册表
│   ├── execution.rs               执行引擎
│   ├── hooks.rs                   Hook 系统
│   ├── orchestration.rs           编排
│   ├── bash.rs                    Shell 执行
│   ├── file_read.rs               文件读取 (多格式)
│   ├── file_write.rs              文件写入
│   ├── file_edit.rs               字符串替换 + 模糊匹配
│   ├── glob_tool.rs               Glob 搜索
│   ├── grep.rs                    正则搜索 (ripgrep)
│   ├── agent.rs                   子 Agent (worktree 隔离)
│   ├── skill.rs                   技能调用
│   ├── ask_user.rs                用户询问
│   ├── send_user_message.rs       用户通知
│   ├── send_message.rs            团队消息
│   ├── web_fetch.rs               HTTP + HTML→文本
│   ├── web_search.rs              Brave 搜索
│   ├── powershell.rs              PowerShell
│   ├── repl.rs                    多语言 REPL
│   ├── config_tool.rs             配置读写
│   ├── structured_output.rs       数据格式化
│   ├── lsp.rs                     LSP 代码智能
│   ├── plan_mode.rs               规划模式
│   ├── worktree.rs                Git Worktree
│   └── tasks.rs                   任务管理 (6 工具)
│
├── compact/                       上下文压缩
│   ├── pipeline.rs                5 阶段管道编排
│   ├── snip.rs                    历史裁剪
│   ├── microcompact.rs            工具结果微压缩
│   ├── auto_compact.rs            自动压缩阈值
│   ├── compaction.rs              完整摘要化
│   ├── tool_result_budget.rs      大结果持久化
│   └── messages.rs                消息规范化
│
├── commands/                      32 斜杠命令
│   ├── mod.rs                     命令注册表
│   └── (30+ 命令文件)
│
├── skills/                        技能系统
│   ├── mod.rs                     注册表 + SkillDefinition
│   ├── loader.rs                  目录发现 + YAML 解析
│   └── bundled.rs                 5 内置技能
│
├── api/                           多 Provider API
│   ├── client.rs                  ApiClient (16 Provider)
│   ├── providers.rs               ProviderInfo 定义
│   ├── streaming.rs               SSE 解析 + StreamAccumulator
│   ├── retry.rs                   重试策略
│   ├── openai_compat.rs           OpenAI 协议适配
│   └── google_provider.rs         Gemini 协议适配
│
├── auth/                          认证
│   ├── mod.rs                     AuthMethod + resolve_auth()
│   ├── api_key.rs                 Keychain 存储
│   └── token.rs                   OAuth token 持久化
│
├── permissions/                   权限系统
│   ├── rules.rs                   规则匹配引擎
│   ├── decision.rs                决策状态机
│   ├── dangerous.rs               危险命令检测
│   └── path_validation.rs         路径校验
│
├── config/                        配置管理
│   ├── settings.rs                加载与合并
│   ├── constants.rs               模型/API/Token 常量
│   ├── validation.rs              配置校验
│   └── claude_md.rs               CLAUDE.md 发现
│
├── session/                       会话持久化
│   ├── storage.rs                 保存/加载
│   ├── transcript.rs              NDJSON 审计日志
│   ├── resume.rs                  会话恢复
│   ├── migrations.rs              版本迁移 (V1→V3)
│   ├── memdir.rs                  记忆系统
│   ├── export.rs                  Markdown 导出
│   ├── audit_export.rs            SHA-256 链式审计
│   └── session_export.rs          JSON 数据包
│
├── ipc/                           IPC 协议
│   ├── protocol.rs                消息类型定义
│   └── headless.rs                Headless 事件循环
│
├── ui/                            TUI 渲染
│   ├── tui.rs                     主事件循环
│   ├── app.rs                     App 状态 + 渲染
│   ├── theme.rs                   主题
│   ├── keybindings.rs             键绑定
│   ├── virtual_scroll.rs          虚拟滚动
│   ├── messages.rs                消息渲染
│   ├── markdown.rs                Markdown 渲染
│   ├── diff.rs                    Diff 样式
│   ├── prompt_input.rs            输入行
│   ├── spinner.rs                 动画 spinner
│   ├── permissions.rs             权限对话框
│   ├── vim.rs                     Vim 模式
│   └── welcome.rs                 欢迎屏幕
│
├── mcp/                           MCP 集成
│   ├── protocol.rs                JSON-RPC 2.0 类型
│   ├── client.rs                  单服务器连接
│   ├── manager.rs                 多服务器管理
│   ├── discovery.rs               服务器发现
│   └── tools.rs                   Tool trait 适配
│
├── plugins/                       插件系统
│   ├── mod.rs                     注册表
│   ├── manifest.rs                plugin.json 解析
│   └── loader.rs                  安装/发现
│
├── teams/                         多 Agent Teams
│   ├── mod.rs                     核心类型
│   ├── context.rs                 task_local 上下文
│   ├── mailbox.rs                 邮箱 IPC
│   ├── helpers.rs                 团队管理
│   ├── protocol.rs                协议消息
│   ├── constants.rs               常量
│   ├── backend.rs                 执行后端 trait
│   ├── in_process.rs              进程内后端
│   └── runner.rs                  进程内运行器
│
├── services/                      后台服务
│   ├── tool_use_summary.rs        工具摘要
│   ├── session_memory.rs          会话记忆
│   ├── prompt_suggestion.rs       提示建议
│   └── lsp_lifecycle.rs           LSP 管理
│
├── lsp_service/                   LSP 服务层
│   └── mod.rs                     服务器配置 + 接口
│
└── utils/                         工具函数
    ├── tokens.rs                  Token 估算
    ├── file_state_cache.rs        LRU 文件缓存
    ├── abort.rs                   AbortController
    ├── bash.rs                    Shell 解析
    ├── git.rs                     Git 操作
    ├── cwd.rs                     工作目录
    ├── shell.rs                   Shell 检测
    └── messages.rs                消息工具
```
