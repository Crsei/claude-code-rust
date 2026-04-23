#![allow(dead_code)]
#[allow(unused_imports)]
use super::tool::{QueryChainTracking, ToolUseContext, Tools};
#[allow(unused_imports)]
use cc_types::message::{Message, SystemMessage, Usage};

/// Thinking/extended-thinking configuration.
///
/// Controls whether the model produces `thinking` content blocks.
#[derive(Debug, Clone)]
pub enum ThinkingConfig {
    /// Thinking is disabled entirely.
    Disabled,
    /// Adaptive: the model decides when to think.
    Adaptive,
    /// Thinking is enabled with an optional token budget.
    Enabled { budget_tokens: Option<usize> },
}

/// 查询配置 — 每次 query() 调用时快照一次
///
/// 对应 TypeScript: query/config.ts 的 QueryConfig
#[derive(Debug, Clone)]
pub struct QueryConfig {
    pub session_id: String,
    pub gates: QueryGates,
}

/// 运行时特性开关 (env/statsig 快照)
#[derive(Debug, Clone)]
pub struct QueryGates {
    /// 流式工具执行 (边流式边执行已完成的工具)
    pub streaming_tool_execution: bool,
    /// 产出工具使用摘要
    pub emit_tool_use_summaries: bool,
    /// 快速模式
    pub fast_mode_enabled: bool,
}

/// query() 函数的参数
///
/// 对应 TypeScript: query.ts 的 QueryParams
pub struct QueryParams {
    pub messages: Vec<Message>,
    pub system_prompt: Vec<String>,
    pub user_context: std::collections::HashMap<String, String>,
    pub system_context: std::collections::HashMap<String, String>,
    pub fallback_model: Option<String>,
    pub query_source: QuerySource,
    pub max_output_tokens_override: Option<usize>,
    pub max_turns: Option<usize>,
    pub skip_cache_write: Option<bool>,
    pub task_budget: Option<TaskBudget>,
}

/// 查询来源
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuerySource {
    Sdk,
    ReplMainThread,
    Compact,
    SessionMemory,
    Agent(String),
    ProactiveTick,
    WebhookEvent,
    ChannelNotification,
}

impl QuerySource {
    pub fn as_str(&self) -> &str {
        match self {
            QuerySource::Sdk => "sdk",
            QuerySource::ReplMainThread => "repl_main_thread",
            QuerySource::Compact => "compact",
            QuerySource::SessionMemory => "session_memory",
            #[allow(unused_variables)]
            QuerySource::Agent(id) => "agent:", // 简化
            QuerySource::ProactiveTick => "proactive_tick",
            QuerySource::WebhookEvent => "webhook_event",
            QuerySource::ChannelNotification => "channel_notification",
        }
    }

    pub fn starts_with_agent(&self) -> bool {
        matches!(self, QuerySource::Agent(_))
    }

    pub fn is_autonomous(&self) -> bool {
        matches!(
            self,
            QuerySource::ProactiveTick
                | QuerySource::WebhookEvent
                | QuerySource::ChannelNotification
        )
    }
}

/// 任务预算
#[derive(Debug, Clone)]
pub struct TaskBudget {
    pub total: u64,
}

/// QueryEngine 配置
///
/// 对应 TypeScript: QueryEngine.ts 的 QueryEngineConfig
#[derive(Clone)]
pub struct QueryEngineConfig {
    pub cwd: String,
    pub tools: Tools,
    pub custom_system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub user_specified_model: Option<String>,
    pub fallback_model: Option<String>,
    pub max_turns: Option<usize>,
    pub max_budget_usd: Option<f64>,
    pub task_budget: Option<TaskBudget>,
    pub verbose: bool,
    pub initial_messages: Option<Vec<Message>>,

    // ── New fields (session lifecycle) ──────────────────────────────────
    /// Registered slash commands (placeholder: names only).
    pub commands: Vec<String>,

    /// Thinking / extended-thinking configuration.
    pub thinking_config: Option<ThinkingConfig>,

    /// JSON schema for structured output mode.
    pub json_schema: Option<serde_json::Value>,

    /// Whether to replay user messages back to SDK consumers.
    pub replay_user_messages: bool,

    /// Whether to persist the session to disk.
    pub persist_session: bool,

    /// Resolved model name (from CLI > config/env > provider default).
    /// Used to initialize AppState.main_loop_model.
    pub resolved_model: Option<String>,

    /// Automatically save session to disk after each assistant turn.
    /// Default: true.
    pub auto_save_session: bool,

    /// Sub-agent context — propagated from parent engine to child tools
    /// so that nested agents can enforce recursion depth limits.
    pub agent_context: Option<AgentContext>,
}

/// Context for sub-agent engines, propagated from parent QueryEngine.
///
/// When the Agent tool spawns a child engine, it sets this on the child's
/// `QueryEngineConfig` so that `execute_tool()` can propagate the correct
/// `agent_id` and `depth` into every `ToolUseContext`.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Unique ID of this agent instance.
    pub agent_id: String,
    /// Chain tracking for recursion depth enforcement.
    pub query_tracking: QueryChainTracking,
    /// Root Langfuse session ID inherited from the parent agent chain.
    pub langfuse_session_id: String,
    /// Sub-agent role/type used for telemetry naming.
    pub agent_type: Option<String>,
}
