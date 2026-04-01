#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use super::app_state::AppState;
#[allow(unused_imports)]
use super::message::{AssistantMessage, Message, ContentBlock};

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
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// 工具输出数据
    pub data: Value,
    /// 额外产生的消息 (如子代理对话)
    pub new_messages: Vec<Message>,
}

/// 工具执行进度回调的数据
#[derive(Debug, Clone)]
pub struct ToolProgress {
    pub tool_use_id: String,
    pub data: Value,
}

/// 权限模式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionMode {
    /// 默认/询问模式: 需要用户确认
    Default,
    /// 自动模式: 自动批准 (带安全分类器)
    Auto,
    /// 绕过模式: 跳过所有权限检查
    Bypass,
    /// 计划模式: 只读, 不执行写入
    Plan,
}

/// 工具权限上下文
#[derive(Debug, Clone)]
pub struct ToolPermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: HashMap<String, AdditionalWorkingDirectory>,
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
    pub is_bypass_permissions_mode_available: bool,
    pub is_auto_mode_available: Option<bool>,
    /// 计划模式之前的权限模式 (用于恢复)
    pub pre_plan_mode: Option<PermissionMode>,
}

#[derive(Debug, Clone, Default)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub read_only: bool,
}

/// 权限规则, 按来源分组
pub type ToolPermissionRulesBySource = HashMap<String, Vec<String>>;

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
    pub messages: Vec<Message>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub query_tracking: Option<QueryChainTracking>,
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
    fn is_enabled(&self) -> bool { true }

    /// 是否并发安全
    fn is_concurrency_safe(&self, _input: &Value) -> bool { false }

    /// 是否只读
    fn is_read_only(&self, _input: &Value) -> bool { false }

    /// 是否破坏性
    fn is_destructive(&self, _input: &Value) -> bool { false }

    /// 输入验证
    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        ValidationResult::Ok
    }

    /// 权限检查
    async fn check_permissions(&self, input: &Value, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Allow { updated_input: input.clone() }
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
    fn max_result_size_chars(&self) -> usize { 100_000 }

    /// 获取操作的文件路径 (如果适用)
    fn get_path(&self, _input: &Value) -> Option<String> { None }

    /// 中断行为: cancel (中断) 或 block (等待完成)
    fn interrupt_behavior(&self) -> InterruptBehavior { InterruptBehavior::Block }

    /// 自动分类器输入 (安全相关)
    fn to_auto_classifier_input(&self, _input: &Value) -> Value { Value::String(String::new()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptBehavior {
    Cancel,
    Block,
}

/// 工具集合类型
pub type Tools = Vec<Arc<dyn Tool>>;
