/// Core query loop -- the heart of the system.
///
/// Corresponds to TypeScript: query.ts's query() async generator.
///
/// Structure:
///   while true {
///     1. SETUP -- destructure state, increment count
///     2. CONTEXT -- apply tool result budget, microcompact, autocompact
///     3. API CALL -- streaming model call, collect assistant message + tool use blocks
///     4. POST-STREAMING -- check abort, handle pending summary
///     5. TERMINAL CHECK (no tool calls):
///        - prompt_too_long recovery
///        - max_output_tokens recovery
///        - stop hooks
///        - token budget check
///     6. TOOL EXECUTION (has tool calls):
///        - partition into concurrent/serial batches
///        - execute tools
///        - check abort during execution
///     7. ATTACHMENTS -- inject file changes, memory, skill discovery
///     8. CONTINUE -- refresh tools, check maxTurns, state = next
///   }
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::types::config::QueryParams;
use crate::types::message::QueryYield;
use crate::types::message::{
    Attachment, AttachmentMessage, ContentBlock, Message, MessageContent, RequestStartEvent,
    StreamEvent, ToolResultContent, Usage, UserMessage,
};
use crate::types::state::{BudgetTracker, QueryLoopState, TokenBudgetDecision};
use crate::types::transitions::Continue;

use crate::services::tool_use_summary::{self, ToolInfo};

use super::deps::{ModelCallParams, QueryDeps};
use super::loop_helpers::{
    execute_tool_calls, handle_max_output_tokens, handle_prompt_too_long, make_abort_message,
    make_error_message, make_user_message, MaxTokensRecovery, PromptRecovery,
};
use super::stop_hooks::{self, StopHookResult};
use super::token_budget::check_token_budget;

/// query() -- core query loop.
///
/// Takes query parameters and dependency injection, returns a Stream yielding `QueryYield`.
/// The caller (QueryEngine) consumes this stream to drive UI updates and message collection.
pub fn query(params: QueryParams, deps: Arc<dyn QueryDeps>) -> impl Stream<Item = QueryYield> {
    stream! {
        // ──────────────────────────────────────────────────────────
        // Initialization
        // ──────────────────────────────────────────────────────────

        let mut state = QueryLoopState::initial(params.messages);
        let system_prompt = params.system_prompt;
        let max_turns = params.max_turns;
        let task_budget = params.task_budget.as_ref().map(|b| b.total);
        let query_source = params.query_source;
        let skip_cache_write = params.skip_cache_write;
        let mut budget_tracker = BudgetTracker::new();
        let mut cumulative_usage = Usage::default();

        // Main loop
        loop {
            // ──────────────────────────────────────────────────────
            // STEP 1: SETUP
            // ──────────────────────────────────────────────────────

            let turn_count = state.turn_count;
            debug!(turn = turn_count, "query loop iteration start");

            // Emit query.turn.start audit event
            let turn_audit_ctx = deps.audit_context().with_turn();
            {
                use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                turn_audit_ctx.emit(
                    EventKind::QueryTurnStart,
                    Stage::QueryTurn,
                    AuditLevel::Info,
                    Outcome::Started,
                    None,
                    Some(serde_json::json!({
                        "turn": turn_count,
                        "messages_count": state.messages.len(),
                    })),
                );
            }

            if deps.is_aborted() {
                info!("aborted before API call");
                yield QueryYield::Message(Message::Assistant(make_abort_message(
                    &deps,
                    "AbortedStreaming",
                )));
                break;
            }

            // ──────────────────────────────────────────────────────
            // STEP 1b: Inject completed background agent results
            // ──────────────────────────────────────────────────────

            let completed_agents = deps.drain_background_results();
            for agent in &completed_agents {
                let content = if agent.had_error {
                    format!(
                        "[Background agent '{}' (id: {}) failed after {:.1}s]\n\n{}",
                        agent.description,
                        agent.agent_id,
                        agent.duration.as_secs_f64(),
                        agent.result_text,
                    )
                } else {
                    format!(
                        "[Background agent '{}' (id: {}) completed in {:.1}s]\n\n{}",
                        agent.description,
                        agent.agent_id,
                        agent.duration.as_secs_f64(),
                        agent.result_text,
                    )
                };

                let sys_msg = Message::System(crate::types::message::SystemMessage {
                    uuid: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    subtype: crate::types::message::SystemSubtype::Informational {
                        level: crate::types::message::InfoLevel::Info,
                    },
                    content,
                });
                yield QueryYield::Message(sys_msg.clone());
                state.messages.push(sys_msg);
            }

            // ──────────────────────────────────────────────────────
            // STEP 2: CONTEXT -- microcompact + autocompact
            // ──────────────────────────────────────────────────────

            let messages = match deps.microcompact(state.messages.clone()).await {
                Ok(msgs) => msgs,
                Err(e) => {
                    warn!(error = %e, "microcompact failed, using original messages");
                    state.messages.clone()
                }
            };

            let message_count_before = messages.len();

            // Fire PreCompact hook before autocompact
            {
                let hooks_map = deps.get_app_state().hooks;
                let compact_pre_configs = crate::tools::hooks::load_hook_configs(&hooks_map, "PreCompact");
                if !compact_pre_configs.is_empty() {
                    let payload = serde_json::json!({
                        "message_count": message_count_before,
                    });
                    let _ = crate::tools::hooks::run_event_hooks("PreCompact", &payload, &compact_pre_configs).await;
                }
            }

            let (messages, auto_compact_tracking) = match deps
                .autocompact(messages.clone(), state.auto_compact_tracking.clone())
                .await
            {
                Ok(Some(result)) => {
                    debug!("autocompact produced compacted messages");

                    // Fire PostCompact hook after successful compaction
                    {
                        let hooks_map = deps.get_app_state().hooks;
                        let compact_post_configs = crate::tools::hooks::load_hook_configs(&hooks_map, "PostCompact");
                        if !compact_post_configs.is_empty() {
                            let message_count_after = result.messages.len();
                            let messages_freed = message_count_before.saturating_sub(message_count_after);
                            let payload = serde_json::json!({
                                "message_count_before": message_count_before,
                                "message_count_after": message_count_after,
                                "messages_freed": messages_freed,
                            });
                            let _ = crate::tools::hooks::run_event_hooks("PostCompact", &payload, &compact_post_configs).await;
                        }
                    }

                    (result.messages, Some(result.tracking))
                }
                Ok(None) => (messages, state.auto_compact_tracking.clone()),
                Err(e) => {
                    warn!(error = %e, "autocompact failed, using original messages");
                    (messages, state.auto_compact_tracking.clone())
                }
            };

            state.messages = messages;
            state.auto_compact_tracking = auto_compact_tracking;

            // ──────────────────────────────────────────────────────
            // STEP 3: API CALL -- streaming model call
            // ──────────────────────────────────────────────────────

            yield QueryYield::RequestStart(RequestStartEvent);

            let tools = deps.get_tools();

            let call_params = ModelCallParams {
                messages: state.messages.clone(),
                system_prompt: system_prompt.clone(),
                tools: tools.clone(),
                model: Some(deps.get_app_state().main_loop_model.clone()),
                max_output_tokens: state.max_output_tokens_override,
                skip_cache_write,
                thinking_enabled: deps.get_app_state().thinking_enabled,
                effort_value: deps.get_app_state().effort_value.clone(),
                advisor_model: deps.get_app_state().advisor_model.clone(),
            };
            let model_for_langfuse = call_params
                .model
                .clone()
                .unwrap_or_else(|| deps.get_app_state().main_loop_model.clone());
            let provider_for_langfuse = deps
                .langfuse_provider_name()
                .unwrap_or_else(|| "unknown".to_string());
            let generation_input = crate::services::langfuse::convert::convert_generation_input(
                &call_params.messages,
                &call_params.system_prompt,
                &call_params.tools,
            );
            let mut generation_span = deps.langfuse_trace().as_ref().and_then(|trace| {
                crate::services::langfuse::create_generation_span(
                    trace,
                    &model_for_langfuse,
                    &provider_for_langfuse,
                    generation_input,
                )
            });

            // Emit model.request.start audit event
            let req_audit_ctx = turn_audit_ctx.with_request();
            {
                use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                req_audit_ctx.emit(
                    EventKind::ModelRequestStart,
                    Stage::ModelCall,
                    AuditLevel::Info,
                    Outcome::Started,
                    None,
                    None,
                );
            }
            let model_call_start = std::time::Instant::now();

            let stream_result = deps.call_model_streaming(call_params).await;
            let mut event_stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    let error_str = e.to_string();
                    crate::services::langfuse::finish_generation_span(
                        generation_span.take(),
                        None,
                        None,
                        None,
                        Some(&error_str),
                    );

                    if error_str.contains("prompt_too_long") || error_str.contains("prompt is too long") {
                        let terminal = handle_prompt_too_long(
                            &deps,
                            &mut state,
                            &error_str,
                        ).await;

                        match terminal {
                            PromptRecovery::Continue(reason) => {
                                state.transition = Some(reason);
                                continue;
                            }
                            PromptRecovery::Terminal(_term) => {
                                yield QueryYield::Message(Message::Assistant(
                                    make_error_message(&deps, &error_str),
                                ));
                                break;
                            }
                        }
                    }

                    warn!(error = %e, "model call failed");
                    yield QueryYield::Message(Message::Assistant(
                        make_error_message(&deps, &error_str),
                    ));
                    break;
                }
            };

            // Consume stream events, forwarding to caller while accumulating
            let mut accumulator = crate::api::streaming::StreamAccumulator::new();
            let mut stream_error: Option<String> = None;
            let mut first_response_at: Option<std::time::Instant> = None;

            use futures::StreamExt;
            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        if first_response_at.is_none()
                            && matches!(
                                &event,
                                StreamEvent::ContentBlockStart { .. }
                                    | StreamEvent::ContentBlockDelta { .. }
                            )
                        {
                            first_response_at = Some(std::time::Instant::now());
                        }
                        accumulator.process_event(&event);
                        yield QueryYield::Stream(event);
                    }
                    Err(e) => {
                        stream_error = Some(e.to_string());
                        break;
                    }
                }
            }

            if let Some(ref err) = stream_error {
                let ttft_ms = first_response_at
                    .map(|instant| instant.duration_since(model_call_start).as_millis() as u64);
                crate::services::langfuse::finish_generation_span(
                    generation_span.take(),
                    None,
                    None,
                    ttft_ms,
                    Some(err),
                );
                warn!(error = %err, "stream error during model call");
                {
                    use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                    req_audit_ctx.emit(
                        EventKind::ModelRequestError,
                        Stage::ModelCall,
                        AuditLevel::Error,
                        Outcome::Failed,
                        Some(model_call_start.elapsed().as_millis() as u64),
                        Some(serde_json::json!({"error": err})),
                    );
                }
                yield QueryYield::Message(Message::Assistant(
                    make_error_message(&deps, err),
                ));
                break;
            }

            let effective_model = deps.get_app_state().main_loop_model;
            let assistant_message = accumulator.build(&effective_model);
            let ttft_ms = first_response_at
                .map(|instant| instant.duration_since(model_call_start).as_millis() as u64);
            crate::services::langfuse::finish_generation_span(
                generation_span.take(),
                Some(crate::services::langfuse::convert::convert_assistant_output(
                    &assistant_message,
                )),
                assistant_message.usage.as_ref(),
                ttft_ms,
                None,
            );

            // Emit model.request.finish audit event
            {
                use crate::observability::{AuditLevel, EventKind, Outcome, Stage};
                let model_duration = model_call_start.elapsed().as_millis() as u64;
                req_audit_ctx.emit(
                    EventKind::ModelRequestFinish,
                    Stage::ModelCall,
                    AuditLevel::Info,
                    Outcome::Completed,
                    Some(model_duration),
                    Some(serde_json::json!({
                        "stop_reason": assistant_message.stop_reason,
                        "tool_use_count": assistant_message.content.iter()
                            .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                            .count(),
                    })),
                );
            }

            // Accumulate usage
            if let Some(ref usage) = assistant_message.usage {
                cumulative_usage.input_tokens += usage.input_tokens;
                cumulative_usage.output_tokens += usage.output_tokens;
                cumulative_usage.cache_read_input_tokens += usage.cache_read_input_tokens;
                cumulative_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            }

            // ──────────────────────────────────────────────────────
            // STEP 4: POST-STREAMING -- check abort, pending summary
            // ──────────────────────────────────────────────────────

            if deps.is_aborted() {
                info!("aborted after streaming");
                yield QueryYield::Message(Message::Assistant(assistant_message));
                break;
            }

            // Inject pending tool use summary as system message
            if let Some(summary) = state.pending_tool_use_summary.take() {
                debug!(summary = %crate::utils::messages::truncate_text(&summary, 200), "injecting tool use summary");
                let sys_msg = Message::System(crate::types::message::SystemMessage {
                    uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    subtype: crate::types::message::SystemSubtype::Informational {
                        level: crate::types::message::InfoLevel::Info,
                    },
                    content: format!("[tool summary] {}", summary),
                });
                state.messages.push(sys_msg);
            }

            // Yield assistant message
            yield QueryYield::Message(Message::Assistant(assistant_message.clone()));
            state.messages.push(Message::Assistant(assistant_message.clone()));

            // ──────────────────────────────────────────────────────
            // STEP 5 vs 6: Branch -- tool calls or not
            // ──────────────────────────────────────────────────────

            let tool_uses = stop_hooks::extract_tool_uses(&assistant_message);

            if tool_uses.is_empty() {
                // ── TERMINAL CHECK (no tool calls) ──

                // 5a. max_output_tokens recovery
                if assistant_message.stop_reason.as_deref() == Some("max_tokens") {
                    let recovery = handle_max_output_tokens(
                        &deps,
                        &mut state,
                        &assistant_message,
                    );

                    match recovery {
                        MaxTokensRecovery::Continue(reason) => {
                            state.transition = Some(reason);
                            continue;
                        }
                        MaxTokensRecovery::Terminal => {
                            break;
                        }
                    }
                }

                // 5b. stop hooks
                let hooks_map = deps.get_app_state().hooks;
                let stop_configs = crate::tools::hooks::load_hook_configs(&hooks_map, "Stop");

                let stop_result = stop_hooks::run_stop_hooks(
                    &assistant_message,
                    &state.messages,
                    state.stop_hook_active,
                    &stop_configs,
                )
                .await;

                match stop_result {
                    Ok(StopHookResult::PreventStop { continuation_message }) => {
                        let user_msg = make_user_message(
                            &deps,
                            &continuation_message,
                            true,
                        );
                        state.messages.push(Message::User(user_msg));
                        state.stop_hook_active = Some(true);
                        state.transition = Some(Continue::StopHookBlocking);
                        state.turn_count += 1;
                        continue;
                    }
                    Ok(StopHookResult::BlockingError { error }) => {
                        warn!(error = %error, "stop hook blocking error");

                        // Fire StopFailure hook
                        let sf_configs = crate::tools::hooks::load_hook_configs(&hooks_map, "StopFailure");
                        if !sf_configs.is_empty() {
                            let payload = serde_json::json!({ "error": error });
                            let _ = crate::tools::hooks::run_event_hooks("StopFailure", &payload, &sf_configs).await;
                        }

                        break;
                    }
                    Ok(StopHookResult::AllowStop) => {}
                    Err(e) => {
                        warn!(error = %e, "stop hook execution error");
                    }
                }

                // 5c. token budget check
                let global_turn_tokens = cumulative_usage.output_tokens;
                let budget_decision = check_token_budget(
                    &mut budget_tracker,
                    if query_source.starts_with_agent() { Some("agent") } else { None },
                    task_budget,
                    global_turn_tokens,
                );

                match budget_decision {
                    TokenBudgetDecision::Continue {
                        nudge_message,
                        continuation_count,
                        ..
                    } => {
                        debug!(
                            continuation = continuation_count,
                            "token budget: continuing"
                        );
                        let user_msg = make_user_message(&deps, &nudge_message, true);
                        state.messages.push(Message::User(user_msg));
                        state.transition = Some(Continue::TokenBudgetContinuation);
                        state.turn_count += 1;
                        continue;
                    }
                    TokenBudgetDecision::Stop { completion_event } => {
                        if let Some(ref event) = completion_event {
                            debug!(
                                pct = event.pct,
                                turns = event.continuation_count,
                                "token budget: stopping"
                            );
                        }
                        break;
                    }
                }
            } else {
                // ── STEP 6: TOOL EXECUTION ──

                let tool_results = execute_tool_calls(
                    &deps,
                    &tool_uses,
                    &tools,
                    &assistant_message,
                )
                .await;

                if deps.is_aborted() {
                    info!("aborted during tool execution");
                    break;
                }

                // Convert tool results to user messages
                for exec_result in &tool_results {
                    // Use structured model_content when available (e.g. images from MCP),
                    // otherwise fall back to text-only content.
                    let (tr_content, display_text) = if exec_result.is_error {
                        let text = format!("Error: {}", exec_result.result.data);
                        (ToolResultContent::Text(text.clone()), text)
                    } else if let Some(ref model_content) = exec_result.result.model_content {
                        let preview = exec_result.result.display_preview
                            .clone()
                            .unwrap_or_else(|| exec_result.result.data.to_string());
                        (model_content.clone(), preview)
                    } else {
                        let text = exec_result.result.data.to_string();
                        (ToolResultContent::Text(text.clone()), text)
                    };

                    let tool_result_block = ContentBlock::ToolResult {
                        tool_use_id: exec_result.tool_use_id.clone(),
                        content: tr_content,
                        is_error: exec_result.is_error,
                    };

                    let user_msg = UserMessage {
                        uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        role: "user".to_string(),
                        content: MessageContent::Blocks(vec![tool_result_block]),
                        is_meta: true,
                        tool_use_result: Some(display_text),
                        source_tool_assistant_uuid: Some(assistant_message.uuid),
                    };

                    let msg = Message::User(user_msg);
                    yield QueryYield::Message(msg.clone());
                    state.messages.push(msg);

                    for sub_msg in &exec_result.result.new_messages {
                        yield QueryYield::Message(sub_msg.clone());
                        state.messages.push(sub_msg.clone());
                    }
                }

                // ── STEP 6b: Generate tool use summary ──
                let tool_infos: Vec<ToolInfo> = tool_results
                    .iter()
                    .map(|r| ToolInfo {
                        name: r.tool_name.clone(),
                        input_summary: r.result.data.to_string(),
                        output_summary: if r.is_error {
                            format!("Error: {}", r.result.data)
                        } else {
                            r.result.data.to_string()
                        },
                    })
                    .collect();

                let last_text = assistant_message.content.iter().find_map(|b| {
                    if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
                });

                if let Some(summary) = tool_use_summary::generate_tool_use_summary(
                    &tool_infos,
                    last_text,
                ) {
                    state.pending_tool_use_summary = Some(summary);
                }

                // ── STEP 7: ATTACHMENTS (placeholder) ──

                // ── STEP 8: CONTINUE -- refresh tools, check maxTurns ──

                if let Some(max) = max_turns {
                    if state.turn_count >= max {
                        info!(turns = state.turn_count, max = max, "max turns reached");
                        let attachment_msg = AttachmentMessage {
                            uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            attachment: Attachment::MaxTurnsReached {
                                max_turns: max,
                                turn_count: state.turn_count,
                            },
                        };
                        yield QueryYield::Message(Message::Attachment(attachment_msg));
                        break;
                    }
                }

                match deps.refresh_tools().await {
                    Ok(_refreshed) => {
                        debug!("tools refreshed successfully");
                    }
                    Err(e) => {
                        debug!(error = %e, "tool refresh failed, continuing with existing tools");
                    }
                }

                state.transition = Some(Continue::NextTurn);
                state.turn_count += 1;
                state.stop_hook_active = None;
                continue;
            }
        }

        info!(turns = state.turn_count, "query loop finished");
    }
}

#[cfg(test)]
#[path = "loop_tests.rs"]
mod loop_tests;
