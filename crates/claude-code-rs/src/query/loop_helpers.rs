//! Helper types and functions for the query loop.
//!
//! Extracted from loop_impl.rs to keep the core stream! macro body focused.

use std::sync::Arc;

use tracing::{debug, warn};
use uuid::Uuid;

use crate::types::message::{AssistantMessage, ContentBlock, Message, MessageContent, UserMessage};
use crate::types::state::QueryLoopState;
use crate::types::tool::ToolProgress;
use crate::types::transitions::{Continue, Terminal};

use super::deps::{QueryDeps, ToolExecRequest, ToolExecResult};

/// Maximum number of max_output_tokens recovery attempts.
pub(crate) const MAX_OUTPUT_TOKENS_RECOVERY_LIMIT: usize = 3;

/// Escalated max output tokens (8k -> 64k).
pub(crate) const ESCALATED_MAX_TOKENS: usize = 64_000;

type ToolUseTuple = (String, String, serde_json::Value);
type ToolUseBatch = (bool, Vec<ToolUseTuple>);

/// prompt_too_long recovery result.
#[allow(unused)]
pub(crate) enum PromptRecovery {
    Continue(Continue),
    Terminal(Terminal),
}

/// max_output_tokens recovery result.
#[allow(unused)]
pub(crate) enum MaxTokensRecovery {
    Continue(Continue),
    Terminal,
}

/// Handle prompt_too_long error recovery.
///
/// Three-step recovery:
/// 1. collapse drain -- remove oldest non-critical messages
/// 2. reactive compact -- emergency compaction
/// 3. unrecoverable -- return error
#[allow(unused)]
pub(crate) async fn handle_prompt_too_long(
    deps: &Arc<dyn QueryDeps>,
    state: &mut QueryLoopState,
    error: &str,
) -> PromptRecovery {
    if !state.has_attempted_reactive_compact {
        debug!("prompt_too_long: attempting reactive compact");
        state.has_attempted_reactive_compact = true;

        match deps.reactive_compact(state.messages.clone()).await {
            Ok(Some(result)) => {
                state.messages = result.messages;
                state.auto_compact_tracking = Some(result.tracking);
                return PromptRecovery::Continue(Continue::ReactiveCompactRetry);
            }
            Ok(None) => {
                debug!("reactive compact returned None, cannot recover");
            }
            Err(e) => {
                warn!(error = %e, "reactive compact failed");
            }
        }
    }

    PromptRecovery::Terminal(Terminal::PromptTooLong)
}

/// Handle max_output_tokens recovery.
///
/// Three-step recovery:
/// 1. escalate -- increase max_output_tokens to ESCALATED_MAX_TOKENS
/// 2. recovery message -- inject "continue from where you left off"
/// 3. reached recovery limit -- terminate
pub(crate) fn handle_max_output_tokens(
    deps: &Arc<dyn QueryDeps>,
    state: &mut QueryLoopState,
    _assistant_message: &AssistantMessage,
) -> MaxTokensRecovery {
    if state.max_output_tokens_override.is_none() {
        debug!("max_output_tokens: escalating to {}", ESCALATED_MAX_TOKENS);
        state.max_output_tokens_override = Some(ESCALATED_MAX_TOKENS);
        state.transition = Some(Continue::MaxOutputTokensEscalate);
        return MaxTokensRecovery::Continue(Continue::MaxOutputTokensEscalate);
    }

    if state.max_output_tokens_recovery_count < MAX_OUTPUT_TOKENS_RECOVERY_LIMIT {
        state.max_output_tokens_recovery_count += 1;
        let attempt = state.max_output_tokens_recovery_count;
        debug!(attempt, "max_output_tokens: recovery attempt");

        let recovery_msg = make_user_message(
            deps,
            "Your response was cut off due to output length limits. Please continue from where you left off.",
            true,
        );
        state.messages.push(Message::User(recovery_msg));
        state.turn_count += 1;
        return MaxTokensRecovery::Continue(Continue::MaxOutputTokensRecovery { attempt });
    }

    debug!("max_output_tokens: recovery limit reached, terminating");
    MaxTokensRecovery::Terminal
}

/// Execute tool calls (batched: concurrency-safe ones together, rest serial).
///
/// Keep this helper thin: it owns batching/order only, then routes every tool
/// call through the canonical [`QueryDeps::execute_tool`] boundary.
pub(crate) async fn execute_tool_calls(
    deps: &Arc<dyn QueryDeps>,
    tool_uses: &[(String, String, serde_json::Value)],
    tools: &crate::types::tool::Tools,
    parent_message: &AssistantMessage,
    on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
) -> Vec<ToolExecResult> {
    let mut results = Vec::new();

    // Partition: consecutive concurrency-safe tools -> one concurrent batch, rest serial
    let mut batches: Vec<ToolUseBatch> = Vec::new();

    for (id, name, input) in tool_uses {
        let tool = tools.iter().find(|t| t.name() == name);
        let is_safe = tool.is_some_and(|t| t.is_concurrency_safe(input));

        if is_safe {
            if let Some(last) = batches.last_mut() {
                if last.0 {
                    last.1.push((id.clone(), name.clone(), input.clone()));
                    continue;
                }
            }
        }

        batches.push((is_safe, vec![(id.clone(), name.clone(), input.clone())]));
    }

    for (batch_index, (is_concurrent, batch)) in batches.into_iter().enumerate() {
        let batch_tool_names = batch
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect::<Vec<String>>();
        let batch_span = if is_concurrent && batch.len() > 1 {
            deps.langfuse_trace().as_ref().and_then(|trace| {
                crate::services::langfuse::create_tool_batch_span(
                    trace,
                    &batch_tool_names,
                    batch_index,
                )
            })
        } else {
            None
        };
        if is_concurrent && batch.len() > 1 {
            // Concurrent execution
            let mut handles = Vec::new();
            for (id, name, input) in batch {
                let deps = deps.clone();
                let parent = parent_message.clone();
                let tools = tools.clone();
                let batch_span = batch_span.clone();
                let on_progress_clone = on_progress.clone();
                let handle = tokio::spawn(async move {
                    let req = ToolExecRequest {
                        tool_use_id: id,
                        tool_name: name,
                        input,
                        langfuse_batch_span: batch_span,
                    };
                    deps.execute_tool(req, &tools, &parent, on_progress_clone)
                        .await
                });
                handles.push(handle);
            }

            for handle in handles {
                match handle.await {
                    Ok(Ok(result)) => results.push(result),
                    Ok(Err(e)) => {
                        warn!(error = %e, "tool execution error");
                        results.push(ToolExecResult {
                            tool_use_id: "unknown".to_string(),
                            tool_name: "unknown".to_string(),
                            result: crate::types::tool::ToolResult {
                                data: serde_json::json!(format!("Internal error: {}", e)),
                                new_messages: vec![],
                                ..Default::default()
                            },
                            is_error: true,
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "tool task panicked");
                    }
                }
            }
        } else {
            // Serial execution
            for (id, name, input) in batch {
                let req = ToolExecRequest {
                    tool_use_id: id,
                    tool_name: name,
                    input,
                    langfuse_batch_span: None,
                };
                match deps
                    .execute_tool(req, tools, parent_message, on_progress.clone())
                    .await
                {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        warn!(error = %e, "tool execution error");
                        results.push(ToolExecResult {
                            tool_use_id: "unknown".to_string(),
                            tool_name: "unknown".to_string(),
                            result: crate::types::tool::ToolResult {
                                data: serde_json::json!(format!("Internal error: {}", e)),
                                new_messages: vec![],
                                ..Default::default()
                            },
                            is_error: true,
                        });
                    }
                }
            }
        }
        crate::services::langfuse::end_span(batch_span);
    }

    results
}

/// Create an abort placeholder assistant message.
pub(crate) fn make_abort_message(deps: &Arc<dyn QueryDeps>, reason: &str) -> AssistantMessage {
    AssistantMessage {
        uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
        timestamp: chrono::Utc::now().timestamp_millis(),
        role: "assistant".to_string(),
        content: vec![],
        usage: None,
        stop_reason: Some(reason.to_string()),
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    }
}

/// Create an API error assistant message.
pub(crate) fn make_error_message(deps: &Arc<dyn QueryDeps>, error: &str) -> AssistantMessage {
    AssistantMessage {
        uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
        timestamp: chrono::Utc::now().timestamp_millis(),
        role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: format!("API error: {}", error),
        }],
        usage: None,
        stop_reason: Some("error".to_string()),
        is_api_error_message: true,
        api_error: Some(error.to_string()),
        cost_usd: 0.0,
    }
}

/// Create a system-injected user message.
pub(crate) fn make_user_message(
    deps: &Arc<dyn QueryDeps>,
    content: &str,
    is_meta: bool,
) -> UserMessage {
    UserMessage {
        uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
        timestamp: chrono::Utc::now().timestamp_millis(),
        role: "user".to_string(),
        content: MessageContent::Text(content.to_string()),
        is_meta,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use futures::Stream;
    use serde_json::Value;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use crate::query::deps::{CompactionResult, ModelCallParams, ModelResponse};
    use crate::types::app_state::AppState;
    use crate::types::message::{StreamEvent, Usage};
    use crate::types::state::AutoCompactTracking;
    use crate::types::tool::{Tool, ToolResult, ToolUseContext, Tools};

    struct BatchTool {
        name: &'static str,
        concurrency_safe: bool,
    }

    #[async_trait::async_trait]
    impl Tool for BatchTool {
        fn name(&self) -> &str {
            self.name
        }

        async fn description(&self, _input: &Value) -> String {
            String::new()
        }

        fn input_json_schema(&self) -> Value {
            serde_json::json!({})
        }

        fn is_concurrency_safe(&self, _input: &Value) -> bool {
            self.concurrency_safe
        }

        async fn call(
            &self,
            _input: Value,
            _ctx: &ToolUseContext,
            _parent_message: &AssistantMessage,
            _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> Result<ToolResult> {
            Ok(ToolResult::default())
        }

        async fn prompt(&self) -> String {
            String::new()
        }
    }

    struct RecordingDeps {
        active: AtomicUsize,
        max_active: AtomicUsize,
        events: parking_lot::Mutex<Vec<String>>,
        aborted: AtomicBool,
    }

    impl RecordingDeps {
        fn new() -> Self {
            Self {
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                events: parking_lot::Mutex::new(Vec::new()),
                aborted: AtomicBool::new(false),
            }
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().clone()
        }
    }

    #[async_trait::async_trait]
    impl QueryDeps for RecordingDeps {
        async fn call_model(&self, _params: ModelCallParams) -> Result<ModelResponse> {
            anyhow::bail!("not used")
        }

        async fn call_model_streaming(
            &self,
            _params: ModelCallParams,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
            anyhow::bail!("not used")
        }

        async fn microcompact(&self, messages: Vec<Message>) -> Result<Vec<Message>> {
            Ok(messages)
        }

        async fn autocompact(
            &self,
            _messages: Vec<Message>,
            _tracking: Option<AutoCompactTracking>,
        ) -> Result<Option<CompactionResult>> {
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
            _tools: &Tools,
            _parent: &AssistantMessage,
            _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
        ) -> Result<ToolExecResult> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            self.events
                .lock()
                .push(format!("start:{}", request.tool_use_id));

            tokio::time::sleep(Duration::from_millis(25)).await;

            self.events
                .lock()
                .push(format!("end:{}", request.tool_use_id));
            self.active.fetch_sub(1, Ordering::SeqCst);

            Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: ToolResult {
                    data: serde_json::json!("ok"),
                    new_messages: vec![],
                    ..Default::default()
                },
                is_error: false,
            })
        }

        fn get_app_state(&self) -> AppState {
            AppState::default()
        }

        fn uuid(&self) -> String {
            uuid::Uuid::new_v4().to_string()
        }

        fn is_aborted(&self) -> bool {
            self.aborted.load(Ordering::Relaxed)
        }

        fn get_tools(&self) -> Tools {
            vec![]
        }

        async fn refresh_tools(&self) -> Result<Tools> {
            Ok(vec![])
        }
    }

    fn parent_message() -> AssistantMessage {
        AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![],
            usage: Some(Usage::default()),
            stop_reason: Some("tool_use".to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }

    #[tokio::test]
    async fn execute_tool_calls_batches_consecutive_safe_tools_only() {
        let concrete_deps = Arc::new(RecordingDeps::new());
        let deps: Arc<dyn QueryDeps> = concrete_deps.clone();
        let tools: Tools = vec![
            Arc::new(BatchTool {
                name: "Safe",
                concurrency_safe: true,
            }),
            Arc::new(BatchTool {
                name: "Unsafe",
                concurrency_safe: false,
            }),
        ];
        let tool_uses = vec![
            (
                "safe_1".to_string(),
                "Safe".to_string(),
                serde_json::json!({}),
            ),
            (
                "safe_2".to_string(),
                "Safe".to_string(),
                serde_json::json!({}),
            ),
            (
                "unsafe_1".to_string(),
                "Unsafe".to_string(),
                serde_json::json!({}),
            ),
            (
                "safe_3".to_string(),
                "Safe".to_string(),
                serde_json::json!({}),
            ),
        ];

        let results = execute_tool_calls(&deps, &tool_uses, &tools, &parent_message(), None).await;

        assert_eq!(
            concrete_deps.max_active.load(Ordering::SeqCst),
            2,
            "only the consecutive safe batch should run concurrently"
        );
        assert_eq!(
            results
                .iter()
                .map(|result| result.tool_use_id.as_str())
                .collect::<Vec<_>>(),
            vec!["safe_1", "safe_2", "unsafe_1", "safe_3"],
            "results should stay in tool_use order"
        );

        let events = concrete_deps.events();
        let end_safe_1 = events
            .iter()
            .position(|event| event == "end:safe_1")
            .unwrap();
        let end_safe_2 = events
            .iter()
            .position(|event| event == "end:safe_2")
            .unwrap();
        let start_unsafe = events
            .iter()
            .position(|event| event == "start:unsafe_1")
            .unwrap();
        assert!(
            start_unsafe > end_safe_1 && start_unsafe > end_safe_2,
            "unsafe tool must not start until the preceding safe batch finishes: {:?}",
            events
        );

        let end_unsafe = events
            .iter()
            .position(|event| event == "end:unsafe_1")
            .unwrap();
        let start_safe_3 = events
            .iter()
            .position(|event| event == "start:safe_3")
            .unwrap();
        assert!(
            start_safe_3 > end_unsafe,
            "later safe tool must wait behind preceding unsafe tool: {:?}",
            events
        );
    }
}
