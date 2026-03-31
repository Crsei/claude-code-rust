/// QueryEngine — 拥有一个对话的完整生命周期
///
/// 对应 TypeScript: QueryEngine.ts
///
/// 职责:
/// - 持有可变消息历史
/// - 管理 abort 控制器
/// - 跟踪 usage (token 消耗, 成本)
/// - 跟踪权限拒绝
/// - 跟踪轮次计数
/// - `submit_message(prompt)` → 返回 Stream<QueryYield>
/// - 内部委托给 `query::query()`

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::Result;
use futures::Stream;
use tracing::info;
use uuid::Uuid;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::query::loop_impl;
use crate::types::app_state::AppState;
use crate::types::config::{QueryEngineConfig, QueryParams, QuerySource};
use crate::types::message::{
    Message, MessageContent, QueryYield, StreamEvent, Usage, UserMessage,
};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

/// 使用量追踪
#[derive(Debug, Clone, Default)]
pub struct UsageTracking {
    /// 总输入 token
    pub total_input_tokens: u64,
    /// 总输出 token
    pub total_output_tokens: u64,
    /// 总缓存读取 token
    pub total_cache_read_tokens: u64,
    /// 总缓存创建 token
    pub total_cache_creation_tokens: u64,
    /// 总成本 (USD)
    pub total_cost_usd: f64,
    /// API 调用次数
    pub api_call_count: u64,
}

impl UsageTracking {
    pub fn add_usage(&mut self, usage: &Usage, cost_usd: f64) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
        self.total_cache_read_tokens += usage.cache_read_input_tokens;
        self.total_cache_creation_tokens += usage.cache_creation_input_tokens;
        self.total_cost_usd += cost_usd;
        self.api_call_count += 1;
    }
}

/// 权限拒绝记录
#[derive(Debug, Clone)]
pub struct PermissionDenial {
    pub tool_name: String,
    pub tool_use_id: String,
    pub reason: String,
    pub timestamp: i64,
}

/// QueryEngine — 单个对话的控制器
///
/// 每个对话(session)创建一个 QueryEngine 实例.
/// 它是 query loop 的上层包装, 负责状态管理和 lifecycle.
pub struct QueryEngine {
    /// 对话 ID
    pub session_id: String,

    /// 配置
    config: QueryEngineConfig,

    /// 对话消息历史 (可变)
    messages: Arc<RwLock<Vec<Message>>>,

    /// Abort 控制: true = 已中止
    aborted: Arc<AtomicBool>,

    /// 使用量跟踪
    usage: Arc<Mutex<UsageTracking>>,

    /// 权限拒绝历史
    permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,

    /// 总轮次计数 (跨多次 submit)
    total_turn_count: Arc<Mutex<usize>>,

    /// 应用状态
    app_state: Arc<RwLock<AppState>>,

    /// 当前工具列表
    tools: Arc<RwLock<Tools>>,
}

impl QueryEngine {
    /// 创建新的 QueryEngine
    pub fn new(config: QueryEngineConfig) -> Self {
        let initial_messages = config.initial_messages.clone().unwrap_or_default();
        let tools = config.tools.clone();

        Self {
            session_id: Uuid::new_v4().to_string(),
            config,
            messages: Arc::new(RwLock::new(initial_messages)),
            aborted: Arc::new(AtomicBool::new(false)),
            usage: Arc::new(Mutex::new(UsageTracking::default())),
            permission_denials: Arc::new(Mutex::new(Vec::new())),
            total_turn_count: Arc::new(Mutex::new(0)),
            app_state: Arc::new(RwLock::new(AppState::default())),
            tools: Arc::new(RwLock::new(tools)),
        }
    }

    /// 提交用户消息, 返回响应流
    ///
    /// 这是 QueryEngine 的主要入口点.
    /// 调用方消费返回的 Stream 来驱动 UI 更新.
    ///
    /// # Arguments
    /// * `prompt` - 用户输入的文本
    /// * `query_source` - 查询来源 (REPL, SDK, Agent 等)
    pub fn submit_message(
        &self,
        prompt: &str,
        query_source: QuerySource,
    ) -> impl Stream<Item = QueryYield> {
        info!(prompt_len = prompt.len(), source = ?query_source, "submitting message");

        // 创建用户消息
        let user_message = UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "user".to_string(),
            content: MessageContent::Text(prompt.to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        };

        // 加入消息历史
        {
            let mut messages = self.messages.write().unwrap();
            messages.push(Message::User(user_message));
        }

        // 构建查询参数
        let messages = self.messages.read().unwrap().clone();
        let system_prompt = self.build_system_prompt();

        let params = QueryParams {
            messages,
            system_prompt,
            user_context: HashMap::new(),
            system_context: HashMap::new(),
            fallback_model: self.config.fallback_model.clone(),
            query_source,
            max_output_tokens_override: None,
            max_turns: self.config.max_turns,
            skip_cache_write: None,
            task_budget: self.config.task_budget.clone(),
        };

        // 创建 deps 实现
        let deps = Arc::new(QueryEngineDeps {
            aborted: self.aborted.clone(),
            app_state: self.app_state.clone(),
            tools: self.tools.clone(),
        });

        // 委托给 query loop
        let stream = loop_impl::query(params, deps);

        // 包装 stream, 拦截消息以更新内部状态
        let messages_ref = self.messages.clone();
        let usage_ref = self.usage.clone();
        let turn_count_ref = self.total_turn_count.clone();

        async_stream::stream! {
            use futures::StreamExt;
            let mut stream = std::pin::pin!(stream);

            while let Some(item) = stream.next().await {
                // 拦截消息以更新内部状态
                match &item {
                    QueryYield::Message(msg) => {
                        // 更新消息历史
                        messages_ref.write().unwrap().push(msg.clone());

                        // 更新 usage
                        if let Message::Assistant(assistant) = msg {
                            if let Some(ref usage) = assistant.usage {
                                usage_ref.lock().unwrap().add_usage(usage, assistant.cost_usd);
                            }
                        }
                    }
                    QueryYield::RequestStart(_) => {
                        *turn_count_ref.lock().unwrap() += 1;
                    }
                    _ => {}
                }

                yield item;
            }
        }
    }

    /// 中止当前查询
    pub fn abort(&self) {
        info!("aborting query engine");
        self.aborted.store(true, Ordering::SeqCst);
    }

    /// 重置 abort 状态 (在新的 submit 前调用)
    pub fn reset_abort(&self) {
        self.aborted.store(false, Ordering::SeqCst);
    }

    /// 获取当前消息历史
    pub fn messages(&self) -> Vec<Message> {
        self.messages.read().unwrap().clone()
    }

    /// 获取使用量跟踪
    pub fn usage(&self) -> UsageTracking {
        self.usage.lock().unwrap().clone()
    }

    /// 获取权限拒绝历史
    pub fn permission_denials(&self) -> Vec<PermissionDenial> {
        self.permission_denials.lock().unwrap().clone()
    }

    /// 记录权限拒绝
    pub fn record_permission_denial(&self, denial: PermissionDenial) {
        self.permission_denials.lock().unwrap().push(denial);
    }

    /// 获取总轮次计数
    pub fn total_turn_count(&self) -> usize {
        *self.total_turn_count.lock().unwrap()
    }

    /// 获取当前 app state
    pub fn app_state(&self) -> AppState {
        self.app_state.read().unwrap().clone()
    }

    /// 更新 app state
    pub fn update_app_state<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        let mut state = self.app_state.write().unwrap();
        updater(&mut state);
    }

    /// 更新工具列表
    pub fn set_tools(&self, tools: Tools) {
        *self.tools.write().unwrap() = tools;
    }

    /// 构建系统提示
    fn build_system_prompt(&self) -> Vec<String> {
        let mut prompts = Vec::new();

        // 自定义系统提示
        if let Some(ref custom) = self.config.custom_system_prompt {
            prompts.push(custom.clone());
        }

        // 追加系统提示
        if let Some(ref append) = self.config.append_system_prompt {
            prompts.push(append.clone());
        }

        // 如果没有自定义提示, 使用默认
        if prompts.is_empty() {
            prompts.push(
                "You are Claude, an AI assistant by Anthropic. \
                 You are helpful, harmless, and honest."
                    .to_string(),
            );
        }

        prompts
    }
}

// ──────────────────────────────────────────────────────────
// QueryEngineDeps — QueryEngine 的 QueryDeps 实现
// ──────────────────────────────────────────────────────────

/// QueryEngine 提供给 query loop 的依赖注入实现
///
/// Phase 1: 大部分方法是 stub, 后续阶段逐步接入真实实现
struct QueryEngineDeps {
    aborted: Arc<AtomicBool>,
    app_state: Arc<RwLock<AppState>>,
    tools: Arc<RwLock<Tools>>,
}

#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
    async fn call_model(&self, _params: ModelCallParams) -> Result<ModelResponse> {
        // Phase 1 stub: 返回一个占位响应
        // 后续阶段接入真实 API 客户端
        anyhow::bail!(
            "call_model not yet implemented — \
             wire up a real API client or use a mock in tests"
        )
    }

    async fn call_model_streaming(
        &self,
        _params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        anyhow::bail!(
            "call_model_streaming not yet implemented — \
             wire up a real API client or use a mock in tests"
        )
    }

    async fn microcompact(&self, messages: Vec<Message>) -> Result<Vec<Message>> {
        // Phase 1 stub: 透传
        Ok(messages)
    }

    async fn autocompact(
        &self,
        _messages: Vec<Message>,
        _tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        // Phase 1 stub: 不压缩
        Ok(None)
    }

    async fn reactive_compact(
        &self,
        _messages: Vec<Message>,
    ) -> Result<Option<CompactionResult>> {
        Ok(None)
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &crate::types::message::AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        // 查找工具
        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool_name)
            .ok_or_else(|| {
                anyhow::anyhow!("tool not found: {}", request.tool_name)
            })?;

        // 构建 ToolUseContext (简化版)
        let ctx = crate::types::tool::ToolUseContext {
            options: crate::types::tool::ToolUseOptions {
                debug: false,
                main_loop_model: self
                    .app_state
                    .read()
                    .unwrap()
                    .main_loop_model
                    .clone(),
                verbose: self.app_state.read().unwrap().verbose,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: {
                let (tx, rx) = tokio::sync::watch::channel(false);
                // 如果已 abort, 立即发送
                if self.aborted.load(Ordering::Relaxed) {
                    let _ = tx.send(true);
                }
                rx
            },
            read_file_state: crate::types::tool::FileStateCache::default(),
            get_app_state: {
                let state = self.app_state.clone();
                Arc::new(move || state.read().unwrap().clone())
            },
            set_app_state: {
                let state = self.app_state.clone();
                Arc::new(move |updater: Box<dyn FnOnce(AppState) -> AppState>| {
                    let mut s = state.write().unwrap();
                    let old = s.clone();
                    *s = updater(old);
                })
            },
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        };

        // 执行工具
        match tool.call(request.input, &ctx, parent_message, None).await {
            Ok(result) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result,
                is_error: false,
            }),
            Err(e) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: crate::types::tool::ToolResult {
                    data: serde_json::json!(format!("Error: {}", e)),
                    new_messages: vec![],
                },
                is_error: true,
            }),
        }
    }

    fn get_app_state(&self) -> AppState {
        self.app_state.read().unwrap().clone()
    }

    fn uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        self.tools.read().unwrap().clone()
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        // Phase 1: 工具列表不变
        Ok(self.tools.read().unwrap().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool::Tools;

    fn make_config() -> QueryEngineConfig {
        QueryEngineConfig {
            cwd: "/tmp".to_string(),
            tools: vec![],
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: None,
            fallback_model: None,
            max_turns: None,
            max_budget_usd: None,
            task_budget: None,
            verbose: false,
            initial_messages: None,
        }
    }

    #[test]
    fn test_query_engine_creation() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.messages().len(), 0);
        assert_eq!(engine.total_turn_count(), 0);
        assert!(engine.usage().total_cost_usd == 0.0);
    }

    #[test]
    fn test_query_engine_abort() {
        let engine = QueryEngine::new(make_config());
        assert!(!engine.aborted.load(Ordering::Relaxed));
        engine.abort();
        assert!(engine.aborted.load(Ordering::Relaxed));
        engine.reset_abort();
        assert!(!engine.aborted.load(Ordering::Relaxed));
    }

    #[test]
    fn test_query_engine_app_state() {
        let engine = QueryEngine::new(make_config());
        let state = engine.app_state();
        assert!(!state.verbose);

        engine.update_app_state(|s| {
            s.verbose = true;
        });

        let state = engine.app_state();
        assert!(state.verbose);
    }

    #[test]
    fn test_query_engine_permission_denial() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.permission_denials().len(), 0);

        engine.record_permission_denial(PermissionDenial {
            tool_name: "Bash".to_string(),
            tool_use_id: "tu_1".to_string(),
            reason: "user denied".to_string(),
            timestamp: 0,
        });

        assert_eq!(engine.permission_denials().len(), 1);
    }

    #[test]
    fn test_build_system_prompt_default() {
        let engine = QueryEngine::new(make_config());
        let prompts = engine.build_system_prompt();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Claude"));
    }

    #[test]
    fn test_build_system_prompt_custom() {
        let mut config = make_config();
        config.custom_system_prompt = Some("You are a coding assistant.".to_string());
        config.append_system_prompt = Some("Always be concise.".to_string());

        let engine = QueryEngine::new(config);
        let prompts = engine.build_system_prompt();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].contains("coding assistant"));
        assert!(prompts[1].contains("concise"));
    }
}
