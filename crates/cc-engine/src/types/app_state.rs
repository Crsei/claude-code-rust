use super::tool::{PermissionMode, ToolPermissionContext};
use std::collections::HashMap;

/// Runtime settings projection — moved to `cc-config` in Phase 3 (issue #72).
///
/// Re-exported here so existing `crate::types::app_state::SettingsJson`
/// call sites keep compiling.
pub use cc_config::runtime_settings::SettingsJson;

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
    /// Active backend implementation ("native" or "codex").
    pub main_loop_backend: String,
    /// 工具权限上下文
    pub tool_permission_context: ToolPermissionContext,
    /// thinking 是否启用
    pub thinking_enabled: Option<bool>,
    /// 快速模式
    pub fast_mode: bool,
    /// effort 值
    pub effort_value: Option<String>,
    /// Agent Teams 上下文 (feature-gated)
    pub team_context: Option<cc_types::teams::TeamContext>,
    /// Hook configurations loaded from settings.json (merged config).
    /// Read by `tools::hooks::load_hook_configs()` and the hook execution pipeline.
    pub hooks: HashMap<String, serde_json::Value>,
    /// Whether KAIROS daemon mode is running
    pub kairos_active: bool,
    /// Whether output is routed through BriefTool only
    pub is_brief_only: bool,
    /// Perpetual session mode
    pub is_assistant_mode: bool,
    /// Proactive tick interval (None = disabled)
    pub autonomous_tick_ms: Option<u64>,
    /// Whether user is looking at terminal (affects autonomy level)
    pub terminal_focus: bool,
    /// Shared keybinding registry (default + user, with hot reload).
    ///
    /// Populated at startup from `~/.cc-rust/keybindings.json` (issue #10).
    /// Multiple UI surfaces (Rust TUI, IPC-driven OpenTUI) share the same
    /// handle so reloads are observed everywhere.
    pub keybindings: cc_keybindings::KeybindingRegistry,
    /// Shared scriptable status-line runner (issue #11). The TUI owns the
    /// renderer-facing side; `/statusline` and the IPC driver both reach
    /// into this handle to inspect / reset the subprocess.
    pub status_line_runner: crate::status_line::StatusLineRunner,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: SettingsJson::default(),
            verbose: false,
            main_loop_model: "claude-sonnet-4-20250514".to_string(),
            main_loop_backend: "native".to_string(),
            tool_permission_context: ToolPermissionContext {
                mode: PermissionMode::Default,
                additional_working_directories: HashMap::new(),
                always_allow_rules: HashMap::new(),
                always_deny_rules: HashMap::new(),
                always_ask_rules: HashMap::new(),
                session_allow_rules: HashMap::new(),
                is_bypass_permissions_mode_available: false,
                is_auto_mode_available: None,
                pre_plan_mode: None,
            },
            thinking_enabled: None,
            fast_mode: false,
            effort_value: None,
            team_context: None,
            hooks: HashMap::new(),
            kairos_active: false,
            is_brief_only: false,
            is_assistant_mode: false,
            autonomous_tick_ms: None,
            terminal_focus: true,
            keybindings: cc_keybindings::KeybindingRegistry::with_defaults(),
            status_line_runner: crate::status_line::StatusLineRunner::new(),
        }
    }
}
