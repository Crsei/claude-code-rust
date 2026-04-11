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
        let is_safe = tool.map_or(false, |t| t.is_concurrency_safe(&input));

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
