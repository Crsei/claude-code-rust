use super::*;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::types::app_state::AppState;
use crate::types::config::QuerySource;
use crate::types::message::{
    AssistantMessage, ContentBlock, ImageSource, StreamEvent, ToolResultContent, Usage,
};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

/// Mock deps for testing.
struct MockDeps {
    responses: parking_lot::Mutex<Vec<ModelResponse>>,
    aborted: std::sync::atomic::AtomicBool,
}

impl MockDeps {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: parking_lot::Mutex::new(responses),
            aborted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl QueryDeps for MockDeps {
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
                                other => panic!(
                                    "second block should be Image, got {:?}",
                                    other
                                ),
                            }
                        }
                        ToolResultContent::Text(t) => {
                            panic!(
                                "expected Blocks in tool result, got Text: {}",
                                t
                            );
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
                model_content: Some(ToolResultContent::Blocks(vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/png".to_string(),
                            data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
                        },
                    },
                ])),
                display_preview: Some("[Screenshot: 1920x1080]".to_string()),
                new_messages: vec![],
            }
        } else {
            // click, type_text, key, scroll → text confirmation
            crate::types::tool::ToolResult {
                data: serde_json::json!(format!("Action '{}' executed successfully", request.tool_name)),
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
        final_msg.content.iter().any(|b| matches!(b, ContentBlock::Text { text } if text.contains("clicked"))),
        "final message should mention clicking"
    );
}
