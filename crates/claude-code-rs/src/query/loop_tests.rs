use super::*;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use cc_types::hooks::{
    HookEventConfig, HookOutput, HookRunner, HooksMap, PostToolHookResult, PreToolHookResult,
};
use futures::StreamExt;
use serde_json::Value;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::types::app_state::AppState;
use crate::types::config::{QuerySource, TaskBudget};
use crate::types::message::{
    AssistantMessage, ContentBlock, ImageSource, MessageContent, StreamEvent, ToolResultContent,
    Usage, UserMessage,
};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

enum MockStreamStep {
    Response(ModelResponse),
    Error(String),
}

/// Mock deps for testing.
struct MockDeps {
    stream_steps: parking_lot::Mutex<Vec<MockStreamStep>>,
    call_params: parking_lot::Mutex<Vec<ModelCallParams>>,
    reactive_compact_result: parking_lot::Mutex<Option<CompactionResult>>,
    reactive_compact_calls: AtomicUsize,
    aborted: AtomicBool,
    stream_finished: Arc<AtomicBool>,
    tool_executed_before_stream_finished: AtomicBool,
    hook_runner: parking_lot::Mutex<Arc<dyn HookRunner>>,
}

impl MockDeps {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self::from_steps(
            responses
                .into_iter()
                .map(MockStreamStep::Response)
                .collect(),
        )
    }

    fn from_steps(stream_steps: Vec<MockStreamStep>) -> Self {
        Self {
            stream_steps: parking_lot::Mutex::new(stream_steps),
            call_params: parking_lot::Mutex::new(Vec::new()),
            reactive_compact_result: parking_lot::Mutex::new(None),
            reactive_compact_calls: AtomicUsize::new(0),
            aborted: AtomicBool::new(false),
            stream_finished: Arc::new(AtomicBool::new(false)),
            tool_executed_before_stream_finished: AtomicBool::new(false),
            hook_runner: parking_lot::Mutex::new(Arc::new(cc_types::hooks::NoopHookRunner)),
        }
    }

    fn recorded_params(&self) -> Vec<ModelCallParams> {
        self.call_params.lock().clone()
    }

    fn set_reactive_compact_result(&self, result: Option<CompactionResult>) {
        *self.reactive_compact_result.lock() = result;
    }

    fn set_hook_runner(&self, runner: Arc<dyn HookRunner>) {
        *self.hook_runner.lock() = runner;
    }

    fn pop_stream_step(&self) -> Result<MockStreamStep> {
        let mut steps = self.stream_steps.lock();
        if steps.is_empty() {
            anyhow::bail!("no more mock responses");
        }
        Ok(steps.remove(0))
    }
}

#[async_trait::async_trait]
impl QueryDeps for MockDeps {
    async fn call_model(&self, params: ModelCallParams) -> Result<ModelResponse> {
        self.call_params.lock().push(params);
        match self.pop_stream_step()? {
            MockStreamStep::Response(resp) => Ok(resp),
            MockStreamStep::Error(error) => anyhow::bail!("{}", error),
        }
    }

    async fn call_model_streaming(
        &self,
        params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        self.call_params.lock().push(params);
        self.stream_finished.store(false, Ordering::SeqCst);

        let resp = match self.pop_stream_step()? {
            MockStreamStep::Response(resp) => resp,
            MockStreamStep::Error(error) => anyhow::bail!("{}", error),
        };

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
        let stream_finished = self.stream_finished.clone();
        let stream = futures::stream::iter(events.into_iter().map(Ok)).inspect(move |event| {
            if matches!(event, Ok(StreamEvent::MessageStop)) {
                stream_finished.store(true, Ordering::SeqCst);
            }
        });
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

    async fn reactive_compact(&self, _messages: Vec<Message>) -> Result<Option<CompactionResult>> {
        self.reactive_compact_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.reactive_compact_result.lock().take())
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        _tools: &Tools,
        _parent: &AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        if !self.stream_finished.load(Ordering::SeqCst) {
            self.tool_executed_before_stream_finished
                .store(true, Ordering::SeqCst);
        }

        Ok(ToolExecResult {
            tool_use_id: request.tool_use_id,
            tool_name: request.tool_name,
            result: crate::types::tool::ToolResult {
                data: serde_json::json!("mock tool output"),
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

    fn hook_runner(&self) -> Arc<dyn HookRunner> {
        self.hook_runner.lock().clone()
    }
}

fn make_user_message_for_test(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: 0,
        role: "user".to_string(),
        content: MessageContent::Text(text.to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

fn make_query_params(messages: Vec<Message>) -> QueryParams {
    QueryParams {
        messages,
        system_prompt: vec![],
        user_context: Default::default(),
        system_context: Default::default(),
        fallback_model: None,
        query_source: QuerySource::ReplMainThread,
        max_output_tokens_override: None,
        max_turns: None,
        skip_cache_write: None,
        task_budget: None,
    }
}

fn make_auto_compact_tracking() -> AutoCompactTracking {
    AutoCompactTracking {
        compacted: true,
        turn_counter: 1,
        turn_id: "test-turn".to_string(),
        consecutive_failures: 0,
    }
}

fn make_text_response(text: &str) -> ModelResponse {
    make_text_response_with_stop_and_output_tokens(text, "end_turn", 50)
}

fn make_text_response_with_stop_and_output_tokens(
    text: &str,
    stop_reason: &str,
    output_tokens: u64,
) -> ModelResponse {
    ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            usage: Some(Usage {
                input_tokens: 100,
                output_tokens,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
            stop_reason: Some(stop_reason.to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.001,
        },
        stream_events: vec![],
        usage: Usage {
            input_tokens: 100,
            output_tokens,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
    }
}

fn request_start_count(items: &[QueryYield]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, QueryYield::RequestStart(_)))
        .count()
}

struct StopContinuationHookRunner {
    message: String,
    calls: AtomicUsize,
}

impl StopContinuationHookRunner {
    fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl HookRunner for StopContinuationHookRunner {
    fn load_hook_configs(&self, _hooks_value: &HooksMap, event_name: &str) -> Vec<HookEventConfig> {
        if event_name == "Stop" {
            vec![HookEventConfig {
                matcher: None,
                hooks: Vec::new(),
            }]
        } else {
            Vec::new()
        }
    }

    async fn run_pre_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PreToolHookResult> {
        Ok(PreToolHookResult::Continue {
            updated_input: None,
            permission_override: None,
        })
    }

    async fn run_post_tool_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _tool_result_data: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        Ok(PostToolHookResult::Continue)
    }

    async fn run_post_tool_failure_hooks(
        &self,
        _tool_name: &str,
        _input: &Value,
        _error: &str,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run_event_hooks(
        &self,
        _event_name: &str,
        _payload: &Value,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<HookOutput> {
        Ok(HookOutput::default())
    }

    async fn run_stop_hooks(
        &self,
        _hook_configs: &[HookEventConfig],
    ) -> anyhow::Result<PostToolHookResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(PostToolHookResult::StopContinuation {
            message: self.message.clone(),
        })
    }
}

#[tokio::test]
async fn test_simple_text_response_terminates() {
    let deps = Arc::new(MockDeps::new(vec![make_text_response("Hello, world!")]));

    let params = QueryParams {
        messages: vec![Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
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

    assert!(
        items.len() >= 2,
        "expected at least 2 items, got {}",
        items.len()
    );
    assert!(matches!(items[0], QueryYield::RequestStart(_)));

    let has_assistant = items
        .iter()
        .any(|item| matches!(item, QueryYield::Message(Message::Assistant(_))));
    assert!(has_assistant, "expected an assistant message in output");
}

#[tokio::test]
async fn test_prompt_too_long_reactive_compact_retries_model_call() {
    let initial_messages = vec![make_user_message_for_test("Summarize this long context")];
    let deps = Arc::new(MockDeps::from_steps(vec![
        MockStreamStep::Error("prompt_too_long: context window exceeded".to_string()),
        MockStreamStep::Response(make_text_response("Recovered after compact.")),
    ]));
    deps.set_reactive_compact_result(Some(CompactionResult {
        messages: initial_messages.clone(),
        tracking: make_auto_compact_tracking(),
    }));

    let stream = query(make_query_params(initial_messages), deps.clone());
    let items: Vec<QueryYield> = stream.collect().await;

    assert_eq!(
        request_start_count(&items),
        2,
        "prompt_too_long recovery should retry the model call"
    );
    assert_eq!(
        deps.reactive_compact_calls.load(Ordering::SeqCst),
        1,
        "prompt_too_long should attempt reactive compact once"
    );

    let recovered = items.iter().any(|item| {
        if let QueryYield::Message(Message::Assistant(msg)) = item {
            msg.content.iter().any(|block| {
                matches!(block, ContentBlock::Text { text } if text == "Recovered after compact.")
            })
        } else {
            false
        }
    });
    assert!(recovered, "expected recovered assistant response");
}

#[tokio::test]
async fn test_fallback_model_retries_stream_start_capacity_error() {
    let deps = Arc::new(MockDeps::from_steps(vec![
        MockStreamStep::Error("529 overloaded: high demand".to_string()),
        MockStreamStep::Response(make_text_response("Recovered on fallback")),
    ]));

    let mut params = make_query_params(vec![make_user_message_for_test("Use the fallback")]);
    params.fallback_model = Some("claude-fallback".to_string());

    let stream = query(params, deps.clone());
    let items: Vec<QueryYield> = stream.collect().await;

    assert_eq!(
        request_start_count(&items),
        2,
        "fallback should emit a second model request start"
    );

    let recorded = deps.recorded_params();
    let primary_model = AppState::default().main_loop_model;
    assert_eq!(
        recorded.len(),
        2,
        "primary failure should be retried once with fallback"
    );
    assert_eq!(recorded[0].model.as_deref(), Some(primary_model.as_str()));
    assert_eq!(recorded[1].model.as_deref(), Some("claude-fallback"));
    assert!(
        items.iter().any(|item| matches!(
            item,
            QueryYield::Message(Message::Assistant(msg))
                if msg.content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Text { text } if text.contains("Recovered on fallback")
                ))
        )),
        "fallback response should be yielded as the assistant message"
    );
}

#[tokio::test]
async fn test_max_tokens_recovery_escalates_next_request_limit() {
    let deps = Arc::new(MockDeps::new(vec![
        make_text_response_with_stop_and_output_tokens("Partial answer", "max_tokens", 50),
        make_text_response("Continuation after larger output limit."),
    ]));

    let stream = query(
        make_query_params(vec![make_user_message_for_test("Write a long answer")]),
        deps.clone(),
    );
    let items: Vec<QueryYield> = stream.collect().await;

    assert_eq!(
        request_start_count(&items),
        2,
        "max_tokens should trigger one retry"
    );
    let params = deps.recorded_params();
    assert_eq!(params.len(), 2, "expected two model calls");
    assert_eq!(params[0].max_output_tokens, None);
    assert_eq!(
        params[1].max_output_tokens,
        Some(crate::query::loop_helpers::ESCALATED_MAX_TOKENS)
    );
}

#[tokio::test]
async fn test_stop_hook_continuation_injects_meta_user_message_once() {
    let deps = Arc::new(MockDeps::new(vec![
        make_text_response("Need final audit."),
        make_text_response("Final answer after stop hook."),
    ]));
    deps.set_hook_runner(Arc::new(StopContinuationHookRunner::new(
        "Run one more validation pass.",
    )));

    let stream = query(
        make_query_params(vec![make_user_message_for_test("Finish the task")]),
        deps.clone(),
    );
    let items: Vec<QueryYield> = stream.collect().await;

    assert_eq!(
        request_start_count(&items),
        2,
        "stop hook continuation should trigger one more model call"
    );
    let params = deps.recorded_params();
    assert_eq!(params.len(), 2, "expected continuation model call");

    let continuation = params[1].messages.iter().rev().find_map(|message| {
        if let Message::User(user) = message {
            if let MessageContent::Text(text) = &user.content {
                return Some((user.is_meta, text.as_str()));
            }
        }
        None
    });
    assert_eq!(
        continuation,
        Some((true, "Run one more validation pass.")),
        "stop hook continuation should be injected as a meta user message"
    );
}

#[tokio::test]
async fn test_token_budget_continuation_injects_nudge_message() {
    let deps = Arc::new(MockDeps::new(vec![
        make_text_response_with_stop_and_output_tokens("Still working.", "end_turn", 50),
        make_text_response_with_stop_and_output_tokens("Budget complete.", "end_turn", 50),
    ]));
    let mut params = make_query_params(vec![make_user_message_for_test("Spend the budget")]);
    params.task_budget = Some(TaskBudget { total: 100 });

    let stream = query(params, deps.clone());
    let items: Vec<QueryYield> = stream.collect().await;

    assert_eq!(
        request_start_count(&items),
        2,
        "token budget nudge should trigger one continuation"
    );
    let params = deps.recorded_params();
    assert_eq!(params.len(), 2, "expected continuation model call");

    let nudge = params[1].messages.iter().rev().find_map(|message| {
        if let Message::User(user) = message {
            if let MessageContent::Text(text) = &user.content {
                return Some((user.is_meta, text.as_str()));
            }
        }
        None
    });
    assert!(
        matches!(nudge, Some((true, text)) if text.contains("Token budget at 50%")),
        "token budget continuation should inject a meta nudge message"
    );
}

#[tokio::test]
async fn test_tool_use_then_text_response() {
    let tool_response = ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
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
            uuid: uuid::Uuid::new_v4(),
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

    let stream = query(params, deps.clone());
    let items: Vec<QueryYield> = stream.collect().await;

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

    assert!(
        !deps
            .tool_executed_before_stream_finished
            .load(Ordering::SeqCst),
        "tool execution should not start before the model stream reaches message_stop"
    );
}

#[tokio::test]
async fn test_max_turns_limit() {
    let tool_response = ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
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
            uuid: uuid::Uuid::new_v4(),
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
            uuid: uuid::Uuid::new_v4(),
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

    let has_assistant = items.iter().any(|item| {
        if let QueryYield::Message(Message::Assistant(msg)) = item {
            msg.stop_reason.as_deref() == Some("AbortedStreaming")
        } else {
            false
        }
    });
    assert!(has_assistant, "expected aborted assistant message");
}

/// MockDeps that returns image content in ToolResult.model_content
struct ImageMockDeps {
    responses: parking_lot::Mutex<Vec<ModelResponse>>,
    aborted: std::sync::atomic::AtomicBool,
}

impl ImageMockDeps {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: parking_lot::Mutex::new(responses),
            aborted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl QueryDeps for ImageMockDeps {
    async fn call_model(&self, _params: ModelCallParams) -> Result<ModelResponse> {
        let mut responses = self.responses.lock();
        if responses.is_empty() {
            anyhow::bail!("no more mock responses");
        }
        Ok(responses.remove(0))
    }

    async fn call_model_streaming(
        &self,
        _params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut responses = self.responses.lock();
        if responses.is_empty() {
            anyhow::bail!("no more mock responses");
        }
        let resp = responses.remove(0);
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

    async fn reactive_compact(&self, _messages: Vec<Message>) -> Result<Option<CompactionResult>> {
        Ok(None)
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        _tools: &Tools,
        _parent: &AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        // Return a tool result with image model_content (simulating MCP screenshot)
        let image_blocks = vec![
            ContentBlock::Text {
                text: "Screenshot captured".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".to_string(),
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
                },
            },
        ];

        Ok(ToolExecResult {
            tool_use_id: request.tool_use_id,
            tool_name: request.tool_name,
            result: crate::types::tool::ToolResult {
                data: serde_json::json!("[Image: image/png]"),
                model_content: Some(ToolResultContent::Blocks(image_blocks)),
                display_preview: Some("[Image: image/png]".to_string()),
                new_messages: vec![],
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
        self.aborted.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        vec![]
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn test_image_tool_result_flows_as_blocks() {
    // Turn 1: model calls a tool (screenshot)
    let tool_response = ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "Let me take a screenshot.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tu_screenshot".to_string(),
                    name: "mcp__computer-use__screenshot".to_string(),
                    input: serde_json::json!({}),
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

    // Turn 2: model sees image and responds
    let text_response = make_text_response("I can see the desktop.");

    let deps = Arc::new(ImageMockDeps::new(vec![tool_response, text_response]));

    let params = QueryParams {
        messages: vec![Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text("Take a screenshot".to_string()),
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

    // Find the tool result user message
    let tool_result_msg = items.iter().find_map(|item| {
        if let QueryYield::Message(Message::User(user_msg)) = item {
            if user_msg.is_meta && user_msg.source_tool_assistant_uuid.is_some() {
                return Some(user_msg);
            }
        }
        None
    });

    let user_msg = tool_result_msg.expect("expected a tool result user message");

    // Verify the tool result contains structured Blocks (not plain Text)
    match &user_msg.content {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 1, "expected 1 tool_result block");
            match &blocks[0] {
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    assert!(!is_error);
                    match content {
                        ToolResultContent::Blocks(inner_blocks) => {
                            assert_eq!(
                                inner_blocks.len(),
                                2,
                                "expected 2 inner blocks (text + image)"
                            );
                            assert!(
                                matches!(&inner_blocks[0], ContentBlock::Text { .. }),
                                "first block should be text"
                            );
                            match &inner_blocks[1] {
                                ContentBlock::Image { source } => {
                                    assert_eq!(source.source_type, "base64");
                                    assert_eq!(source.media_type, "image/png");
                                    assert_eq!(source.data, "iVBORw0KGgoAAAANSUhEUg==");
                                }
                                other => panic!("second block should be Image, got {:?}", other),
                            }
                        }
                        ToolResultContent::Text(t) => {
                            panic!("expected Blocks in tool result, got Text: {}", t);
                        }
                    }
                }
                other => panic!("expected ToolResult block, got {:?}", other),
            }
        }
        _ => panic!("expected Blocks content"),
    }

    // Verify display_preview is used for tool_use_result (not raw base64)
    assert_eq!(
        user_msg.tool_use_result.as_deref(),
        Some("[Image: image/png]"),
        "tool_use_result should contain display preview, not base64 data"
    );

    // Verify there are 2 request starts (two turns = tool call + final response)
    let request_starts = items
        .iter()
        .filter(|i| matches!(i, QueryYield::RequestStart(_)))
        .count();
    assert_eq!(request_starts, 2, "expected 2 API turns");
}

// ---------------------------------------------------------------------------
// Computer Use end-to-end smoke test
// ---------------------------------------------------------------------------

/// MockDeps that dispatches tool results by tool name:
/// - screenshot → image content
/// - left_click / type_text → text confirmation
struct CuMockDeps {
    responses: parking_lot::Mutex<Vec<ModelResponse>>,
    aborted: std::sync::atomic::AtomicBool,
}

impl CuMockDeps {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: parking_lot::Mutex::new(responses),
            aborted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl QueryDeps for CuMockDeps {
    async fn call_model(&self, _params: ModelCallParams) -> Result<ModelResponse> {
        let mut responses = self.responses.lock();
        if responses.is_empty() {
            anyhow::bail!("no more mock responses");
        }
        Ok(responses.remove(0))
    }

    async fn call_model_streaming(
        &self,
        _params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut responses = self.responses.lock();
        if responses.is_empty() {
            anyhow::bail!("no more mock responses");
        }
        let resp = responses.remove(0);
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

    async fn reactive_compact(&self, _messages: Vec<Message>) -> Result<Option<CompactionResult>> {
        Ok(None)
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        _tools: &Tools,
        _parent: &AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        // Dispatch by tool name to simulate different CU tools
        let result = if request.tool_name.contains("screenshot") {
            crate::types::tool::ToolResult {
                data: serde_json::json!("[Image: image/png]"),
                model_content: Some(ToolResultContent::Blocks(vec![ContentBlock::Image {
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: "image/png".to_string(),
                        data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
                    },
                }])),
                display_preview: Some("[Screenshot: 1920x1080]".to_string()),
                new_messages: vec![],
            }
        } else {
            // click, type_text, key, scroll → text confirmation
            crate::types::tool::ToolResult {
                data: serde_json::json!(format!(
                    "Action '{}' executed successfully",
                    request.tool_name
                )),
                new_messages: vec![],
                ..Default::default()
            }
        };

        Ok(ToolExecResult {
            tool_use_id: request.tool_use_id,
            tool_name: request.tool_name,
            result,
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
        self.aborted.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        vec![]
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        Ok(vec![])
    }
}

/// Full Computer Use smoke test:
///   Turn 1: model calls screenshot → receives image
///   Turn 2: model calls left_click → receives text confirmation
///   Turn 3: model responds with final text
#[tokio::test]
async fn test_computer_use_screenshot_click_round_trip() {
    // Turn 1: model takes a screenshot
    let screenshot_response = ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "Let me take a screenshot to see the desktop.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tu_screenshot".to_string(),
                    name: "mcp__computer-use__screenshot".to_string(),
                    input: serde_json::json!({}),
                },
            ],
            usage: Some(Usage::default()),
            stop_reason: Some("tool_use".to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.001,
        },
        stream_events: vec![],
        usage: Usage::default(),
    };

    // Turn 2: model sees image, decides to click
    let click_response = ModelResponse {
        assistant_message: AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "I can see a button at (500, 300). Clicking it.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tu_click".to_string(),
                    name: "mcp__computer-use__left_click".to_string(),
                    input: serde_json::json!({"x": 500, "y": 300}),
                },
            ],
            usage: Some(Usage::default()),
            stop_reason: Some("tool_use".to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.001,
        },
        stream_events: vec![],
        usage: Usage::default(),
    };

    // Turn 3: model confirms result
    let final_response = make_text_response("I clicked the button successfully.");

    let deps = Arc::new(CuMockDeps::new(vec![
        screenshot_response,
        click_response,
        final_response,
    ]));

    let params = QueryParams {
        messages: vec![Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text("Click the button on screen".to_string()),
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

    // Verify 3 API turns (screenshot, click, final)
    let request_starts = items
        .iter()
        .filter(|i| matches!(i, QueryYield::RequestStart(_)))
        .count();
    assert_eq!(request_starts, 3, "expected 3 API turns");

    // Verify 3 assistant messages
    let assistant_msgs: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let QueryYield::Message(Message::Assistant(msg)) = item {
                Some(msg)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(assistant_msgs.len(), 3, "expected 3 assistant messages");

    // Verify tool result messages
    let tool_result_msgs: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let QueryYield::Message(Message::User(msg)) = item {
                if msg.is_meta && msg.source_tool_assistant_uuid.is_some() {
                    return Some(msg);
                }
            }
            None
        })
        .collect();
    assert_eq!(tool_result_msgs.len(), 2, "expected 2 tool result messages");

    // First tool result (screenshot) should have Blocks content with Image
    match &tool_result_msgs[0].content {
        MessageContent::Blocks(blocks) => match &blocks[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert!(
                    matches!(content, ToolResultContent::Blocks(_)),
                    "screenshot result should be Blocks (image), got Text"
                );
            }
            other => panic!("expected ToolResult, got {:?}", other),
        },
        _ => panic!("expected Blocks content"),
    }

    // Second tool result (click) should have Text content
    match &tool_result_msgs[1].content {
        MessageContent::Blocks(blocks) => match &blocks[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert!(
                    matches!(content, ToolResultContent::Text(_)),
                    "click result should be Text"
                );
            }
            other => panic!("expected ToolResult, got {:?}", other),
        },
        _ => panic!("expected Blocks content"),
    }

    // Final message should be text
    let final_msg = assistant_msgs.last().unwrap();
    assert!(
        final_msg
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::Text { text } if text.contains("clicked"))),
        "final message should mention clicking"
    );
}
