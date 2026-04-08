//! PlanMode tools — EnterPlanMode / ExitPlanMode.
//!
//! Corresponds to TypeScript:
//!   src/tools/EnterPlanModeTool/EnterPlanModeTool.ts
//!   src/tools/ExitPlanModeTool/ExitPlanModeV2Tool.ts
//!
//! Plan mode switches the permission context to read-only, preventing
//! any write operations.  The model explores the codebase, designs an
//! approach, and then calls ExitPlanMode to present the plan for approval.
//!
//! State lifecycle:
//!   1. EnterPlanMode saves `pre_plan_mode` and sets mode → Plan
//!   2. Model explores (read-only tools only)
//!   3. ExitPlanMode restores mode from `pre_plan_mode`

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// ---------------------------------------------------------------------------
// EnterPlanMode
// ---------------------------------------------------------------------------

/// EnterPlanMode — switch the session to read-only planning mode.
pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    async fn description(&self, _input: &Value) -> String {
        "Enter plan mode for complex tasks requiring exploration and design before coding."
            .to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn validate_input(&self, _input: &Value, ctx: &ToolUseContext) -> ValidationResult {
        // Cannot enter plan mode from agent context
        if ctx.agent_id.is_some() {
            return ValidationResult::Error {
                message: "EnterPlanMode cannot be used in agent contexts.".to_string(),
                error_code: 1,
            };
        }

        // Already in plan mode?
        let state = (ctx.get_app_state)();
        if state.tool_permission_context.mode == PermissionMode::Plan {
            return ValidationResult::Error {
                message: "Already in plan mode.".to_string(),
                error_code: 2,
            };
        }

        ValidationResult::Ok
    }

    async fn call(
        &self,
        _input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        // Save current mode and switch to Plan
        (ctx.set_app_state)(Box::new(|mut state| {
            state.tool_permission_context.pre_plan_mode =
                Some(state.tool_permission_context.mode.clone());
            state.tool_permission_context.mode = PermissionMode::Plan;
            state
        }));

        let instructions = concat!(
            "Entered plan mode. You should now focus on exploring the codebase ",
            "and designing an implementation approach.\n\n",
            "In plan mode, you should:\n",
            "1. Thoroughly explore the codebase to understand existing patterns\n",
            "2. Identify similar features and architectural approaches\n",
            "3. Consider multiple approaches and their trade-offs\n",
            "4. Use AskUserQuestion if you need to clarify the approach\n",
            "5. Design a concrete implementation strategy\n",
            "6. When ready, use ExitPlanMode to present your plan for approval\n\n",
            "Remember: DO NOT write or edit any files yet. ",
            "This is a read-only exploration and planning phase.",
        );

        Ok(ToolResult {
            data: json!({ "message": instructions }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        concat!(
            "Enter plan mode for complex tasks requiring exploration and design. ",
            "In plan mode, only read-only tools (Read, Glob, Grep, LSP) are allowed. ",
            "Use this before implementing complex features to ensure a thorough ",
            "understanding of the codebase and a well-designed approach.",
        )
        .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        String::new() // hidden from tool-use display
    }
}

// ---------------------------------------------------------------------------
// ExitPlanMode
// ---------------------------------------------------------------------------

/// ExitPlanMode — present the plan for approval and exit plan mode.
pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    async fn description(&self, _input: &Value) -> String {
        "Exit plan mode and begin implementation.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plan": {
                    "type": "string",
                    "description": "The implementation plan (optional — can also be written to a plan file)"
                }
            },
            "additionalProperties": false
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn validate_input(&self, _input: &Value, ctx: &ToolUseContext) -> ValidationResult {
        let state = (ctx.get_app_state)();
        if state.tool_permission_context.mode != PermissionMode::Plan {
            return ValidationResult::Error {
                message: concat!(
                    "You are not in plan mode. This tool is only for exiting plan mode ",
                    "after writing a plan. If your plan was already approved, continue ",
                    "with implementation.",
                )
                .to_string(),
                error_code: 1,
            };
        }
        ValidationResult::Ok
    }

    async fn check_permissions(&self, _input: &Value, _ctx: &ToolUseContext) -> PermissionResult {
        // Require user confirmation to exit plan mode
        PermissionResult::Ask {
            message: "Exit plan mode and begin implementation?".to_string(),
        }
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let plan = input
            .get("plan")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Restore previous permission mode
        (ctx.set_app_state)(Box::new(|mut state| {
            let restore_mode = state
                .tool_permission_context
                .pre_plan_mode
                .take()
                .unwrap_or(PermissionMode::Default);
            state.tool_permission_context.mode = restore_mode;
            state
        }));

        let mut result = json!({
            "message": "Exited plan mode. Normal operations restored. You may now implement the plan.",
        });

        if !plan.is_empty() {
            result["plan"] = json!(plan);
        }

        Ok(ToolResult {
            data: result,
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        concat!(
            "Exit plan mode after designing your implementation approach. ",
            "Optionally include the plan text. The user will be asked to ",
            "approve before normal operations resume.",
        )
        .to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        String::new() // hidden from tool-use display
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::app_state::AppState;
    use parking_lot::RwLock;
    use std::sync::Arc;

    fn make_ctx(state: Arc<RwLock<AppState>>) -> ToolUseContext {
        let state_r = Arc::clone(&state);
        let state_w = Arc::clone(&state);

        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".to_string(),
                verbose: false,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || state_r.read().clone()),
            set_app_state: Arc::new(move |f: Box<dyn FnOnce(AppState) -> AppState>| {
                let mut s = state_w.write();
                let old = s.clone();
                *s = f(old);
            }),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
        }
    }

    #[test]
    fn test_enter_plan_mode_name() {
        let tool = EnterPlanModeTool;
        assert_eq!(tool.name(), "EnterPlanMode");
        assert!(tool.is_read_only(&json!({})));
        assert!(tool.is_concurrency_safe(&json!({})));
    }

    #[test]
    fn test_exit_plan_mode_name() {
        let tool = ExitPlanModeTool;
        assert_eq!(tool.name(), "ExitPlanMode");
    }

    #[tokio::test]
    async fn test_enter_plan_mode_blocks_agent() {
        let tool = EnterPlanModeTool;
        let state = Arc::new(RwLock::new(AppState::default()));
        let mut ctx = make_ctx(state);
        ctx.agent_id = Some("agent-1".to_string());

        let result = tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 1, .. }
        ));
    }

    #[tokio::test]
    async fn test_enter_plan_mode_blocks_if_already_plan() {
        let tool = EnterPlanModeTool;
        let state = Arc::new(RwLock::new(AppState::default()));
        {
            let mut s = state.write();
            s.tool_permission_context.mode = PermissionMode::Plan;
        }
        let ctx = make_ctx(state);

        let result = tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 2, .. }
        ));
    }

    #[tokio::test]
    async fn test_enter_exit_plan_mode_roundtrip() {
        let state = Arc::new(RwLock::new(AppState::default()));
        let dummy_msg = AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        };

        // Enter plan mode
        let enter_tool = EnterPlanModeTool;
        let ctx = make_ctx(Arc::clone(&state));
        let validation = enter_tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(validation, ValidationResult::Ok));

        let result = enter_tool
            .call(json!({}), &ctx, &dummy_msg, None)
            .await
            .unwrap();
        assert!(result.data["message"]
            .as_str()
            .unwrap()
            .contains("plan mode"));

        // Verify state changed
        {
            let s = state.read();
            assert_eq!(s.tool_permission_context.mode, PermissionMode::Plan);
            assert_eq!(
                s.tool_permission_context.pre_plan_mode,
                Some(PermissionMode::Default)
            );
        }

        // Exit plan mode
        let exit_tool = ExitPlanModeTool;
        let ctx2 = make_ctx(Arc::clone(&state));
        let validation = exit_tool.validate_input(&json!({}), &ctx2).await;
        assert!(matches!(validation, ValidationResult::Ok));

        let result = exit_tool
            .call(json!({"plan": "My plan here"}), &ctx2, &dummy_msg, None)
            .await
            .unwrap();
        assert!(result.data["message"].as_str().unwrap().contains("Exited"));
        assert_eq!(result.data["plan"].as_str().unwrap(), "My plan here");

        // Verify state restored
        {
            let s = state.read();
            assert_eq!(s.tool_permission_context.mode, PermissionMode::Default);
            assert!(s.tool_permission_context.pre_plan_mode.is_none());
        }
    }

    #[tokio::test]
    async fn test_exit_plan_mode_rejects_outside_plan() {
        let tool = ExitPlanModeTool;
        let state = Arc::new(RwLock::new(AppState::default()));
        let ctx = make_ctx(state);

        let result = tool.validate_input(&json!({}), &ctx).await;
        assert!(matches!(
            result,
            ValidationResult::Error { error_code: 1, .. }
        ));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_restores_auto() {
        let state = Arc::new(RwLock::new(AppState::default()));
        {
            let mut s = state.write();
            s.tool_permission_context.mode = PermissionMode::Plan;
            s.tool_permission_context.pre_plan_mode = Some(PermissionMode::Auto);
        }

        let exit_tool = ExitPlanModeTool;
        let dummy_msg = AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        };
        let ctx = make_ctx(Arc::clone(&state));

        let _ = exit_tool
            .call(json!({}), &ctx, &dummy_msg, None)
            .await
            .unwrap();

        let s = state.read();
        assert_eq!(s.tool_permission_context.mode, PermissionMode::Auto);
    }

    #[test]
    fn test_plan_mode_schema() {
        let enter = EnterPlanModeTool;
        let schema = enter.input_json_schema();
        assert!(schema.get("properties").is_some());

        let exit = ExitPlanModeTool;
        let schema = exit.input_json_schema();
        assert!(schema["properties"].get("plan").is_some());
    }
}
