#![allow(dead_code)]
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use super::app_state::AppState;
#[allow(unused_imports)]
use cc_types::message::{AssistantMessage, ContentBlock, Message, ToolResultContent};

/// Async callback for interactive permission requests.
///
/// Called with (tool_use_id, tool_name, description, options).
/// Returns the user's decision: "allow", "deny", or "always_allow".
pub type PermissionCallback = Arc<
    dyn Fn(String, String, String, Vec<String>) -> Pin<Box<dyn Future<Output = String> + Send>>
        + Send
        + Sync,
>;

/// Async callback for interactive "ask the user" tool requests.
///
/// Called with the plain-text question and resolves to the user's answer.
pub type AskUserCallback =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

/// 工具输入验证结果
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Ok,
    Error { message: String, error_code: i32 },
}

/// 权限检查结果
#[derive(Debug, Clone)]
pub enum PermissionResult {
    /// 允许执行, 可能更新输入
    Allow { updated_input: Value },
    /// 拒绝执行
    Deny { message: String },
    /// 需要用户确认
    Ask { message: String },
}

/// 工具执行结果
#[derive(Debug, Clone, Default)]
pub struct ToolResult {
    /// 工具输出数据 (legacy: 仅用于兼容和日志)
    pub data: Value,
    /// 结构化内容块, 用于回传给模型 (支持图片等多模态内容)
    ///
    /// 当 `Some`, query loop 使用此字段构建 `ToolResultContent::Blocks(...)`;
    /// 当 `None`, 退化为 `ToolResultContent::Text(data.to_string())`.
    pub model_content: Option<ToolResultContent>,
    /// 人类可读的预览文本 (用于 UI/日志, 不含图片二进制数据)
    pub display_preview: Option<String>,
    /// 额外产生的消息 (如子代理对话)
    pub new_messages: Vec<Message>,
}

impl ToolResult {
    /// Create a tool result with structured multimodal content for the model.
    pub fn with_content(
        data: Value,
        model_content: ToolResultContent,
        display_preview: String,
    ) -> Self {
        Self {
            data,
            model_content: Some(model_content),
            display_preview: Some(display_preview),
            new_messages: vec![],
        }
    }
}

/// 工具执行进度回调的数据
#[derive(Debug, Clone)]
pub struct ToolProgress {
    pub tool_use_id: String,
    pub data: Value,
}

// Permission-context types moved into `cc-types::permissions` in Phase 4
// (issue #73) so workspace crates like cc-sandbox and cc-permissions can
// consult them without a reverse dep on the root crate. Re-exported here
// so existing `crate::types::tool::{PermissionMode, …}` paths resolve.
// `ToolPermissionRulesBySource` is a pub alias — re-export it too so any
// future consumer in the root crate can still reach it via the classic
// `crate::types::tool::` path.
#[allow(unused_imports)]
pub use cc_types::permissions::ToolPermissionRulesBySource;
pub use cc_types::permissions::{
    AdditionalWorkingDirectory, PermissionMode, ToolPermissionContext,
};

/// 文件状态缓存 (LRU, 追踪工具已读/已写的文件)
#[derive(Debug, Clone, Default)]
pub struct FileStateCache {
    // 简化版: 后续用 lru crate 替换
    pub entries: HashMap<String, FileCacheEntry>,
}

#[derive(Debug, Clone)]
pub struct FileCacheEntry {
    pub content_hash: u64,
    pub last_read_timestamp: i64,
}

/// 工具使用上下文 — 工具执行时的完整环境
///
/// 对应 TypeScript: ToolUseContext
/// 这是传递给每个工具 call() 的主要参数
pub struct ToolUseContext {
    pub options: ToolUseOptions,
    pub abort_signal: tokio::sync::watch::Receiver<bool>,
    pub read_file_state: FileStateCache,
    pub get_app_state: Arc<dyn Fn() -> AppState + Send + Sync>,
    pub set_app_state: Arc<dyn Fn(Box<dyn FnOnce(AppState) -> AppState>) + Send + Sync>,
    pub session_id: String,
    pub langfuse_session_id: String,
    pub messages: Vec<Message>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub query_tracking: Option<QueryChainTracking>,
    /// Async callback for interactive permission prompts (headless/TUI mode).
    /// When set and a tool requires `Ask` permission, this callback is invoked
    /// instead of immediately denying. If `None`, `Ask` falls back to deny.
    pub permission_callback: Option<PermissionCallback>,
    /// Async callback for interactive AskUserQuestion prompts (headless/TUI mode).
    /// When set, AskUserQuestion routes through the frontend instead of reading stdin.
    pub ask_user_callback: Option<AskUserCallback>,
    /// Sender for background agent completion results.
    /// When `Some`, the Agent tool can spawn background tasks.
    /// When `None`, `run_in_background` falls back to synchronous execution.
    pub bg_agent_tx: Option<cc_types::agent_channel::AgentSender>,
    /// Hook runner used by tools (e.g. the Agent tool fires SubagentStart /
    /// SubagentStop events through this trait rather than importing
    /// `crate::tools::hooks` directly).
    pub hook_runner: Arc<dyn cc_types::hooks::HookRunner>,
    /// Command dispatcher — propagated to child engines spawned by the Agent
    /// tool so they inherit the same slash-command registry.
    pub command_dispatcher: Arc<dyn cc_types::commands::CommandDispatcher>,
}

/// 工具使用选项 (不可变配置)
#[derive(Debug, Clone)]
pub struct ToolUseOptions {
    pub debug: bool,
    pub main_loop_model: String,
    pub verbose: bool,
    pub is_non_interactive_session: bool,
    pub custom_system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub max_budget_usd: Option<f64>,
}

/// 查询链跟踪
#[derive(Debug, Clone)]
pub struct QueryChainTracking {
    pub chain_id: String,
    pub depth: usize,
}

/// Tool trait — 所有工具必须实现
///
/// 对应 TypeScript: Tool 接口 (src/Tool.ts)
///
/// 注意: 渲染相关方法 (renderToolUseMessage, renderToolResultMessage 等)
/// 在 Rust 版本中分离到 ui::ToolRenderer trait
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述 (可根据输入动态生成)
    async fn description(&self, input: &Value) -> String;

    /// 输入 JSON Schema
    fn input_json_schema(&self) -> Value;

    /// 是否启用
    fn is_enabled(&self) -> bool {
        true
    }

    /// 是否并发安全
    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    /// 是否只读
    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    /// 是否破坏性
    fn is_destructive(&self, _input: &Value) -> bool {
        false
    }

    /// 输入验证
    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        ValidationResult::Ok
    }

    /// 权限检查
    async fn check_permissions(&self, input: &Value, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Allow {
            updated_input: input.clone(),
        }
    }

    /// 执行工具
    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        parent_message: &AssistantMessage,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult>;

    /// 系统提示词片段
    async fn prompt(&self) -> String;

    /// 用户可见的工具名称
    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        self.name().to_string()
    }

    /// 工具结果最大字符数 (超过则持久化到磁盘)
    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    /// 获取操作的文件路径 (如果适用)
    fn get_path(&self, _input: &Value) -> Option<String> {
        None
    }

    /// 中断行为: cancel (中断) 或 block (等待完成)
    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    /// 自动分类器输入 (安全相关)
    fn to_auto_classifier_input(&self, _input: &Value) -> Value {
        Value::String(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptBehavior {
    Cancel,
    Block,
}

/// 工具集合类型
pub type Tools = Vec<Arc<dyn Tool>>;
