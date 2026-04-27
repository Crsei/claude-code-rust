//! Streaming tool executor for concurrent tool execution.

use serde_json::Value;

use crate::tools::hooks::HookEventConfig;
use crate::types::message::AssistantMessage;
use crate::types::tool::{ToolResult, ToolUseContext, Tools};

use super::pipeline::run_tool_use;
use super::ToolExecutionResult;

/// State of a tracked tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackedToolState {
    Queued,
    Executing,
    Completed,
    Yielded,
}

/// A tracked tool execution with its state and result.
#[derive(Debug)]
pub struct TrackedTool {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: Value,
    pub state: TrackedToolState,
    pub result: Option<ToolExecutionResult>,
    pub is_concurrency_safe: bool,
}

/// StreamingToolExecutor — coordinates concurrent vs serial tool execution.
///
/// Corresponds to TypeScript: `StreamingToolExecutor` in StreamingToolExecutor.ts
///
/// Tools are added as they arrive from the streaming API response.
/// Concurrency-safe tools start executing immediately; non-safe tools
/// wait until all preceding concurrent tools complete.
pub struct StreamingToolExecutor {
    tracked: Vec<TrackedTool>,
    /// Whether any Bash tool has errored (triggers sibling abort).
    has_bash_error: bool,
    /// Hook configurations to pass into each `run_tool_use()` call.
    hook_configs: Vec<HookEventConfig>,
}

impl StreamingToolExecutor {
    pub fn new() -> Self {
        Self {
            tracked: Vec::new(),
            has_bash_error: false,
            hook_configs: Vec::new(),
        }
    }

    /// Create with hook configurations loaded from AppState.
    pub fn with_hook_configs(hook_configs: Vec<HookEventConfig>) -> Self {
        Self {
            tracked: Vec::new(),
            has_bash_error: false,
            hook_configs,
        }
    }

    /// Add a tool use block to the executor.
    pub fn add_tool(
        &mut self,
        tool_use_id: String,
        tool_name: String,
        input: Value,
        tools: &Tools,
    ) {
        let tool = tools.iter().find(|t| t.name() == tool_name);
        let is_safe = tool.is_some_and(|t| t.is_concurrency_safe(&input));

        self.tracked.push(TrackedTool {
            tool_use_id,
            tool_name,
            input,
            state: TrackedToolState::Queued,
            result: None,
            is_concurrency_safe: is_safe,
        });
    }

    /// Execute all queued tools respecting concurrency constraints.
    ///
    /// Returns results in FIFO order (order they were added).
    pub async fn execute_all(
        &mut self,
        tools: &Tools,
        ctx: &ToolUseContext,
        parent_message: &AssistantMessage,
    ) -> Vec<ToolExecutionResult> {
        let mut results = Vec::new();

        // Process tools in order, batching consecutive concurrent-safe ones
        let mut i = 0;
        while i < self.tracked.len() {
            if self.tracked[i].is_concurrency_safe {
                // Collect consecutive concurrent-safe tools
                let batch_start = i;
                while i < self.tracked.len() && self.tracked[i].is_concurrency_safe {
                    self.tracked[i].state = TrackedToolState::Executing;
                    i += 1;
                }

                // Execute batch (sequentially for now — see orchestration.rs note)
                for j in batch_start..i {
                    let tracked = &self.tracked[j];
                    let result = run_tool_use(
                        &tracked.tool_use_id,
                        &tracked.tool_name,
                        tracked.input.clone(),
                        tools,
                        ctx,
                        parent_message,
                        None,
                        &self.hook_configs,
                    )
                    .await;

                    // Check for Bash sibling abort
                    if result.is_error && tracked.tool_name == "Bash" {
                        self.has_bash_error = true;
                    }

                    self.tracked[j].state = TrackedToolState::Completed;
                    self.tracked[j].result = Some(result);
                }
            } else {
                // Serial execution
                self.tracked[i].state = TrackedToolState::Executing;

                // If a Bash error occurred, generate synthetic error for remaining tools
                if self.has_bash_error {
                    let tid = self.tracked[i].tool_use_id.clone();
                    let tname = self.tracked[i].tool_name.clone();
                    self.tracked[i].state = TrackedToolState::Completed;
                    self.tracked[i].result = Some(ToolExecutionResult {
                        tool_use_id: tid,
                        tool_name: tname,
                        result: ToolResult {
                            data: Value::String(
                                "Execution skipped: a sibling Bash tool encountered an error."
                                    .into(),
                            ),
                            new_messages: vec![],
                            ..Default::default()
                        },
                        is_error: true,
                        new_messages: vec![],
                        hook_stopped_continuation: false,
                        duration_ms: 0,
                    });
                    i += 1;
                    continue;
                }

                let tracked = &self.tracked[i];
                let result = run_tool_use(
                    &tracked.tool_use_id,
                    &tracked.tool_name,
                    tracked.input.clone(),
                    tools,
                    ctx,
                    parent_message,
                    None,
                    &self.hook_configs,
                )
                .await;

                if result.is_error && tracked.tool_name == "Bash" {
                    self.has_bash_error = true;
                }

                self.tracked[i].state = TrackedToolState::Completed;
                self.tracked[i].result = Some(result);
                i += 1;
            }
        }

        // Collect results in FIFO order and mark as yielded
        for tracked in &mut self.tracked {
            tracked.state = TrackedToolState::Yielded;
            if let Some(result) = tracked.result.take() {
                results.push(result);
            }
        }

        results
    }

    /// Check if any Bash tool has errored.
    pub fn has_bash_error(&self) -> bool {
        self.has_bash_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::hooks::HookEventConfig;
    use crate::types::tool::{Tool, ToolProgress, ToolResult, ToolUseContext};
    use serde_json::{json, Value};
    use std::sync::Arc;

    // ── Minimal Tool stubs ────────────────────────────────────────────────

    /// A tool stub that reports itself as concurrency-safe.
    struct ConcurrentStub;
    #[async_trait::async_trait]
    impl Tool for ConcurrentStub {
        fn name(&self) -> &str {
            "ConcurrentStub"
        }
        async fn description(&self, _: &Value) -> String {
            String::new()
        }
        fn input_json_schema(&self) -> Value {
            Value::Null
        }
        fn is_concurrency_safe(&self, _: &Value) -> bool {
            true
        }
        async fn call(
            &self,
            _: Value,
            _: &ToolUseContext,
            _: &crate::types::message::AssistantMessage,
            _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                data: Value::Null,
                new_messages: vec![],
                ..Default::default()
            })
        }
        async fn prompt(&self) -> String {
            String::new()
        }
    }

    /// A tool stub that reports itself as NOT concurrency-safe (serial).
    struct SerialStub;
    #[async_trait::async_trait]
    impl Tool for SerialStub {
        fn name(&self) -> &str {
            "SerialStub"
        }
        async fn description(&self, _: &Value) -> String {
            String::new()
        }
        fn input_json_schema(&self) -> Value {
            Value::Null
        }
        fn is_concurrency_safe(&self, _: &Value) -> bool {
            false
        }
        async fn call(
            &self,
            _: Value,
            _: &ToolUseContext,
            _: &crate::types::message::AssistantMessage,
            _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                data: Value::Null,
                new_messages: vec![],
                ..Default::default()
            })
        }
        async fn prompt(&self) -> String {
            String::new()
        }
    }

    // ── TrackedToolState tests ────────────────────────────────────────────

    #[test]
    fn test_tracked_tool_state_eq() {
        assert_eq!(TrackedToolState::Queued, TrackedToolState::Queued);
        assert_eq!(TrackedToolState::Executing, TrackedToolState::Executing);
        assert_eq!(TrackedToolState::Completed, TrackedToolState::Completed);
        assert_eq!(TrackedToolState::Yielded, TrackedToolState::Yielded);
    }

    #[test]
    fn test_tracked_tool_state_ne() {
        assert_ne!(TrackedToolState::Queued, TrackedToolState::Executing);
        assert_ne!(TrackedToolState::Executing, TrackedToolState::Completed);
        assert_ne!(TrackedToolState::Completed, TrackedToolState::Yielded);
        assert_ne!(TrackedToolState::Queued, TrackedToolState::Yielded);
    }

    #[test]
    fn test_tracked_tool_state_clone() {
        let s = TrackedToolState::Executing;
        let cloned = s.clone();
        assert_eq!(s, cloned);
    }

    #[test]
    fn test_tracked_tool_state_debug() {
        let s = format!("{:?}", TrackedToolState::Queued);
        assert_eq!(s, "Queued");
        let s2 = format!("{:?}", TrackedToolState::Yielded);
        assert_eq!(s2, "Yielded");
    }

    // ── TrackedTool struct construction ───────────────────────────────────

    #[test]
    fn test_tracked_tool_construction() {
        let tt = TrackedTool {
            tool_use_id: "id-abc".to_string(),
            tool_name: "Grep".to_string(),
            input: json!({"pattern": "foo"}),
            state: TrackedToolState::Queued,
            result: None,
            is_concurrency_safe: true,
        };
        assert_eq!(tt.tool_use_id, "id-abc");
        assert_eq!(tt.tool_name, "Grep");
        assert_eq!(tt.state, TrackedToolState::Queued);
        assert!(tt.is_concurrency_safe);
        assert!(tt.result.is_none());
    }

    #[test]
    fn test_tracked_tool_debug() {
        let tt = TrackedTool {
            tool_use_id: "x".to_string(),
            tool_name: "Bash".to_string(),
            input: json!(null),
            state: TrackedToolState::Completed,
            result: None,
            is_concurrency_safe: false,
        };
        let dbg = format!("{:?}", tt);
        assert!(dbg.contains("Bash"));
        assert!(dbg.contains("Completed"));
    }

    // ── StreamingToolExecutor::new() ──────────────────────────────────────

    #[test]
    fn test_executor_new_initial_state() {
        let executor = StreamingToolExecutor::new();
        // No bash error initially
        assert!(!executor.has_bash_error());
        // No tracked tools
        assert!(executor.tracked.is_empty());
    }

    #[test]
    fn test_executor_with_hook_configs_empty() {
        let executor = StreamingToolExecutor::with_hook_configs(vec![]);
        assert!(!executor.has_bash_error());
        assert!(executor.tracked.is_empty());
        assert!(executor.hook_configs.is_empty());
    }

    #[test]
    fn test_executor_with_hook_configs_preserves_configs() {
        let cfg = HookEventConfig {
            matcher: Some("Bash".to_string()),
            hooks: vec![],
        };
        let executor = StreamingToolExecutor::with_hook_configs(vec![cfg]);
        assert_eq!(executor.hook_configs.len(), 1);
    }

    // ── add_tool(): concurrency-safe flag assignment ───────────────────────

    #[test]
    fn test_add_tool_concurrent_safe_flag_true() {
        let mut executor = StreamingToolExecutor::new();
        let tools: crate::types::tool::Tools = vec![Arc::new(ConcurrentStub)];

        executor.add_tool(
            "id-1".to_string(),
            "ConcurrentStub".to_string(),
            json!({}),
            &tools,
        );

        assert_eq!(executor.tracked.len(), 1);
        assert!(
            executor.tracked[0].is_concurrency_safe,
            "ConcurrentStub must be marked concurrency-safe"
        );
        assert_eq!(executor.tracked[0].tool_use_id, "id-1");
        assert_eq!(executor.tracked[0].state, TrackedToolState::Queued);
    }

    #[test]
    fn test_add_tool_serial_flag_false() {
        let mut executor = StreamingToolExecutor::new();
        let tools: crate::types::tool::Tools = vec![Arc::new(SerialStub)];

        executor.add_tool(
            "id-2".to_string(),
            "SerialStub".to_string(),
            json!({}),
            &tools,
        );

        assert_eq!(executor.tracked.len(), 1);
        assert!(
            !executor.tracked[0].is_concurrency_safe,
            "SerialStub must NOT be concurrency-safe"
        );
    }

    #[test]
    fn test_add_tool_unknown_name_defaults_to_not_safe() {
        let mut executor = StreamingToolExecutor::new();
        // Empty tools list — tool name won't be found, map_or(false, ...) kicks in
        let tools: crate::types::tool::Tools = vec![];

        executor.add_tool(
            "id-3".to_string(),
            "UnknownTool".to_string(),
            json!({}),
            &tools,
        );

        assert_eq!(executor.tracked.len(), 1);
        assert!(
            !executor.tracked[0].is_concurrency_safe,
            "Unknown tools must default to not concurrency-safe"
        );
    }

    #[test]
    fn test_add_multiple_tools_fifo_order() {
        let mut executor = StreamingToolExecutor::new();
        let tools: crate::types::tool::Tools = vec![Arc::new(ConcurrentStub), Arc::new(SerialStub)];

        executor.add_tool(
            "id-a".to_string(),
            "ConcurrentStub".to_string(),
            json!({}),
            &tools,
        );
        executor.add_tool(
            "id-b".to_string(),
            "SerialStub".to_string(),
            json!({}),
            &tools,
        );
        executor.add_tool(
            "id-c".to_string(),
            "ConcurrentStub".to_string(),
            json!({}),
            &tools,
        );

        assert_eq!(executor.tracked.len(), 3);
        assert_eq!(executor.tracked[0].tool_use_id, "id-a");
        assert_eq!(executor.tracked[1].tool_use_id, "id-b");
        assert_eq!(executor.tracked[2].tool_use_id, "id-c");

        assert!(executor.tracked[0].is_concurrency_safe);
        assert!(!executor.tracked[1].is_concurrency_safe);
        assert!(executor.tracked[2].is_concurrency_safe);
    }

    #[test]
    fn test_add_tool_initial_state_is_queued() {
        let mut executor = StreamingToolExecutor::new();
        let tools: crate::types::tool::Tools = vec![Arc::new(SerialStub)];

        executor.add_tool(
            "id-q".to_string(),
            "SerialStub".to_string(),
            json!({}),
            &tools,
        );

        assert_eq!(
            executor.tracked[0].state,
            TrackedToolState::Queued,
            "Freshly added tools must start in Queued state"
        );
    }

    #[test]
    fn test_add_tool_stores_input() {
        let mut executor = StreamingToolExecutor::new();
        let tools: crate::types::tool::Tools = vec![Arc::new(ConcurrentStub)];
        let input = json!({"key": "value", "num": 42});

        executor.add_tool(
            "id-inp".to_string(),
            "ConcurrentStub".to_string(),
            input.clone(),
            &tools,
        );

        assert_eq!(executor.tracked[0].input, input);
    }

    // ── has_bash_error() getter ────────────────────────────────────────────

    #[test]
    fn test_has_bash_error_initially_false() {
        let executor = StreamingToolExecutor::new();
        assert!(!executor.has_bash_error());
    }

    #[test]
    fn test_has_bash_error_can_be_set_directly() {
        // Test the internal flag by direct construction (white-box)
        let executor = StreamingToolExecutor {
            tracked: vec![],
            has_bash_error: true,
            hook_configs: vec![],
        };
        assert!(executor.has_bash_error());
    }
}
