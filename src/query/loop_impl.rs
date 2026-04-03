/// 核心查询循环 — 整个系统的心脏
///
/// 对应 TypeScript: query.ts 的 query() async generator
///
/// 结构:
///   while true {
///     1. SETUP — 解构状态, 增加查询计数
///     2. CONTEXT — 应用工具结果预算, 微压缩, 自动压缩
///     3. API CALL — 流式调用模型, 收集助手消息 + 工具调用块
///     4. POST-STREAMING — 检查 abort, 处理挂起摘要
///     5. TERMINAL CHECK (无工具调用时):
///        - prompt_too_long 恢复 (collapse drain → reactive compact → error)
///        - max_output_tokens 恢复 (escalate → recovery message → error)
///        - stop hooks
///        - token budget check
///        - return Terminal
///     6. TOOL EXECUTION (有工具调用时):
///        - 分区为并发/串行批次
///        - 执行工具
///        - 检查执行期间 abort
///     7. ATTACHMENTS — 注入文件变更, 记忆, 技能发现
///     8. CONTINUE — 刷新工具, 检查 maxTurns, state = next
///   }
///
/// 在 Rust 中, TypeScript 的 AsyncGenerator 映射为 `impl Stream<Item = QueryYield>`.
/// 使用 `async_stream::stream!` 宏, 将 `yield` 映射为 stream 产出.

use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::types::config::QueryParams;
use crate::types::message::{
    AssistantMessage, Attachment, AttachmentMessage, ContentBlock, Message, MessageContent,
    RequestStartEvent, ToolResultContent, Usage, UserMessage,
};
use crate::types::state::{BudgetTracker, QueryLoopState, TokenBudgetDecision};
use crate::types::transitions::{Continue, Terminal};
use crate::types::message::QueryYield;

use super::deps::{ModelCallParams, QueryDeps};
use super::stop_hooks::{self, StopHookResult};
use super::token_budget::check_token_budget;

/// 最大输出 token 恢复尝试次数
const MAX_OUTPUT_TOKENS_RECOVERY_LIMIT: usize = 3;

/// 升级后的最大输出 token (8k → 64k)
const ESCALATED_MAX_TOKENS: usize = 64_000;

/// query() — 核心查询循环
///
/// 接受查询参数和依赖注入, 返回一个 Stream 产出 `QueryYield`.
/// 调用方 (QueryEngine) 消费此 stream 来驱动 UI 更新和消息收集.
///
/// # Arguments
/// * `params` - 查询参数 (消息, 系统提示, 配置等)
/// * `deps` - I/O 依赖 (API 客户端, 压缩器, 工具执行器等)
pub fn query(
    params: QueryParams,
    deps: Arc<dyn QueryDeps>,
) -> impl Stream<Item = QueryYield> {
    stream! {
        // ──────────────────────────────────────────────────────────
        // 初始化
        // ──────────────────────────────────────────────────────────

        let mut state = QueryLoopState::initial(params.messages);
        let system_prompt = params.system_prompt;
        let max_turns = params.max_turns;
        let task_budget = params.task_budget.as_ref().map(|b| b.total);
        let query_source = params.query_source;
        let skip_cache_write = params.skip_cache_write;
        let mut budget_tracker = BudgetTracker::new();
        let mut cumulative_usage = Usage::default();

        // 主循环 — 对应 TypeScript 的 while(true)
        loop {
            // ──────────────────────────────────────────────────────
            // STEP 1: SETUP — 解构状态
            // ──────────────────────────────────────────────────────

            let turn_count = state.turn_count;
            debug!(turn = turn_count, "query loop iteration start");

            // 检查 abort
            if deps.is_aborted() {
                info!("aborted before API call");
                yield QueryYield::Message(Message::Assistant(make_abort_message(
                    &deps,
                    "AbortedStreaming",
                )));
                break;
            }

            // ──────────────────────────────────────────────────────
            // STEP 2: CONTEXT — 微压缩 + 自动压缩
            // ──────────────────────────────────────────────────────

            // 微压缩: 裁剪过大的工具结果
            let messages = match deps.microcompact(state.messages.clone()).await {
                Ok(msgs) => msgs,
                Err(e) => {
                    warn!(error = %e, "microcompact failed, using original messages");
                    state.messages.clone()
                }
            };

            // 自动压缩: 达到 token 阈值时压缩历史
            let (messages, auto_compact_tracking) = match deps
                .autocompact(messages.clone(), state.auto_compact_tracking.clone())
                .await
            {
                Ok(Some(result)) => {
                    debug!("autocompact produced compacted messages");
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
            // STEP 3: API CALL — 流式调用模型
            // ──────────────────────────────────────────────────────

            // 产出请求开始事件
            yield QueryYield::RequestStart(RequestStartEvent);

            let tools = deps.get_tools();

            let call_params = ModelCallParams {
                messages: state.messages.clone(),
                system_prompt: system_prompt.clone(),
                tools: tools.clone(),
                model: None, // 使用默认模型
                max_output_tokens: state.max_output_tokens_override,
                skip_cache_write,
                thinking_enabled: deps.get_app_state().thinking_enabled,
                effort_value: deps.get_app_state().effort_value.clone(),
            };

            // 使用流式调用, 实时产出 StreamEvent
            let stream_result = deps.call_model_streaming(call_params).await;
            let mut event_stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    let error_str = e.to_string();

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

            // 逐事件消费流, 实时转发给上层, 同时累积构建完整消息
            let mut accumulator = crate::api::streaming::StreamAccumulator::new();
            let mut stream_error: Option<String> = None;

            use futures::StreamExt;
            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        accumulator.process_event(&event);
                        yield QueryYield::Stream(event);
                    }
                    Err(e) => {
                        stream_error = Some(e.to_string());
                        break;
                    }
                }
            }

            if let Some(err) = stream_error {
                warn!(error = %err, "stream error during model call");
                yield QueryYield::Message(Message::Assistant(
                    make_error_message(&deps, &err),
                ));
                break;
            }

            let assistant_message = accumulator.build();

            // 累计 usage
            if let Some(ref usage) = assistant_message.usage {
                cumulative_usage.input_tokens += usage.input_tokens;
                cumulative_usage.output_tokens += usage.output_tokens;
                cumulative_usage.cache_read_input_tokens += usage.cache_read_input_tokens;
                cumulative_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            }

            // ──────────────────────────────────────────────────────
            // STEP 4: POST-STREAMING — 检查 abort, 处理挂起摘要
            // ──────────────────────────────────────────────────────

            // 检查 abort
            if deps.is_aborted() {
                info!("aborted after streaming");
                yield QueryYield::Message(Message::Assistant(assistant_message));
                break;
            }

            // 处理挂起的工具使用摘要 (如果有)
            if let Some(ref _summary) = state.pending_tool_use_summary {
                // Phase 1: 摘要功能暂不完整实现
                // 在完整版本中, 这里会将摘要作为系统消息注入
                state.pending_tool_use_summary = None;
            }

            // 产出助手消息
            yield QueryYield::Message(Message::Assistant(assistant_message.clone()));

            // 将助手消息加入对话历史
            state.messages.push(Message::Assistant(assistant_message.clone()));

            // ──────────────────────────────────────────────────────
            // STEP 5 vs 6: 分支 — 有无工具调用
            // ──────────────────────────────────────────────────────

            let tool_uses = stop_hooks::extract_tool_uses(&assistant_message);

            if tool_uses.is_empty() {
                // ── TERMINAL CHECK (无工具调用) ──

                // 5a. max_output_tokens 恢复
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
                let stop_result = stop_hooks::run_stop_hooks(
                    &assistant_message,
                    &state.messages,
                    state.stop_hook_active,
                )
                .await;

                match stop_result {
                    Ok(StopHookResult::PreventStop { continuation_message }) => {
                        // 注入续写消息, 继续循环
                        let user_msg = make_user_message(
                            &deps,
                            &continuation_message,
                            true, // is_meta
                        );
                        state.messages.push(Message::User(user_msg));
                        state.stop_hook_active = Some(true);
                        state.transition = Some(Continue::StopHookBlocking);
                        state.turn_count += 1;
                        continue;
                    }
                    Ok(StopHookResult::BlockingError { error }) => {
                        warn!(error = %error, "stop hook blocking error");
                        // 终止
                        break;
                    }
                    Ok(StopHookResult::AllowStop) => {
                        // 继续到 token budget 检查
                    }
                    Err(e) => {
                        warn!(error = %e, "stop hook execution error");
                        // 出错时也允许停止
                    }
                }

                // 5c. token budget 检查
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
                        // 正常终止
                        break;
                    }
                }
            } else {
                // ── STEP 6: TOOL EXECUTION (有工具调用) ──

                let tool_results = execute_tool_calls(
                    &deps,
                    &tool_uses,
                    &tools,
                    &assistant_message,
                )
                .await;

                // 检查工具执行期间 abort
                if deps.is_aborted() {
                    info!("aborted during tool execution");
                    break;
                }

                // 将工具结果转为用户消息, 加入对话历史
                for exec_result in &tool_results {
                    let tool_result_content = if exec_result.is_error {
                        format!("Error: {}", exec_result.result.data)
                    } else {
                        exec_result.result.data.to_string()
                    };

                    let tool_result_block = ContentBlock::ToolResult {
                        tool_use_id: exec_result.tool_use_id.clone(),
                        content: ToolResultContent::Text(tool_result_content.clone()),
                        is_error: exec_result.is_error,
                    };

                    let user_msg = UserMessage {
                        uuid: Uuid::parse_str(&deps.uuid()).unwrap_or_else(|_| Uuid::new_v4()),
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        role: "user".to_string(),
                        content: MessageContent::Blocks(vec![tool_result_block]),
                        is_meta: true,
                        tool_use_result: Some(tool_result_content),
                        source_tool_assistant_uuid: Some(assistant_message.uuid),
                    };

                    let msg = Message::User(user_msg);
                    yield QueryYield::Message(msg.clone());
                    state.messages.push(msg);

                    // 产出子消息 (如 Agent 工具产生的嵌套对话)
                    for sub_msg in &exec_result.result.new_messages {
                        yield QueryYield::Message(sub_msg.clone());
                        state.messages.push(sub_msg.clone());
                    }
                }

                // ── STEP 7: ATTACHMENTS — 注入文件变更等 ──
                // Phase 1 简化: 暂不注入附件
                // 在完整版本中这里会:
                // - 检测文件系统变更 (git diff)
                // - 注入记忆消息
                // - 注入技能发现

                // ── STEP 8: CONTINUE — 刷新工具, 检查 maxTurns ──

                // 检查 maxTurns 限制
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

                // 尝试刷新工具 (MCP 等可能变化)
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

        debug!(turns = state.turn_count, "query loop finished");
    }
}

// ──────────────────────────────────────────────────────────
// 辅助类型和函数
// ──────────────────────────────────────────────────────────

/// prompt_too_long 恢复结果
#[allow(unused)]
enum PromptRecovery {
    Continue(Continue),
    Terminal(Terminal),
}

/// max_output_tokens 恢复结果
#[allow(unused)]
enum MaxTokensRecovery {
    Continue(Continue),
    Terminal,
}

/// 处理 prompt_too_long 错误的恢复逻辑
///
/// 三步恢复:
/// 1. collapse drain (折叠排空) — 移除最旧的非关键消息
/// 2. reactive compact (响应式压缩) — 紧急压缩
/// 3. 不可恢复 — 返回错误
#[allow(unused)]
async fn handle_prompt_too_long(
    deps: &Arc<dyn QueryDeps>,
    state: &mut QueryLoopState,
    error: &str,
) -> PromptRecovery {
    // Step 1: 如果尚未尝试响应式压缩, 先尝试
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

    // 不可恢复
    PromptRecovery::Terminal(Terminal::PromptTooLong)
}

/// 处理 max_output_tokens 恢复
///
/// 三步恢复:
/// 1. escalate — 将 max_output_tokens 从默认值提升到 ESCALATED_MAX_TOKENS
/// 2. recovery message — 注入 "continue from where you left off" 消息
/// 3. 达到 recovery limit — 终止
fn handle_max_output_tokens(
    deps: &Arc<dyn QueryDeps>,
    state: &mut QueryLoopState,
    _assistant_message: &AssistantMessage,
) -> MaxTokensRecovery {
    // Step 1: 如果尚未升级, 先升级
    if state.max_output_tokens_override.is_none() {
        debug!("max_output_tokens: escalating to {}", ESCALATED_MAX_TOKENS);
        state.max_output_tokens_override = Some(ESCALATED_MAX_TOKENS);
        state.transition = Some(Continue::MaxOutputTokensEscalate);
        return MaxTokensRecovery::Continue(Continue::MaxOutputTokensEscalate);
    }

    // Step 2: 尝试 recovery (注入续写消息)
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

    // Step 3: 达到 recovery limit
    debug!("max_output_tokens: recovery limit reached, terminating");
    MaxTokensRecovery::Terminal
}

/// 执行工具调用 (按批次: 并发安全的一起, 其余串行)
async fn execute_tool_calls(
    deps: &Arc<dyn QueryDeps>,
    tool_uses: &[(String, String, serde_json::Value)],
    tools: &crate::types::tool::Tools,
    parent_message: &AssistantMessage,
) -> Vec<super::deps::ToolExecResult> {
    use super::deps::{ToolExecRequest, ToolExecResult};

    let mut results = Vec::new();

    // 分区: 连续的并发安全工具 → 一个并发批次, 其余串行
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

    for (is_concurrent, batch) in batches {
        if is_concurrent && batch.len() > 1 {
            // 并发执行
            let mut handles = Vec::new();
            for (id, name, input) in batch {
                let deps = deps.clone();
                let parent = parent_message.clone();
                let tools = tools.clone();
                let handle = tokio::spawn(async move {
                    let req = ToolExecRequest {
                        tool_use_id: id,
                        tool_name: name,
                        input,
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
                        // 产生一个错误结果
                        results.push(ToolExecResult {
                            tool_use_id: "unknown".to_string(),
                            tool_name: "unknown".to_string(),
                            result: crate::types::tool::ToolResult {
                                data: serde_json::json!(format!("Internal error: {}", e)),
                                new_messages: vec![],
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
            // 串行执行
            for (id, name, input) in batch {
                let req = ToolExecRequest {
                    tool_use_id: id,
                    tool_name: name,
                    input,
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
                            },
                            is_error: true,
                        });
                    }
                }
            }
        }
    }

    results
}

/// 创建 abort 占位助手消息
fn make_abort_message(deps: &Arc<dyn QueryDeps>, reason: &str) -> AssistantMessage {
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

/// 创建 API 错误助手消息
fn make_error_message(deps: &Arc<dyn QueryDeps>, error: &str) -> AssistantMessage {
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

/// 创建用户消息 (系统注入)
fn make_user_message(
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
    use std::pin::Pin;

    use anyhow::Result;
    use futures::StreamExt;

    use crate::query::deps::{
        CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest,
        ToolExecResult,
    };
    use crate::types::app_state::AppState;
    use crate::types::config::QuerySource;
    use crate::types::message::StreamEvent;
    use crate::types::state::AutoCompactTracking;
    use crate::types::tool::{ToolProgress, Tools};

    /// 测试用的 mock deps
    struct MockDeps {
        /// 预设的模型响应序列
        responses: std::sync::Mutex<Vec<ModelResponse>>,
        aborted: std::sync::atomic::AtomicBool,
    }

    impl MockDeps {
        fn new(responses: Vec<ModelResponse>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
                aborted: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl QueryDeps for MockDeps {
        async fn call_model(&self, _params: ModelCallParams) -> Result<ModelResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no more mock responses");
            }
            Ok(responses.remove(0))
        }

        async fn call_model_streaming(
            &self,
            _params: ModelCallParams,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no more mock responses");
            }
            let resp = responses.remove(0);
            // Convert ModelResponse into a stream of events that
            // StreamAccumulator can reconstruct.
            let mut events = Vec::new();
            events.push(StreamEvent::MessageStart {
                usage: resp.usage.clone(),
            });
            for (i, block) in resp.assistant_message.content.iter().enumerate() {
                events.push(StreamEvent::ContentBlockStart {
                    index: i,
                    content_block: block.clone(),
                });
                events.push(StreamEvent::ContentBlockStop { index: i });
            }
            events.push(StreamEvent::MessageDelta {
                delta: crate::types::message::MessageDelta {
                    stop_reason: resp.assistant_message.stop_reason.clone(),
                },
                usage: Some(resp.usage),
            });
            events.push(StreamEvent::MessageStop);
            let stream = futures::stream::iter(events.into_iter().map(Ok));
            Ok(Box::pin(stream))
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
            Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: crate::types::tool::ToolResult {
                    data: serde_json::json!("mock tool output"),
                    new_messages: vec![],
                },
                is_error: false,
            })
        }

        fn get_app_state(&self) -> AppState {
            AppState::default()
        }

        fn uuid(&self) -> String {
            Uuid::new_v4().to_string()
        }

        fn is_aborted(&self) -> bool {
            self.aborted.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn get_tools(&self) -> Tools {
            vec![]
        }

        async fn refresh_tools(&self) -> Result<Tools> {
            Ok(vec![])
        }
    }

    fn make_text_response(text: &str) -> ModelResponse {
        ModelResponse {
            assistant_message: AssistantMessage {
                uuid: Uuid::new_v4(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                role: "assistant".to_string(),
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                usage: Some(Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                }),
                stop_reason: Some("end_turn".to_string()),
                is_api_error_message: false,
                api_error: None,
                cost_usd: 0.001,
            },
            stream_events: vec![],
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        }
    }

    #[tokio::test]
    async fn test_simple_text_response_terminates() {
        let deps = Arc::new(MockDeps::new(vec![make_text_response("Hello, world!")]));

        let params = QueryParams {
            messages: vec![Message::User(UserMessage {
                uuid: Uuid::new_v4(),
                timestamp: 0,
                role: "user".to_string(),
                content: MessageContent::Text("Hi".to_string()),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            })],
            system_prompt: vec!["You are a helpful assistant.".to_string()],
            user_context: Default::default(),
            system_context: Default::default(),
            fallback_model: None,
            query_source: QuerySource::ReplMainThread,
            max_output_tokens_override: None,
            max_turns: None,
            skip_cache_write: None,
            task_budget: None,
        };

        let stream = query(params, deps);
        let items: Vec<QueryYield> = stream.collect().await;

        // Should have: RequestStart, (stream events...), AssistantMessage
        assert!(items.len() >= 2, "expected at least 2 items, got {}", items.len());

        // First should be RequestStart
        assert!(matches!(items[0], QueryYield::RequestStart(_)));

        // Should contain an assistant message
        let has_assistant = items.iter().any(|item| {
            matches!(item, QueryYield::Message(Message::Assistant(_)))
        });
        assert!(has_assistant, "expected an assistant message in output");
    }

    #[tokio::test]
    async fn test_tool_use_then_text_response() {
        let tool_response = ModelResponse {
            assistant_message: AssistantMessage {
                uuid: Uuid::new_v4(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                role: "assistant".to_string(),
                content: vec![
                    ContentBlock::Text {
                        text: "Let me check.".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "tu_1".to_string(),
                        name: "Bash".to_string(),
                        input: serde_json::json!({"command": "echo hello"}),
                    },
                ],
                usage: Some(Usage {
                    input_tokens: 100,
                    output_tokens: 80,
                    ..Default::default()
                }),
                stop_reason: Some("tool_use".to_string()),
                is_api_error_message: false,
                api_error: None,
                cost_usd: 0.001,
            },
            stream_events: vec![],
            usage: Usage::default(),
        };

        let text_response = make_text_response("Done! The output was hello.");

        let deps = Arc::new(MockDeps::new(vec![tool_response, text_response]));

        let params = QueryParams {
            messages: vec![Message::User(UserMessage {
                uuid: Uuid::new_v4(),
                timestamp: 0,
                role: "user".to_string(),
                content: MessageContent::Text("Run echo hello".to_string()),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            })],
            system_prompt: vec![],
            user_context: Default::default(),
            system_context: Default::default(),
            fallback_model: None,
            query_source: QuerySource::ReplMainThread,
            max_output_tokens_override: None,
            max_turns: None,
            skip_cache_write: None,
            task_budget: None,
        };

        let stream = query(params, deps);
        let items: Vec<QueryYield> = stream.collect().await;

        // Should have 2 request starts (two turns), assistant messages, tool result messages
        let request_starts = items
            .iter()
            .filter(|i| matches!(i, QueryYield::RequestStart(_)))
            .count();
        assert_eq!(request_starts, 2, "expected 2 request starts (two turns)");

        let assistant_msgs = items
            .iter()
            .filter(|i| matches!(i, QueryYield::Message(Message::Assistant(_))))
            .count();
        assert_eq!(assistant_msgs, 2, "expected 2 assistant messages");
    }

    #[tokio::test]
    async fn test_max_turns_limit() {
        // Both responses have tool calls, but max_turns=1 should stop after first
        let tool_response = ModelResponse {
            assistant_message: AssistantMessage {
                uuid: Uuid::new_v4(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                role: "assistant".to_string(),
                content: vec![ContentBlock::ToolUse {
                    id: "tu_1".to_string(),
                    name: "Bash".to_string(),
                    input: serde_json::json!({"command": "ls"}),
                }],
                usage: Some(Usage::default()),
                stop_reason: Some("tool_use".to_string()),
                is_api_error_message: false,
                api_error: None,
                cost_usd: 0.0,
            },
            stream_events: vec![],
            usage: Usage::default(),
        };

        let deps = Arc::new(MockDeps::new(vec![tool_response]));

        let params = QueryParams {
            messages: vec![Message::User(UserMessage {
                uuid: Uuid::new_v4(),
                timestamp: 0,
                role: "user".to_string(),
                content: MessageContent::Text("list files".to_string()),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            })],
            system_prompt: vec![],
            user_context: Default::default(),
            system_context: Default::default(),
            fallback_model: None,
            query_source: QuerySource::ReplMainThread,
            max_output_tokens_override: None,
            max_turns: Some(1),
            skip_cache_write: None,
            task_budget: None,
        };

        let stream = query(params, deps);
        let items: Vec<QueryYield> = stream.collect().await;

        // Should have a MaxTurnsReached attachment
        let has_max_turns = items.iter().any(|item| {
            matches!(
                item,
                QueryYield::Message(Message::Attachment(AttachmentMessage {
                    attachment: Attachment::MaxTurnsReached { .. },
                    ..
                }))
            )
        });
        assert!(has_max_turns, "expected MaxTurnsReached attachment");
    }

    #[tokio::test]
    async fn test_abort_before_api_call() {
        let deps = Arc::new(MockDeps::new(vec![]));
        deps.aborted
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let params = QueryParams {
            messages: vec![Message::User(UserMessage {
                uuid: Uuid::new_v4(),
                timestamp: 0,
                role: "user".to_string(),
                content: MessageContent::Text("Hi".to_string()),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            })],
            system_prompt: vec![],
            user_context: Default::default(),
            system_context: Default::default(),
            fallback_model: None,
            query_source: QuerySource::ReplMainThread,
            max_output_tokens_override: None,
            max_turns: None,
            skip_cache_write: None,
            task_budget: None,
        };

        let stream = query(params, deps);
        let items: Vec<QueryYield> = stream.collect().await;

        // Should have an aborted assistant message
        let has_assistant = items.iter().any(|item| {
            if let QueryYield::Message(Message::Assistant(msg)) = item {
                msg.stop_reason.as_deref() == Some("AbortedStreaming")
            } else {
                false
            }
        });
        assert!(has_assistant, "expected aborted assistant message");
    }
}
