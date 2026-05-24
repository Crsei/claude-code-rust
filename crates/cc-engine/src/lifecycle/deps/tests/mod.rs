use super::*;
use crate::lifecycle::QueryEngine;
use crate::types::config::QueryEngineConfig;
use crate::types::message::{AssistantMessage, MessageContent, ToolResultContent, UserMessage};
use crate::types::tool::{
    PermissionCallback, PermissionMode, PermissionResponsePayload, PermissionResult, Tool,
    ToolResult, ToolUseContext,
};
use serde_json::{json, Value};

struct CanonicalTool {
    name: &'static str,
    validation_error: Option<&'static str>,
    permission: Option<PermissionResult>,
    result: ToolResult,
    max_result_size_chars: usize,
    seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
    progress_payload: Option<Value>,
}

#[async_trait::async_trait]
impl Tool for CanonicalTool {
    fn name(&self) -> &str {
        self.name
    }

    async fn description(&self, _input: &Value) -> String {
        String::new()
    }

    fn input_json_schema(&self) -> Value {
        json!({})
    }

    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        if let Some(message) = self.validation_error {
            ValidationResult::Error {
                message: message.to_string(),
                error_code: 1,
            }
        } else {
            ValidationResult::Ok
        }
    }

    async fn check_permissions(&self, input: &Value, _ctx: &ToolUseContext) -> PermissionResult {
        self.permission
            .clone()
            .unwrap_or_else(|| PermissionResult::Allow {
                updated_input: input.clone(),
            })
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        *self.seen_input.lock() = Some(input);
        if let (Some(callback), Some(payload)) = (on_progress, self.progress_payload.clone()) {
            callback(ToolProgress {
                tool_use_id: String::new(),
                data: payload,
            });
        }
        Ok(self.result.clone())
    }

    async fn prompt(&self) -> String {
        String::new()
    }

    fn max_result_size_chars(&self) -> usize {
        self.max_result_size_chars
    }
}

struct FailingTool {
    name: &'static str,
    seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
}

#[async_trait::async_trait]
impl Tool for FailingTool {
    fn name(&self) -> &str {
        self.name
    }

    async fn description(&self, _input: &Value) -> String {
        String::new()
    }

    fn input_json_schema(&self) -> Value {
        json!({})
    }

    async fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        ValidationResult::Ok
    }

    async fn check_permissions(&self, input: &Value, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Allow {
            updated_input: input.clone(),
        }
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent_message: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        *self.seen_input.lock() = Some(input);
        Err(anyhow::anyhow!("tool failed for test"))
    }

    async fn prompt(&self) -> String {
        String::new()
    }
}

fn make_config(tools: Tools) -> QueryEngineConfig {
    QueryEngineConfig {
        cwd: ".".to_string(),
        tools,
        custom_system_prompt: None,
        append_system_prompt: None,
        user_specified_model: None,
        fallback_model: None,
        max_turns: None,
        max_budget_usd: None,
        task_budget: None,
        verbose: false,
        initial_messages: None,
        commands: vec![],
        thinking_config: None,
        json_schema: None,
        replay_user_messages: false,
        persist_session: false,
        resolved_model: None,
        auto_save_session: false,
        agent_context: None,
    }
}

fn make_deps(tools: Tools, mode: PermissionMode) -> QueryEngineDeps {
    let engine = QueryEngine::new(make_config(tools));
    engine.state.write().app_state.tool_permission_context.mode = mode;

    QueryEngineDeps {
        aborted: engine.aborted.clone(),
        state: engine.state.clone(),
        cwd: ".".to_string(),
        session_id: "test-session".to_string(),
        audit_ctx: crate::observability::AuditContext::noop("test"),
        langfuse_trace: None,
        api_client: None,
        agent_context: None,
        permission_callback: None,
        permission_event_callback: None,
        bg_agent_tx: None,
        tool_progress_callback: None,
        pending_bg_results: crate::agent_runtime::PendingBackgroundResults::new(),
        active_steer_state: engine.active_steer_state.clone(),
        hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
        command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
        auto_classifier_fn: None,
    }
}

fn parent_message() -> AssistantMessage {
    AssistantMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: 0,
        role: "assistant".to_string(),
        content: vec![],
        usage: None,
        stop_reason: None,
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    }
}

fn tool_request(tool_name: &str, input: Value) -> ToolExecRequest {
    ToolExecRequest {
        tool_use_id: "tu_test".to_string(),
        tool_name: tool_name.to_string(),
        input,
        langfuse_batch_span: None,
    }
}

fn user_message(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: 0,
        role: "user".to_string(),
        content: MessageContent::Text(text.to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

fn canonical_tool(
    name: &'static str,
    seen_input: Arc<parking_lot::Mutex<Option<Value>>>,
) -> Arc<CanonicalTool> {
    Arc::new(CanonicalTool {
        name,
        validation_error: None,
        permission: None,
        result: ToolResult {
            data: json!("ok"),
            new_messages: vec![],
            ..Default::default()
        },
        max_result_size_chars: 100_000,
        seen_input,
        progress_payload: None,
    })
}

struct FailingPreToolHookRunner {
    critical: bool,
}

#[async_trait::async_trait]
impl cc_types::hooks::HookRunner for FailingPreToolHookRunner {
    fn load_hook_configs(
        &self,
        _hooks_value: &cc_types::hooks::HooksMap,
        event_name: &str,
    ) -> Vec<cc_types::hooks::HookEventConfig> {
        if event_name == "PreToolUse" {
            vec![cc_types::hooks::HookEventConfig {
                matcher: Some("HookedTool".to_string()),
                critical: self.critical,
                hooks: vec![cc_types::hooks::HookEntry::Command {
                    command: "failing-test-hook".to_string(),
                    timeout: 1,
                    shell: None,
                    if_condition: None,
                }],
            }]
        } else {
            vec![]
        }
    }

    async fn run_pre_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PreToolHookResult> {
        Err(anyhow::anyhow!("pre hook failed for test"))
    }

    async fn run_post_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _tool_result_data: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
        Ok(cc_types::hooks::PostToolHookResult::Continue)
    }

    async fn run_post_tool_failure_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _error: &str,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run_event_hooks(
        &self,
        _event_name: &str,
        _payload: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::HookOutput> {
        Ok(cc_types::hooks::HookOutput::default())
    }

    async fn run_stop_hooks(
        &self,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
        Ok(cc_types::hooks::PostToolHookResult::Continue)
    }
}

struct FailingPostToolHookRunner {
    event_name: &'static str,
    critical: bool,
}

#[async_trait::async_trait]
impl cc_types::hooks::HookRunner for FailingPostToolHookRunner {
    fn load_hook_configs(
        &self,
        _hooks_value: &cc_types::hooks::HooksMap,
        event_name: &str,
    ) -> Vec<cc_types::hooks::HookEventConfig> {
        if event_name == self.event_name {
            vec![cc_types::hooks::HookEventConfig {
                matcher: Some("HookedTool".to_string()),
                critical: self.critical,
                hooks: vec![cc_types::hooks::HookEntry::Command {
                    command: "failing-test-hook".to_string(),
                    timeout: 1,
                    shell: None,
                    if_condition: None,
                }],
            }]
        } else {
            vec![]
        }
    }

    async fn run_pre_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PreToolHookResult> {
        Ok(cc_types::hooks::PreToolHookResult::Continue {
            updated_input: None,
            permission_override: None,
        })
    }

    async fn run_post_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _tool_result_data: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
        if self.event_name == "PostToolUse" {
            Err(anyhow::anyhow!("post hook failed for test"))
        } else {
            Ok(cc_types::hooks::PostToolHookResult::Continue)
        }
    }

    async fn run_post_tool_failure_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _error: &str,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<()> {
        if self.event_name == "PostToolUseFailure" {
            Err(anyhow::anyhow!("post failure hook failed for test"))
        } else {
            Ok(())
        }
    }

    async fn run_event_hooks(
        &self,
        _event_name: &str,
        _payload: &Value,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::HookOutput> {
        Ok(cc_types::hooks::HookOutput::default())
    }

    async fn run_stop_hooks(
        &self,
        _hook_configs: &[cc_types::hooks::HookEventConfig],
    ) -> anyhow::Result<cc_types::hooks::PostToolHookResult> {
        Ok(cc_types::hooks::PostToolHookResult::Continue)
    }
}

fn mcp_tool(server_name: &str, tool_name: &str) -> Arc<dyn Tool> {
    Arc::new(crate::mcp_tool_adapter::McpToolWrapper {
        def: cc_mcp::McpToolDef {
            name: tool_name.to_string(),
            description: format!("{server_name} tool"),
            input_schema: json!({"type": "object"}),
            server_name: server_name.to_string(),
        },
        server_name: server_name.to_string(),
        manager: Arc::new(tokio::sync::Mutex::new(cc_mcp::manager::McpManager::new())),
    })
}

#[test]
fn refreshed_mcp_merge_replaces_wrapped_tools_without_prefix_guessing() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let native_mcp_named_tool: Arc<dyn Tool> =
        canonical_tool("mcp__computer-use__screenshot", seen_input);
    let stale_mcp_tool = mcp_tool("old-server", "mcp__old-server__stale");
    let fresh_mcp_tool = mcp_tool("new-server", "mcp__new-server__fresh");

    let merged = merge_refreshed_mcp_tools(
        vec![native_mcp_named_tool, stale_mcp_tool],
        vec![fresh_mcp_tool],
    );
    let names = merged.iter().map(|tool| tool.name()).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["mcp__computer-use__screenshot", "mcp__new-server__fresh"]
    );
    assert_eq!(merged[0].mcp_server_name(), None);
    assert_eq!(merged[1].mcp_server_name(), Some("new-server"));
}

#[test]
fn exact_auto_compact_trigger_keeps_heuristic_when_report_missing() {
    assert!(exact_auto_compact_triggered(true, None));
    assert!(!exact_auto_compact_triggered(false, None));
}

#[test]
fn exact_auto_compact_trigger_overrides_near_threshold_heuristic() {
    let below = cc_utils::tokens::token_usage_report_from_count(
        159_000,
        "claude-sonnet-4-20250514",
        cc_utils::tokens::TokenCountMethod::ProviderExact,
        Some("test"),
    );
    let above = cc_utils::tokens::token_usage_report_from_count(
        161_000,
        "claude-sonnet-4-20250514",
        cc_utils::tokens::TokenCountMethod::ProviderExact,
        Some("test"),
    );

    assert!(!exact_auto_compact_triggered(true, Some(&below)));
    assert!(exact_auto_compact_triggered(false, Some(&above)));
}

#[test]
fn auto_compact_exact_count_request_uses_final_request_boundary() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool: Arc<dyn Tool> = canonical_tool("BoundaryTool", seen_input);
    let params = ModelCallParams {
        messages: vec![user_message("pre-pipeline")],
        system_prompt: vec!["system boundary".to_string()],
        tools: vec![tool],
        model: Some("claude-sonnet-4-20250514".to_string()),
        max_output_tokens: Some(123),
        skip_cache_write: None,
        thinking_enabled: Some(true),
        effort_value: Some("low".to_string()),
        output_config: None,
        model_reasoning_effort: None,
        advisor_model: Some("advisor-model".to_string()),
    };

    let request = build_auto_compact_exact_count_request(
        &params,
        vec![user_message("post-pipeline")],
        "claude-sonnet-4-20250514",
    );

    assert_eq!(request.messages[0]["content"], json!("post-pipeline"));
    assert_eq!(
        request.system.as_ref().unwrap()[0]["text"],
        json!("system boundary")
    );
    assert_eq!(
        request.tools.as_ref().unwrap()[0]["name"],
        json!("BoundaryTool")
    );
    assert_eq!(request.max_tokens, 123);
    assert!(request.thinking.is_some());
    assert_eq!(request.advisor_model.as_deref(), Some("advisor-model"));
}

#[test]
fn central_permission_default_mode_asks_for_bash_after_tool_allow() {
    let app_state = AppState::default();
    let mut input = json!({"command": "rm -rf F:/temp/gomoku_subagent/*"});

    let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None, None);

    assert!(
        matches!(result, PermissionResult::Ask { .. }),
        "default mode must ask even when the tool-local check allowed the command"
    );
}

#[test]
fn central_permission_allow_rule_still_allows_matching_bash_prefix() {
    let mut app_state = AppState::default();
    app_state
        .tool_permission_context
        .always_allow_rules
        .insert("test".into(), vec!["Bash(prefix:git)".into()]);
    let mut input = json!({"command": "git status"});

    let result = central_permission_result_for_tool("Bash", &mut input, &app_state, None, None);

    assert!(matches!(result, PermissionResult::Allow { .. }));
}

#[test]
fn central_permission_sandbox_allowed_command_allows_workspace_bash() {
    let mut app_state = AppState::default();
    app_state.settings.sandbox.enabled = Some(true);
    app_state.settings.sandbox.mode = Some("workspace".into());
    app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
    let mut input = json!({"command": "cargo test --all"});

    let result = crate::tool_runtime::execution::with_sandbox_availability_override(
        crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
        || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
    );

    assert!(matches!(result, PermissionResult::Allow { .. }));
}

#[test]
fn central_permission_sandbox_allowed_command_asks_when_sandbox_unavailable() {
    let mut app_state = AppState::default();
    app_state.settings.sandbox.enabled = Some(true);
    app_state.settings.sandbox.mode = Some("workspace".into());
    app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
    let mut input = json!({"command": "cargo test --all"});

    let result = crate::tool_runtime::execution::with_sandbox_availability_override(
        crate::sandbox::Availability::Unavailable {
            platform: "test",
            reason: "forced unavailable".to_string(),
        },
        || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
    );

    assert!(matches!(result, PermissionResult::Ask { .. }));
}

#[test]
fn central_permission_sandbox_allowed_command_does_not_override_ask_rule() {
    let mut app_state = AppState::default();
    app_state.settings.sandbox.enabled = Some(true);
    app_state.settings.sandbox.mode = Some("workspace".into());
    app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
    app_state
        .tool_permission_context
        .always_ask_rules
        .insert("test".into(), vec!["Bash".into()]);
    let mut input = json!({"command": "cargo test --all"});

    let result = crate::tool_runtime::execution::with_sandbox_availability_override(
        crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
        || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
    );

    assert!(matches!(result, PermissionResult::Ask { .. }));
}

#[test]
fn central_permission_sandbox_allowed_command_does_not_override_plan_mode() {
    let mut app_state = AppState::default();
    app_state.tool_permission_context.mode = PermissionMode::Plan;
    app_state.settings.sandbox.enabled = Some(true);
    app_state.settings.sandbox.mode = Some("workspace".into());
    app_state.settings.sandbox.allowed_commands = vec!["cargo test".into()];
    let mut input = json!({"command": "cargo test --all"});

    let result = crate::tool_runtime::execution::with_sandbox_availability_override(
        crate::sandbox::Availability::Available(crate::sandbox::Mechanism::Bubblewrap),
        || central_permission_result_for_tool("Bash", &mut input, &app_state, None, None),
    );

    assert!(matches!(result, PermissionResult::Ask { .. }));
}

#[test]
fn central_permission_ask_rule_overrides_hook_allow() {
    let mut app_state = AppState::default();
    app_state
        .tool_permission_context
        .always_ask_rules
        .insert("test".into(), vec!["Bash".into()]);
    let mut input = json!({"command": "git status"});
    let hook_decision = crate::permissions::decision::HookPermissionDecision {
        allow: true,
        source: Some("PreToolUse:test".into()),
        ..Default::default()
    };

    let result = central_permission_result_for_tool(
        "Bash",
        &mut input,
        &app_state,
        Some(&hook_decision),
        None,
    );

    assert!(matches!(result, PermissionResult::Ask { .. }));
}

#[tokio::test]
async fn execute_tool_optional_pre_hook_error_continues_to_tool_call() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("HookedTool", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPreToolHookRunner { critical: false });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_critical_pre_hook_error_blocks_tool_call() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("HookedTool", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPreToolHookRunner { critical: true });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.result.data.as_str().is_some_and(|text| {
        text.contains("Critical pre-tool hook failed") && text.contains("pre hook failed for test")
    }));
    assert!(
        seen_input.lock().is_none(),
        "critical pre-hook failure must stop before Tool::call"
    );
}

#[tokio::test]
async fn execute_tool_optional_post_hook_error_keeps_tool_success() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("HookedTool", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPostToolHookRunner {
        event_name: "PostToolUse",
        critical: false,
    });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.result.data, json!("ok"));
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_critical_post_hook_error_returns_failed_result() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("HookedTool", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPostToolHookRunner {
        event_name: "PostToolUse",
        critical: true,
    });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.result.data.as_str().is_some_and(|text| {
        text.contains("Critical post-tool hook failed")
            && text.contains("post hook failed for test")
    }));
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_optional_post_failure_hook_error_keeps_tool_error() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool: Arc<dyn Tool> = Arc::new(FailingTool {
        name: "HookedTool",
        seen_input: seen_input.clone(),
    });
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPostToolHookRunner {
        event_name: "PostToolUseFailure",
        critical: false,
    });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert_eq!(result.result.data, json!("Error: tool failed for test"));
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_critical_post_failure_hook_error_returns_hook_failure() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool: Arc<dyn Tool> = Arc::new(FailingTool {
        name: "HookedTool",
        seen_input: seen_input.clone(),
    });
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    deps.hook_runner = Arc::new(FailingPostToolHookRunner {
        event_name: "PostToolUseFailure",
        critical: true,
    });

    let result = deps
        .execute_tool(
            tool_request("HookedTool", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.result.data.as_str().is_some_and(|text| {
        text.contains("Critical post-failure hook failed")
            && text.contains("tool failed for test")
            && text.contains("post failure hook failed for test")
    }));
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_rejects_validation_error_before_call() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = Arc::new(CanonicalTool {
        name: "ValidateMe",
        validation_error: Some("missing required field"),
        permission: None,
        result: ToolResult::default(),
        max_result_size_chars: 100_000,
        seen_input: seen_input.clone(),
        progress_payload: None,
    });
    let deps = make_deps(vec![tool], PermissionMode::Bypass);

    let result = deps
        .execute_tool(
            tool_request("ValidateMe", json!({})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result
        .result
        .data
        .as_str()
        .is_some_and(|text| text.contains("Input validation error")));
    assert!(
        seen_input.lock().is_none(),
        "validation failure must stop before Tool::call"
    );
}

#[tokio::test]
async fn execute_tool_sanitizes_input_and_enforces_result_size() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = Arc::new(CanonicalTool {
        name: "SanitizeMe",
        validation_error: None,
        permission: None,
        result: ToolResult {
            data: json!("x".repeat(200)),
            model_content: Some(ToolResultContent::Text("model content".to_string())),
            display_preview: Some("preview".to_string()),
            new_messages: vec![],
        },
        max_result_size_chars: 40,
        seen_input: seen_input.clone(),
        progress_payload: None,
    });
    let deps = make_deps(vec![tool], PermissionMode::Bypass);

    let result = deps
        .execute_tool(
            tool_request(
                "SanitizeMe",
                json!({"keep": true, "_simulatedSedEdit": "remove me"}),
            ),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let input = seen_input.lock().clone().expect("tool should be called");
    assert_eq!(input.get("keep"), Some(&json!(true)));
    assert!(
        input.get("_simulatedSedEdit").is_none(),
        "canonical path should strip simulated edit marker"
    );
    assert!(
        result
            .result
            .data
            .as_str()
            .is_some_and(|text| text.contains("characters omitted")),
        "canonical path should enforce max_result_size_chars"
    );
    assert_eq!(result.result.display_preview.as_deref(), Some("preview"));
    assert!(matches!(
        result.result.model_content,
        Some(ToolResultContent::Text(ref text)) if text == "model content"
    ));
}

#[tokio::test]
#[serial_test::serial]
async fn execute_tool_allows_plan_file_write_in_plan_mode() {
    struct TestWriteTool;

    #[async_trait::async_trait]
    impl Tool for TestWriteTool {
        fn name(&self) -> &str {
            "Write"
        }

        async fn description(&self, _input: &serde_json::Value) -> String {
            "test write".to_string()
        }

        fn input_json_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn is_read_only(&self, _input: &serde_json::Value) -> bool {
            false
        }

        async fn call(
            &self,
            input: serde_json::Value,
            _ctx: &crate::types::tool::ToolUseContext,
            _parent_message: &AssistantMessage,
            _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> anyhow::Result<crate::types::tool::ToolResult> {
            let path = input
                .get("file_path")
                .and_then(|value| value.as_str())
                .unwrap();
            let content = input
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap();
            std::fs::write(path, content)?;
            Ok(crate::types::tool::ToolResult {
                data: serde_json::json!("ok"),
                new_messages: vec![],
                ..Default::default()
            })
        }

        async fn prompt(&self) -> String {
            String::new()
        }
    }

    struct OriginalCwdGuard(std::path::PathBuf);

    impl Drop for OriginalCwdGuard {
        fn drop(&mut self) {
            crate::bootstrap::PROCESS_STATE.write().original_cwd = self.0.clone();
        }
    }

    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".cc-rust")).unwrap();
    let original_cwd = crate::bootstrap::PROCESS_STATE.read().original_cwd.clone();
    let _guard = OriginalCwdGuard(original_cwd);
    crate::bootstrap::PROCESS_STATE.write().original_cwd = temp.path().to_path_buf();

    let plan_path = crate::config::paths::current_plan_file_path(temp.path());
    let plan_path_string = plan_path.to_string_lossy().into_owned();
    let content = "## Plan\n- verify canonical plan file write";
    let tool: Arc<dyn Tool> = Arc::new(TestWriteTool);
    let deps = make_deps(vec![tool], PermissionMode::Plan);

    let result = deps
        .execute_tool(
            tool_request(
                "Write",
                json!({
                    "file_path": plan_path_string,
                    "content": content,
                }),
            ),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "canonical plan file write should not require a prompt: {:?}",
        result.result.data
    );
    assert_eq!(std::fs::read_to_string(plan_path).unwrap(), content);
}

#[tokio::test]
async fn execute_tool_blocks_dangerous_command_before_call() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("Bash", seen_input.clone());
    let deps = make_deps(vec![tool], PermissionMode::Default);

    let result = deps
        .execute_tool(
            tool_request("Bash", json!({"command": "rm -rf /"})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result
        .result
        .data
        .as_str()
        .is_some_and(|text| text.contains("Dangerous command blocked")));
    assert!(
        seen_input.lock().is_none(),
        "security validation must stop before Tool::call"
    );
}

#[tokio::test]
async fn execute_tool_returns_permission_deny_without_call() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = Arc::new(CanonicalTool {
        name: "DenyMe",
        validation_error: None,
        permission: Some(PermissionResult::Deny {
            message: "blocked by test".to_string(),
        }),
        result: ToolResult::default(),
        max_result_size_chars: 100_000,
        seen_input: seen_input.clone(),
        progress_payload: None,
    });
    let deps = make_deps(vec![tool], PermissionMode::Default);

    let result = deps
        .execute_tool(
            tool_request("DenyMe", json!({})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result
        .result
        .data
        .as_str()
        .is_some_and(|text| text.contains("Permission denied: blocked by test")));
    assert!(
        seen_input.lock().is_none(),
        "permission denial must stop before Tool::call"
    );
}

#[tokio::test]
async fn execute_tool_bypass_skips_tool_local_permission_prompt() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = Arc::new(CanonicalTool {
        name: "AskMe",
        validation_error: None,
        permission: Some(PermissionResult::Ask {
            message: "needs approval".to_string(),
        }),
        result: ToolResult {
            data: json!("ran"),
            new_messages: vec![],
            ..Default::default()
        },
        max_result_size_chars: 100_000,
        seen_input: seen_input.clone(),
        progress_payload: None,
    });
    let mut deps = make_deps(vec![tool], PermissionMode::Bypass);
    let callback_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    deps.permission_callback = Some(Arc::new({
        let callback_calls = callback_calls.clone();
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { PermissionResponsePayload::decision("deny") })
        }
    }));

    let result = deps
        .execute_tool(
            tool_request("AskMe", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.result.data, json!("ran"));
    assert_eq!(callback_calls.load(Ordering::SeqCst), 0);
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_auto_mode_does_not_classify_when_rule_denies() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("DenyMe", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Auto);
    deps.state
        .write()
        .app_state
        .tool_permission_context
        .always_deny_rules
        .insert("test".to_string(), vec!["DenyMe".to_string()]);

    let classifier_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    deps.auto_classifier_fn = Some(Arc::new({
        let classifier_calls = classifier_calls.clone();
        move |_, _, _, _, _| {
            classifier_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Some(AutoClassifierDecision::allow(
                    "test-classifier",
                    AutoClassifierStage::Fast,
                ))
            })
        }
    }));

    let result = deps
        .execute_tool(
            tool_request("DenyMe", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert_eq!(classifier_calls.load(Ordering::SeqCst), 0);
    assert!(
        seen_input.lock().is_none(),
        "central deny rule must stop before Tool::call"
    );
}

#[tokio::test]
async fn execute_tool_auto_classifier_denials_fall_back_to_prompt() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = canonical_tool("NeedsClassifier", seen_input.clone());
    let mut deps = make_deps(vec![tool], PermissionMode::Auto);
    let classifier_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    deps.auto_classifier_fn = Some(Arc::new({
        let classifier_calls = classifier_calls.clone();
        move |_, _, _, _, _| {
            classifier_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Some(AutoClassifierDecision::deny(
                    "test-classifier",
                    AutoClassifierStage::Thinking,
                    "blocked by classifier",
                ))
            })
        }
    }));
    let callback: PermissionCallback =
        Arc::new(|_| Box::pin(async { PermissionResponsePayload::decision("allow") }));
    deps.permission_callback = Some(callback);

    for _ in 0..2 {
        let result = deps
            .execute_tool(
                tool_request("NeedsClassifier", json!({"value": true})),
                &deps.get_tools(),
                &parent_message(),
                None,
            )
            .await
            .unwrap();
        assert!(result.is_error);
    }
    assert!(
        seen_input.lock().is_none(),
        "classifier denials must stop before Tool::call"
    );

    let result = deps
        .execute_tool(
            tool_request("NeedsClassifier", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(classifier_calls.load(Ordering::SeqCst), 3);
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));

    *seen_input.lock() = None;
    let result = deps
        .execute_tool(
            tool_request("NeedsClassifier", json!({"value": true})),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(
        classifier_calls.load(Ordering::SeqCst),
        3,
        "interactive fallback should avoid further classifier calls"
    );
    assert_eq!(seen_input.lock().clone(), Some(json!({"value": true})));
}

#[tokio::test]
async fn execute_tool_auto_classifier_uses_tool_specific_input() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("classifier-input.txt");
    let path_string = path.to_string_lossy().into_owned();
    let captured = Arc::new(parking_lot::Mutex::new(None::<(Value, Value)>));
    let mut deps = make_deps(
        vec![Arc::new(cc_tools::fs::file_write::FileWriteTool::new())],
        PermissionMode::Auto,
    );
    deps.state
        .write()
        .app_state
        .tool_permission_context
        .additional_working_directories
        .insert(
            path_string.clone(),
            cc_types::permissions::AdditionalWorkingDirectory {
                path: temp.path().to_string_lossy().into_owned(),
                read_only: false,
            },
        );
    deps.auto_classifier_fn = Some(Arc::new({
        let captured = captured.clone();
        move |_, raw_input, classifier_input, _, _| {
            *captured.lock() = Some((raw_input, classifier_input));
            Box::pin(async {
                Some(AutoClassifierDecision::allow(
                    "test-classifier",
                    AutoClassifierStage::Fast,
                ))
            })
        }
    }));

    let result = deps
        .execute_tool(
            tool_request(
                "Write",
                json!({
                    "file_path": path_string,
                    "content": "secret body that should not be in classifier_input",
                }),
            ),
            &deps.get_tools(),
            &parent_message(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let (raw_input, classifier_input) = captured.lock().clone().expect("classifier called");
    assert_eq!(
        raw_input.get("content").and_then(Value::as_str),
        Some("secret body that should not be in classifier_input")
    );
    assert_eq!(
        classifier_input.get("file_path"),
        raw_input.get("file_path")
    );
    assert_eq!(classifier_input.get("content"), None);
    assert_eq!(
        classifier_input.get("operation"),
        Some(&json!("write_file"))
    );
}

#[tokio::test]
async fn execute_tool_preserves_ask_callback_and_progress_ids() {
    let seen_input = Arc::new(parking_lot::Mutex::new(None));
    let tool = Arc::new(CanonicalTool {
        name: "AskMe",
        validation_error: None,
        permission: Some(PermissionResult::Ask {
            message: "needs approval".to_string(),
        }),
        result: ToolResult {
            data: json!("approved"),
            new_messages: vec![],
            ..Default::default()
        },
        max_result_size_chars: 100_000,
        seen_input,
        progress_payload: Some(json!({"phase": "running"})),
    });
    let mut deps = make_deps(vec![tool], PermissionMode::Default);
    let callback: PermissionCallback = Arc::new(|_| {
        Box::pin(async {
            PermissionResponsePayload::new(
                "allow",
                Some("Apply this approval narrowly.".to_string()),
            )
        })
    });
    deps.permission_callback = Some(callback);
    let seen_progress = Arc::new(parking_lot::Mutex::new(None));
    let progress_callback: Arc<dyn Fn(ToolProgress) + Send + Sync> = {
        let seen_progress = seen_progress.clone();
        Arc::new(move |progress| {
            *seen_progress.lock() = Some(progress);
        })
    };

    let result = deps
        .execute_tool(
            tool_request("AskMe", json!({})),
            &deps.get_tools(),
            &parent_message(),
            Some(progress_callback),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.result.new_messages.len(), 1);
    match &result.result.new_messages[0] {
        Message::User(message) => {
            assert!(message.is_meta);
            match &message.content {
                MessageContent::Text(text) => {
                    assert_eq!(text, "Apply this approval narrowly.");
                }
                other => panic!("expected text feedback message, got {other:?}"),
            }
        }
        other => panic!("expected user feedback message, got {other:?}"),
    }
    let progress = seen_progress
        .lock()
        .clone()
        .expect("tool should emit progress");
    assert_eq!(progress.tool_use_id, "tu_test");
    assert_eq!(progress.data, json!({"phase": "running"}));
}
