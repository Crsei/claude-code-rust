/// query() 的 I/O 依赖 — 可在测试中 mock
///
/// 对应 TypeScript: query/deps.ts 的 QueryDeps
///
/// 使用 trait 而非 struct-of-functions, 更 Rust-idiomatic
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use serde_json::Value;

use crate::types::app_state::AppState;
use crate::types::message::{AssistantMessage, Message, StreamEvent, Usage};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, ToolResult, Tools};

/// API 调用的结果 — 从流收集后的完整助手响应
#[derive(Debug, Clone)]
pub struct ModelResponse {
    /// 助手消息 (包含文本和工具调用块)
    pub assistant_message: AssistantMessage,
    /// 流事件 (用于透传给调用方)
    #[allow(dead_code)]
    pub stream_events: Vec<StreamEvent>,
    /// 总用量
    #[allow(dead_code)]
    pub usage: Usage,
}

/// 压缩结果
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub messages: Vec<Message>,
    pub tracking: AutoCompactTracking,
}

/// 工具执行请求
#[derive(Debug, Clone)]
pub struct ToolExecRequest {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: Value,
    pub langfuse_batch_span: Option<crate::services::langfuse::LangfuseSpan>,
}

/// 工具执行结果 (附带原始 tool_use_id 以便关联)
#[derive(Debug, Clone)]
pub struct ToolExecResult {
    pub tool_use_id: String,
    pub tool_name: String,
    pub result: ToolResult,
    pub is_error: bool,
}

/// ModelCallParams — 调用模型时需要的全部参数
#[derive(Clone)]
pub struct ModelCallParams {
    pub messages: Vec<Message>,
    pub system_prompt: Vec<String>,
    pub tools: Tools,
    pub model: Option<String>,
    pub max_output_tokens: Option<usize>,
    pub skip_cache_write: Option<bool>,
    pub thinking_enabled: Option<bool>,
    pub effort_value: Option<String>,
}

impl std::fmt::Debug for ModelCallParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCallParams")
            .field("messages_count", &self.messages.len())
            .field("system_prompt", &self.system_prompt)
            .field("tools_count", &self.tools.len())
            .field("model", &self.model)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("skip_cache_write", &self.skip_cache_write)
            .field("thinking_enabled", &self.thinking_enabled)
            .field("effort_value", &self.effort_value)
            .finish()
    }
}

#[async_trait::async_trait]
pub trait QueryDeps: Send + Sync {
    /// 调用模型 (流式)
    ///
    /// 返回一个流, 产出 StreamEvent (用于实时中继给 UI),
    /// 以及最终的 ModelResponse (通过 await 完成收集).
    ///
    /// 在生产中委托给 api/client.rs
    /// 在测试中返回预设的消息序列
    async fn call_model(&self, params: ModelCallParams) -> Result<ModelResponse>;

    /// 调用模型 (流式, 返回 Stream 供实时中继)
    ///
    /// 默认实现: 调用 call_model 后一次性返回所有事件.
    /// 生产实现应当返回真正的流.
    async fn call_model_streaming(
        &self,
        params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;

    /// 微压缩: 裁剪过大的工具结果
    async fn microcompact(&self, messages: Vec<Message>) -> Result<Vec<Message>>;

    /// 自动压缩: 达到 token 阈值时压缩历史
    async fn autocompact(
        &self,
        messages: Vec<Message>,
        tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>>;

    /// 响应式压缩: prompt_too_long 恢复时调用
    async fn reactive_compact(&self, messages: Vec<Message>) -> Result<Option<CompactionResult>>;

    /// 执行单个工具
    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &AssistantMessage,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult>;

    /// 获取当前 app state
    fn get_app_state(&self) -> AppState;

    /// 生成 UUID (可在测试中确定化)
    fn uuid(&self) -> String;

    /// 检查 abort 信号
    fn is_aborted(&self) -> bool;

    /// 获取当前 tools (可能在每轮迭代间刷新)
    fn get_tools(&self) -> Tools;

    /// 刷新工具列表 (MCP 工具等可能动态变化)
    async fn refresh_tools(&self) -> Result<Tools>;

    /// Drain completed background agent results (called at turn start).
    /// Default: returns empty vec (no background agent support).
    fn drain_background_results(
        &self,
    ) -> Vec<cc_types::background_agents::CompletedBackgroundAgent> {
        vec![]
    }

    /// Hook runner for the query loop (PreCompact / PostCompact / Stop /
    /// StopFailure events). Defaults to a no-op runner so tests using the
    /// default trait impl don't need to provide one.
    fn hook_runner(&self) -> Arc<dyn cc_types::hooks::HookRunner> {
        Arc::new(cc_types::hooks::NoopHookRunner)
    }

    /// Get the audit context for this submit.
    fn audit_context(&self) -> crate::observability::AuditContext {
        crate::observability::AuditContext::noop("unknown")
    }

    /// Root Langfuse trace for the current submit, if Langfuse is enabled.
    fn langfuse_trace(&self) -> Option<crate::services::langfuse::LangfuseTrace> {
        None
    }

    /// Provider label used in Langfuse observation metadata.
    fn langfuse_provider_name(&self) -> Option<String> {
        None
    }
}
