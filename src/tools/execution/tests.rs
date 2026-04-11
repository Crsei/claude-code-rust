use super::*;
use super::security::{enforce_result_size, find_tool, security_validate};
use crate::types::app_state::AppState;
use crate::types::tool::{
    FileStateCache, PermissionMode, PermissionResult, ToolPermissionContext, ToolUseOptions,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use crate::types::message::AssistantMessage;
use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext, Tools};

// -- Helper: minimal ToolUseContext for security_validate tests ----------

fn make_ctx_with_mode(mode: PermissionMode) -> ToolUseContext {
    let mut app = AppState::default();
    app.tool_permission_context.mode = mode;

    let (_tx, rx) = tokio::sync::watch::channel(false);
    ToolUseContext {
        options: ToolUseOptions {
            debug: false,
            main_loop_model: "test".into(),
            verbose: false,
            is_non_interactive_session: false,
            custom_system_prompt: None,
            append_system_prompt: None,
            max_budget_usd: None,
        },
        abort_signal: rx,
        read_file_state: FileStateCache {
            entries: HashMap::new(),
        },
        get_app_state: Arc::new(move || app.clone()),
        set_app_state: Arc::new(|_| {}),
        messages: vec![],
        agent_id: None,
        agent_type: None,
        query_tracking: None,
        permission_callback: None,
        bg_agent_tx: None,
    }
}

// -- Stub tools for testing is_read_only behavior -----------------------

struct ReadOnlyStub;
#[async_trait::async_trait]
impl Tool for ReadOnlyStub {
    fn name(&self) -> &str { "Grep" }
    async fn description(&self, _: &Value) -> String { String::new() }
    fn input_json_schema(&self) -> Value { Value::Null }
    fn is_read_only(&self, _: &Value) -> bool { true }
    async fn call(
        &self, _: Value, _: &ToolUseContext,
        _: &crate::types::message::AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult> {
        Ok(ToolResult { data: Value::Null, new_messages: vec![] })
    }
    async fn prompt(&self) -> String { String::new() }
}

struct WritableStub;
#[async_trait::async_trait]
impl Tool for WritableStub {
    fn name(&self) -> &str { "Bash" }
    async fn description(&self, _: &Value) -> String { String::new() }
    fn input_json_schema(&self) -> Value { Value::Null }
    fn is_read_only(&self, _: &Value) -> bool { false }
    async fn call(
        &self, _: Value, _: &ToolUseContext,
        _: &crate::types::message::AssistantMessage,
        _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult> {
        Ok(ToolResult { data: Value::Null, new_messages: vec![] })
    }
    async fn prompt(&self) -> String { String::new() }
}

// -- Existing tests -----------------------------------------------------

#[test]
fn test_find_tool_missing() {
    let tools: Tools = vec![];
    assert!(find_tool("NonExistent", &tools).is_none());
}

#[test]
fn test_enforce_result_size_small() {
    let data = Value::String("hello".into());
    let result = enforce_result_size(data.clone(), 1000);
    assert_eq!(result, data);
}

#[test]
fn test_enforce_result_size_large() {
    let large = "x".repeat(10_000);
    let data = Value::String(large);
    let result = enforce_result_size(data, 1000);
    if let Value::String(s) = result {
        assert!(s.len() < 10_000);
        assert!(s.contains("characters omitted"));
    } else {
        panic!("expected string");
    }
}

#[test]
fn test_streaming_executor_new() {
    let executor = StreamingToolExecutor::new();
    assert!(!executor.has_bash_error());
}

#[test]
fn test_tracked_tool_state_transitions() {
    assert_ne!(TrackedToolState::Queued, TrackedToolState::Executing);
    assert_ne!(TrackedToolState::Executing, TrackedToolState::Completed);
}

// -- Stage 3c: security_validate tests ----------------------------------

#[test]
fn test_plan_mode_blocks_write_tools() {
    let ctx = make_ctx_with_mode(PermissionMode::Plan);
    let tool = WritableStub;
    let input = serde_json::json!({"command": "ls"});
    let now = Instant::now();

    let result = security_validate("id1", "Bash", &input, &tool, &ctx, now);
    assert!(result.is_some(), "Plan mode should block non-read-only tool");
    let err = result.unwrap();
    assert!(err.is_error);
    assert!(
        err.result.data.as_str().unwrap().contains("Plan mode"),
        "Error message should mention Plan mode"
    );
}

#[test]
fn test_plan_mode_allows_read_tools() {
    let ctx = make_ctx_with_mode(PermissionMode::Plan);
    let tool = ReadOnlyStub;
    let input = serde_json::json!({"pattern": "foo"});
    let now = Instant::now();

    let result = security_validate("id2", "Grep", &input, &tool, &ctx, now);
    assert!(result.is_none(), "Plan mode should allow read-only tool");
}

#[test]
fn test_dangerous_command_blocked() {
    let ctx = make_ctx_with_mode(PermissionMode::Default);
    let tool = WritableStub;
    let input = serde_json::json!({"command": "rm -rf /"});
    let now = Instant::now();

    let result = security_validate("id3", "Bash", &input, &tool, &ctx, now);
    assert!(result.is_some(), "Dangerous command should be blocked");
    let err = result.unwrap();
    assert!(err.result.data.as_str().unwrap().contains("Dangerous command blocked"));
}

#[test]
fn test_safe_command_allowed() {
    let ctx = make_ctx_with_mode(PermissionMode::Default);
    let tool = WritableStub;
    let input = serde_json::json!({"command": "ls -la"});
    let now = Instant::now();

    let result = security_validate("id4", "Bash", &input, &tool, &ctx, now);
    assert!(result.is_none(), "Safe command should be allowed");
}

#[test]
fn test_path_traversal_blocked() {
    let ctx = make_ctx_with_mode(PermissionMode::Default);
    let tool = WritableStub; // is_read_only = false, but tool_name matters for path check
    let input = serde_json::json!({"file_path": "/../../../../../etc/passwd"});
    let now = Instant::now();

    let result = security_validate("id5", "Write", &input, &tool, &ctx, now);
    assert!(result.is_some(), "Path traversal should be blocked");
    let err = result.unwrap();
    assert!(err.result.data.as_str().unwrap().contains("Invalid file path"));
}

#[test]
fn test_path_outside_cwd_blocked() {
    // Initialize ProcessState with a known cwd so the boundary check works
    {
        let mut ps = crate::bootstrap::PROCESS_STATE.write();
        ps.original_cwd = std::path::PathBuf::from(std::env::current_dir().unwrap());
    }

    let ctx = make_ctx_with_mode(PermissionMode::Default);
    let tool = WritableStub;
    // Use an absolute path that is definitely outside the cwd
    let outside_path = if cfg!(windows) {
        "C:\\Windows\\System32\\evil.txt"
    } else {
        "/tmp/evil.txt"
    };
    let input = serde_json::json!({"file_path": outside_path});
    let now = Instant::now();

    let result = security_validate("id6", "Write", &input, &tool, &ctx, now);
    assert!(
        result.is_some(),
        "Path outside cwd should be blocked, path={}",
        outside_path
    );
    let err = result.unwrap();
    assert!(err.result.data.as_str().unwrap().contains("outside the allowed"));
}

#[test]
fn test_bypass_mode_skips_all() {
    let ctx = make_ctx_with_mode(PermissionMode::Bypass);
    let tool = WritableStub;

    // Dangerous command — would normally be blocked
    let input = serde_json::json!({"command": "rm -rf /"});
    let now = Instant::now();
    let result = security_validate("id7", "Bash", &input, &tool, &ctx, now);
    assert!(result.is_none(), "Bypass mode should skip all security checks");

    // Path traversal — would normally be blocked
    let input2 = serde_json::json!({"file_path": "/../../../../../etc/passwd"});
    let result2 = security_validate("id8", "Write", &input2, &tool, &ctx, now);
    assert!(result2.is_none(), "Bypass mode should skip path check too");
}

#[test]
fn test_powershell_dangerous_command_blocked() {
    let ctx = make_ctx_with_mode(PermissionMode::Default);
    let tool = WritableStub;
    let input = serde_json::json!({"command": "rm -rf /"});
    let now = Instant::now();

    // PowerShell should also be checked
    let result = security_validate("id9", "PowerShell", &input, &tool, &ctx, now);
    assert!(result.is_some(), "PowerShell dangerous command should be blocked");
}
