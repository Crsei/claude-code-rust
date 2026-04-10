use super::tool::{PermissionMode, ToolPermissionContext};
use std::collections::HashMap;

/// 应用全局状态 (简化版)
///
/// 对应 TypeScript: state/AppState.ts
/// 在 TypeScript 中通过 React context + DeepImmutable 管理
/// 在 Rust 中通过 Arc<RwLock<AppState>> 管理
#[derive(Debug, Clone)]
pub struct AppState {
    /// 当前设置
    pub settings: SettingsJson,
    /// 详细模式
    pub verbose: bool,
    /// 主循环模型
    pub main_loop_model: String,
    /// 工具权限上下文
    pub tool_permission_context: ToolPermissionContext,
    /// thinking 是否启用
    pub thinking_enabled: Option<bool>,
    /// 快速模式
    pub fast_mode: bool,
    /// effort 值
    pub effort_value: Option<String>,
    /// Agent Teams 上下文 (feature-gated)
    pub team_context: Option<crate::teams::types::TeamContext>,
    /// Hook configurations loaded from settings.json (merged config).
    /// Read by `tools::hooks::load_hook_configs()` and the hook execution pipeline.
    pub hooks: HashMap<String, serde_json::Value>,
}

/// 设置 JSON (简化版)
#[derive(Debug, Clone, Default)]
pub struct SettingsJson {
    pub model: Option<String>,
    pub theme: Option<String>,
    pub verbose: Option<bool>,
    // 后续添加更多设置字段
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: SettingsJson::default(),
            verbose: false,
            main_loop_model: "claude-sonnet-4-20250514".to_string(),
            tool_permission_context: ToolPermissionContext {
                mode: PermissionMode::Default,
                additional_working_directories: HashMap::new(),
                always_allow_rules: HashMap::new(),
                always_deny_rules: HashMap::new(),
                always_ask_rules: HashMap::new(),
                is_bypass_permissions_mode_available: false,
                is_auto_mode_available: None,
                pre_plan_mode: None,
            },
            thinking_enabled: None,
            fast_mode: false,
            effort_value: None,
            team_context: None,
            hooks: HashMap::new(),
        }
    }
}
