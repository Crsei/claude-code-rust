//! QueryEngineDeps -- QueryDeps implementation for the QueryEngine.
//!
//! Provides the query loop with access to the engine's shared state
//! (abort flag, app state, tools) and, optionally, a real `ApiClient`
//! for making Anthropic API calls.

use parking_lot::RwLock;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use uuid::Uuid;

use crate::query::deps::{
    CompactionResult, ModelCallParams, ModelResponse, QueryDeps, ToolExecRequest, ToolExecResult,
};
use crate::types::app_state::AppState;
use crate::types::message::{Message, StreamEvent};
use crate::types::state::AutoCompactTracking;
use crate::types::tool::{ToolProgress, Tools};

use super::helpers::{build_messages_request, format_conversation_for_summary};
use super::QueryEngineState;

/// Dependency injection bridge: provides the query loop with access to the
/// engine's shared state (abort flag, app state, tools) and, optionally, a
/// real `ApiClient` for making Anthropic API calls.
pub(crate) struct QueryEngineDeps {
    pub(crate) aborted: Arc<AtomicBool>,
    pub(crate) state: Arc<RwLock<QueryEngineState>>,
    /// When `Some`, the deps will use this client for `call_model` /
    /// `call_model_streaming`. When `None`, those methods bail with a
    /// descriptive error.
    pub(crate) api_client: Option<Arc<crate::api::client::ApiClient>>,
    /// Sub-agent context -- propagated into `ToolUseContext` so that
    /// nested Agent tool calls can enforce recursion depth limits.
    pub(crate) agent_context: Option<crate::types::config::AgentContext>,
}

#[async_trait::async_trait]
impl QueryDeps for QueryEngineDeps {
    async fn call_model(&self, mut params: ModelCallParams) -> Result<ModelResponse> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model: no API client configured -- \
                 set ANTHROPIC_API_KEY, use /login to store a key, or provide a mock in tests"
            )
        })?;

        // Fill model: AppState (user/config/env) > provider default
        if params.model.is_none() {
            let app_model = self.state.read().app_state.main_loop_model.clone();
            params.model = Some(if app_model.is_empty() {
                client.config().default_model.clone()
            } else {
                app_model
            });
        }

        let request = build_messages_request(&params);
        let stream = client.messages_stream(request).await?;
        let mut stream = std::pin::pin!(stream);

        let mut accumulator = crate::api::streaming::StreamAccumulator::new();
        let mut stream_events = Vec::new();

        use futures::StreamExt;
        while let Some(event_result) = stream.next().await {
            let event = event_result?;
            accumulator.process_event(&event);
            stream_events.push(event);
        }

        let usage = accumulator.usage.clone();
        let assistant_message = accumulator.build();

        Ok(ModelResponse {
            assistant_message,
            stream_events,
            usage,
        })
    }

    async fn call_model_streaming(
        &self,
        mut params: ModelCallParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let client = self.api_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "call_model_streaming: no API client configured -- \
                 set ANTHROPIC_API_KEY, use /login to store a key, or provide a mock in tests"
            )
        })?;

        // Fill model: AppState (user/config/env) > provider default
        if params.model.is_none() {
            let app_model = self.state.read().app_state.main_loop_model.clone();
            params.model = Some(if app_model.is_empty() {
                client.config().default_model.clone()
            } else {
                app_model
            });
        }

        let request = build_messages_request(&params);
        client.messages_stream(request).await
    }

    async fn microcompact(&self, messages: Vec<Message>) -> Result<Vec<Message>> {
        let result = crate::compact::microcompact::microcompact_messages(messages);
        if result.tokens_freed > 0 {
            tracing::debug!(
                tokens_freed = result.tokens_freed,
                "microcompact: trimmed old tool results"
            );
        }
        Ok(result.messages)
    }

    async fn autocompact(
        &self,
        messages: Vec<Message>,
        tracking: Option<AutoCompactTracking>,
    ) -> Result<Option<CompactionResult>> {
        let model = {
            let app = &self.state.read().app_state;
            if app.main_loop_model.is_empty() {
                self.api_client
                    .as_ref()
                    .map(|c| c.config().default_model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
            } else {
                app.main_loop_model.clone()
            }
        };

        // Run the local context pipeline (budget -> snip -> microcompact -> auto-compact check)
        let pipeline_result = crate::compact::pipeline::run_context_pipeline(
            messages.clone(),
            tracking.clone(),
            &model,
        )
        .await;

        // If auto-compact was triggered AND we have an API client, generate a model summary
        if let Some(ref updated_tracking) = pipeline_result.tracking {
            if updated_tracking.compacted {
                // Try model-based summarization if API client is available
                if let Some(ref _client) = self.api_client {
                    let summary_prompt = crate::compact::compaction::build_compaction_prompt();
                    let pre_tokens = crate::utils::tokens::estimate_messages_tokens(&messages);

                    // Build a summarization request
                    let summary_messages =
                        vec![Message::User(crate::types::message::UserMessage {
                            uuid: Uuid::new_v4(),
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            role: "user".into(),
                            content: crate::types::message::MessageContent::Text(
                                format_conversation_for_summary(&messages),
                            ),
                            is_meta: true,
                            tool_use_result: None,
                            source_tool_assistant_uuid: None,
                        })];

                    let summary_params = ModelCallParams {
                        messages: summary_messages,
                        system_prompt: vec![summary_prompt],
                        tools: vec![],
                        model: Some(model.clone()),
                        max_output_tokens: Some(20_000),
                        skip_cache_write: Some(true),
                        thinking_enabled: None,
                        effort_value: None,
                    };

                    match self.call_model(summary_params).await {
                        Ok(response) => {
                            // Extract summary text from assistant response
                            let summary_text = response
                                .assistant_message
                                .content
                                .iter()
                                .filter_map(|b| match b {
                                    crate::types::message::ContentBlock::Text { text } => {
                                        Some(text.as_str())
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");

                            let config = crate::compact::compaction::CompactionConfig {
                                model: model.clone(),
                                session_id: String::new(),
                                query_source: "compact".into(),
                            };

                            let post_messages =
                                crate::compact::compaction::build_post_compact_messages(
                                    &summary_text,
                                    &messages,
                                    &config,
                                );

                            let post_tokens =
                                crate::utils::tokens::estimate_messages_tokens(&post_messages);

                            tracing::info!(
                                pre_tokens = pre_tokens,
                                post_tokens = post_tokens,
                                "autocompact: model-based summary complete"
                            );

                            let new_tracking = crate::compact::compaction::tracking_on_success(
                                tracking.as_ref(),
                                &Uuid::new_v4().to_string(),
                            );

                            return Ok(Some(CompactionResult {
                                messages: post_messages,
                                tracking: new_tracking,
                            }));
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "autocompact: model summary failed, using local pipeline");
                            let new_tracking =
                                crate::compact::compaction::tracking_on_failure(tracking.as_ref());
                            // Fall through to local-only result
                            return Ok(Some(CompactionResult {
                                messages: pipeline_result.messages,
                                tracking: new_tracking,
                            }));
                        }
                    }
                }

                // No API client -- return local pipeline result
                return Ok(Some(CompactionResult {
                    messages: pipeline_result.messages,
                    tracking: updated_tracking.clone(),
                }));
            }
        }

        // Pipeline ran but auto-compact was not triggered -- return local compacted messages
        if pipeline_result.compacted {
            return Ok(Some(CompactionResult {
                messages: pipeline_result.messages,
                tracking: tracking.unwrap_or(AutoCompactTracking {
                    compacted: false,
                    turn_counter: 0,
                    turn_id: String::new(),
                    consecutive_failures: 0,
                }),
            }));
        }

        Ok(None)
    }

    async fn reactive_compact(&self, messages: Vec<Message>) -> Result<Option<CompactionResult>> {
        let model = {
            let app = &self.state.read().app_state;
            if app.main_loop_model.is_empty() {
                self.api_client
                    .as_ref()
                    .map(|c| c.config().default_model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
            } else {
                app.main_loop_model.clone()
            }
        };

        match crate::compact::pipeline::try_reactive_compact(messages, &model).await {
            Some(result) => {
                tracing::info!(
                    tokens_freed = result.tokens_freed,
                    "reactive compact: freed tokens via emergency pipeline"
                );
                Ok(Some(CompactionResult {
                    messages: result.messages,
                    tracking: result.tracking,
                }))
            }
            None => Ok(None),
        }
    }

    async fn execute_tool(
        &self,
        request: ToolExecRequest,
        tools: &Tools,
        parent_message: &crate::types::message::AssistantMessage,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolExecResult> {
        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool_name)
            .ok_or_else(|| anyhow::anyhow!("tool not found: {}", request.tool_name))?;

        let ctx = crate::types::tool::ToolUseContext {
            options: crate::types::tool::ToolUseOptions {
                debug: false,
                main_loop_model: self.state.read().app_state.main_loop_model.clone(),
                verbose: self.state.read().app_state.verbose,
                is_non_interactive_session: false,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: {
                let (tx, rx) = tokio::sync::watch::channel(false);
                if self.aborted.load(Ordering::Relaxed) {
                    let _ = tx.send(true);
                }
                rx
            },
            read_file_state: crate::types::tool::FileStateCache::default(),
            get_app_state: {
                let state = self.state.clone();
                Arc::new(move || state.read().app_state.clone())
            },
            set_app_state: {
                let state = self.state.clone();
                Arc::new(move |updater: Box<dyn FnOnce(AppState) -> AppState>| {
                    let mut s = state.write();
                    let old = s.app_state.clone();
                    s.app_state = updater(old);
                })
            },
            messages: vec![],
            agent_id: self.agent_context.as_ref().map(|ac| ac.agent_id.clone()),
            agent_type: self.agent_context.as_ref().map(|_| "subagent".to_string()),
            query_tracking: self
                .agent_context
                .as_ref()
                .map(|ac| ac.query_tracking.clone()),
        };

        match tool.call(request.input, &ctx, parent_message, None).await {
            Ok(result) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result,
                is_error: false,
            }),
            Err(e) => Ok(ToolExecResult {
                tool_use_id: request.tool_use_id,
                tool_name: request.tool_name,
                result: crate::types::tool::ToolResult {
                    data: serde_json::json!(format!("Error: {}", e)),
                    new_messages: vec![],
                },
                is_error: true,
            }),
        }
    }

    fn get_app_state(&self) -> AppState {
        self.state.read().app_state.clone()
    }

    fn uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    fn get_tools(&self) -> Tools {
        self.state.read().tools.clone()
    }

    async fn refresh_tools(&self) -> Result<Tools> {
        Ok(self.state.read().tools.clone())
    }
}
