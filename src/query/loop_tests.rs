use super::*;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest,
    ToolExecResult,
};
use crate::types::app_state::AppState;
use crate::types::config::QuerySource;
use crate::types::message::{AssistantMessage, ContentBlock, StreamEvent, Usage};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

/// Mock deps for testing.
struct MockDeps {
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

fn make_text_response(text: &str) -> ModelResponse {
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

    assert!(items.len() >= 2, "expected at least 2 items, got {}", items.len());
    assert!(matches!(items[0], QueryYield::RequestStart(_)));

    let has_assistant = items.iter().any(|item| {
        matches!(item, QueryYield::Message(Message::Assistant(_)))
    });
    assert!(has_assistant, "expected an assistant message in output");
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

    let stream = query(params, deps);
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
