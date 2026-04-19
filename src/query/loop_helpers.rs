//! Helper types and functions for the query loop.
//!
//! Extracted from loop_impl.rs to keep the core stream! macro body focused.

use std::sync::Arc;

use tracing::{debug, warn};
use uuid::Uuid;

use crate::types::message::{AssistantMessage, ContentBlock, Message, MessageContent, UserMessage};
use crate::types::state::QueryLoopState;
use crate::types::transitions::{Continue, Terminal};

use super::deps::{QueryDeps, ToolExecRequest, ToolExecResult};

/// Maximum number of max_output_tokens recovery attempts.
pub(crate) const MAX_OUTPUT_TOKENS_RECOVERY_LIMIT: usize = 3;

/// Escalated max output tokens (8k -> 64k).
pub(crate) const ESCALATED_MAX_TOKENS: usize = 64_000;

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
pub(crate) async fn execute_tool_calls(
    deps: &Arc<dyn QueryDeps>,
    tool_uses: &[(String, String, serde_json::Value)],
    tools: &crate::types::tool::Tools,
    parent_message: &AssistantMessage,
) -> Vec<ToolExecResult> {
    let mut results = Vec::new();

    // Partition: consecutive concurrency-safe tools -> one concurrent batch, rest serial
    let mut batches: Vec<(bool, Vec<(String, String, serde_json::Value)>)> = Vec::new();

    for (id, name, input) in tool_uses {
        let tool = tools.iter().find(|t| t.name() == name);
        let is_safe = tool.map_or(false, |t| t.is_concurrency_safe(input));

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

    let mut batch_index = 0usize;
    for (is_concurrent, batch) in batches {
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
                let handle = tokio::spawn(async move {
                    let req = ToolExecRequest {
                        tool_use_id: id,
                        tool_name: name,
                        input,
                        langfuse_batch_span: batch_span,
                    };
                    deps.execute_tool(req, &tools, &parent, None).await
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
                match deps.execute_tool(req, tools, parent_message, None).await {
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
        batch_index += 1;
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
