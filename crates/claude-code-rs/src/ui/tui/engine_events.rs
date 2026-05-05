use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::config::QuerySource;
use crate::types::message::{
    AssistantMessage, ContentBlock, InfoLevel, Message, MessageContent, StreamEvent, SystemMessage,
    SystemSubtype, UserMessage,
};
use crate::ui::app::App;
use crate::ui::permissions::PermissionChoice;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;
/// Tracks the partial assistant message being streamed.
pub(super) struct StreamingState {
    /// Accumulated content blocks from streaming events.
    blocks: Vec<ContentBlock>,
    /// Whether we are inside a content block.
    active: bool,
}

impl StreamingState {
    pub(super) fn new() -> Self {
        Self {
            blocks: Vec::new(),
            active: false,
        }
    }

    fn clear(&mut self) {
        self.blocks.clear();
        self.active = false;
    }

    fn is_partial(&self) -> bool {
        self.active || !self.blocks.is_empty()
    }

    fn ensure_block(&mut self, index: usize, fallback: ContentBlock) -> &mut ContentBlock {
        while self.blocks.len() <= index {
            self.blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }
        if matches!(self.blocks[index], ContentBlock::Text { ref text } if text.is_empty()) {
            self.blocks[index] = fallback;
        }
        &mut self.blocks[index]
    }
}

// ---------------------------------------------------------------------------
// Engine event channel type
// ---------------------------------------------------------------------------

/// Events sent from engine tasks to the TUI main loop.
pub(super) enum EngineEvent {
    /// An SDK message from the engine stream.
    Sdk(Box<SdkMessage>),
    /// A tool permission prompt that must be answered by the UI.
    PermissionRequest {
        tool_name: String,
        description: String,
        response_tx: oneshot::Sender<String>,
    },
    /// The engine query task has completed (stream exhausted).
    Done,
}

// ---------------------------------------------------------------------------
// Permission bridge
// ---------------------------------------------------------------------------

pub(super) fn install_tui_permission_callback(
    engine: &Arc<QueryEngine>,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    let callback: crate::types::tool::PermissionCallback = Arc::new(
        move |_tool_use_id: String,
              tool_name: String,
              description: String,
              _options: Vec<String>| {
            let tx = tx.clone();
            Box::pin(async move {
                let (response_tx, response_rx) = oneshot::channel();
                let event = EngineEvent::PermissionRequest {
                    tool_name,
                    description,
                    response_tx,
                };

                if tx.send(event).is_err() {
                    return "deny".to_string();
                }

                response_rx.await.unwrap_or_else(|_| "deny".to_string())
            })
        },
    );
    engine.set_permission_callback(callback);
}

pub(super) fn permission_choice_to_decision(choice: PermissionChoice) -> &'static str {
    match choice {
        PermissionChoice::Allow => "allow",
        PermissionChoice::Deny => "deny",
        PermissionChoice::AlwaysAllow => "always_allow",
    }
}

// ---------------------------------------------------------------------------
// Engine query task
// ---------------------------------------------------------------------------

/// Spawn a tokio task that drives a QueryEngine query and sends results
/// through the `tx` channel.
pub(super) fn spawn_engine_query(
    engine: Arc<QueryEngine>,
    prompt: String,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    tokio::spawn(async move {
        let stream = engine.submit_message(&prompt, QuerySource::ReplMainThread);
        futures::pin_mut!(stream);

        while let Some(msg) = stream.next().await {
            if tx.send(EngineEvent::Sdk(Box::new(msg))).is_err() {
                break; // receiver dropped (app exited)
            }
        }

        let _ = tx.send(EngineEvent::Done);
    });
}

// ---------------------------------------------------------------------------
// SDK message handler
// ---------------------------------------------------------------------------

/// Handle an SDK message from the engine, updating the App state.
pub(super) fn handle_sdk_message(app: &mut App, msg: SdkMessage, ss: &mut StreamingState) {
    match msg {
        SdkMessage::SystemInit(_init) => {
            debug!("TUI: received SystemInit");
        }

        SdkMessage::StreamEvent(sdk_stream) => {
            match sdk_stream.event {
                StreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => {
                    let is_new_message = !ss.active;
                    if is_new_message {
                        // First content block — start a new streaming message
                        ss.blocks.clear();
                        ss.active = true;
                    }
                    while ss.blocks.len() <= index {
                        ss.blocks.push(ContentBlock::Text {
                            text: String::new(),
                        });
                    }
                    ss.blocks[index] = content_block;
                    if is_new_message {
                        app.add_message(make_partial_assistant(&ss.blocks));
                    } else {
                        app.replace_last_message(make_partial_assistant(&ss.blocks));
                    }
                }
                StreamEvent::ContentBlockDelta { index, ref delta } => {
                    let is_new_message = !ss.active && ss.blocks.is_empty();
                    if is_new_message {
                        ss.active = true;
                    }
                    let mut handled = false;
                    if stream_delta_type_matches(delta, "text_delta") {
                        if let Some(t) = delta.get("text").and_then(|v| v.as_str()) {
                            if let ContentBlock::Text { text } = ss.ensure_block(
                                index,
                                ContentBlock::Text {
                                    text: String::new(),
                                },
                            ) {
                                text.push_str(t);
                            }
                            if is_new_message {
                                app.add_message(make_partial_assistant(&ss.blocks));
                            } else {
                                app.replace_last_message(make_partial_assistant(&ss.blocks));
                            }
                            handled = true;
                        }
                    }
                    if !handled && stream_delta_type_matches(delta, "thinking_delta") {
                        if let Some(t) = delta.get("thinking").and_then(|v| v.as_str()) {
                            if let ContentBlock::Thinking { thinking, .. } = ss.ensure_block(
                                index,
                                ContentBlock::Thinking {
                                    thinking: String::new(),
                                    signature: None,
                                },
                            ) {
                                thinking.push_str(t);
                            }
                            if is_new_message {
                                app.add_message(make_partial_assistant(&ss.blocks));
                            } else {
                                app.replace_last_message(make_partial_assistant(&ss.blocks));
                            }
                        }
                    }
                }
                StreamEvent::MessageStop => {
                    // Stream complete; the full Assistant message follows.
                    ss.active = false;
                }
                _ => {}
            }
        }

        SdkMessage::Assistant(assistant) => {
            // Replace the partial streaming message with the final one.
            if ss.is_partial() {
                app.replace_last_message(Message::Assistant(assistant.message));
                ss.clear();
            } else {
                app.add_message(Message::Assistant(assistant.message));
            }
        }

        SdkMessage::UserReplay(user) => {
            if user.is_replay && !user.is_synthetic {
                return;
            }

            let content = match user.content_blocks {
                Some(blocks) => MessageContent::Blocks(blocks),
                None => MessageContent::Text(user.content),
            };
            app.add_message(Message::User(UserMessage {
                uuid: user.uuid,
                timestamp: user.timestamp,
                role: "user".to_string(),
                content,
                is_meta: user.is_synthetic,
                tool_use_result: user.tool_use_result,
                source_tool_assistant_uuid: user.source_tool_assistant_uuid,
            }));
        }

        SdkMessage::Result(result) => {
            // Finalize any leftover streaming state
            ss.clear();

            app.set_streaming(false);
            app.update_session_cost(result.total_cost_usd);
            // Feed aggregate usage into the status-line payload (issue #11).
            // `result.usage` is engine `UsageTracking` (accumulated across
            // turns) — the payload wants per-session totals, so we pass
            // the totals straight through.
            app.update_session_usage(
                result.usage.total_input_tokens,
                result.usage.total_output_tokens,
                result.usage.total_cache_read_tokens,
                result.usage.total_cache_creation_tokens,
                result.usage.api_call_count,
            );
            if result.is_error {
                app.add_message(Message::System(SystemMessage {
                    uuid: uuid::Uuid::new_v4(),
                    timestamp: now_ts(),
                    subtype: SystemSubtype::Informational {
                        level: InfoLevel::Error,
                    },
                    content: result.result,
                }));
            }

            // Generate next-prompt suggestions from last assistant turn
            generate_suggestions(app);

            debug!(
                turns = result.num_turns,
                cost = format!("{:.4}", result.total_cost_usd),
                duration_ms = result.duration_ms,
                "TUI: query completed"
            );
        }

        SdkMessage::ApiRetry(retry) => {
            app.set_spinner_message(format!(
                "Retrying ({}/{})...",
                retry.attempt, retry.max_retries
            ));
        }

        SdkMessage::CompactBoundary(_) => {
            app.add_message(Message::System(SystemMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: now_ts(),
                subtype: SystemSubtype::CompactBoundary {
                    compact_metadata: None,
                },
                content: String::new(),
            }));
        }

        _ => {}
    }
}

fn stream_delta_type_matches(delta: &serde_json::Value, expected: &str) -> bool {
    delta
        .get("type")
        .and_then(|v| v.as_str())
        .map_or(true, |actual| actual == expected)
}

/// Build a partial assistant message for streaming display.
fn make_partial_assistant(blocks: &[ContentBlock]) -> Message {
    Message::Assistant(AssistantMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        role: "assistant".to_string(),
        content: blocks.to_vec(),
        usage: None,
        stop_reason: None,
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    })
}
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a user message from text.
pub(super) fn create_user_message(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        role: "user".to_string(),
        content: MessageContent::Text(text.to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

/// Generate prompt suggestions from the last assistant message in the conversation.
fn generate_suggestions(app: &mut App) {
    let messages = app.messages();
    let mut svc = PromptSuggestionService::new(true);

    // Check suppression first (not enough messages, etc.)
    if let Some(reason) = svc.get_suppression_reason(messages.len(), false) {
        debug!("prompt suggestions suppressed: {:?}", reason);
        return;
    }

    if !svc.should_enable() {
        return;
    }

    // Find last assistant message
    let last_assistant = messages.iter().rev().find_map(|msg| match msg {
        Message::Assistant(a) => Some(a),
        _ => None,
    });
    let Some(assistant) = last_assistant else {
        return;
    };

    // Extract tool names and text summary
    let tool_names: Vec<String> = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let summary: String = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(suggestions) = svc.try_generate(&summary, &tool_names) {
        app.set_suggestions(suggestions);
    }
}

/// Current UTC timestamp in seconds.
pub(super) fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}
