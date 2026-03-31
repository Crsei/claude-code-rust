use super::message::{Message, SystemMessage, Usage};
use super::tool::{ToolUseContext, Tools, QueryChainTracking};

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
}

impl QuerySource {
    pub fn as_str(&self) -> &str {
        match self {
            QuerySource::Sdk => "sdk",
            QuerySource::ReplMainThread => "repl_main_thread",
            QuerySource::Compact => "compact",
            QuerySource::SessionMemory => "session_memory",
            QuerySource::Agent(id) => "agent:", // 简化
        }
    }

    pub fn starts_with_agent(&self) -> bool {
        matches!(self, QuerySource::Agent(_))
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
}
