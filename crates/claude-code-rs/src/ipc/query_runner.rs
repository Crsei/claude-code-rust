//! Compatibility adapter for the IPC client query-turn path.

use std::sync::Arc;

use futures::stream::BoxStream;
use parking_lot::Mutex;

use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::config::QuerySource;

use super::sdk_mapper::handle_sdk_message;
use super::sink::FrontendSink;

impl cc_ipc_client::query_runner::QueryTurnHost<SdkMessage> for QueryEngine {
    type Stream = BoxStream<'static, SdkMessage>;

    fn reset_abort(&self) {
        QueryEngine::reset_abort(self);
    }

    fn submit_client_message(&self, prompt_text: &str) -> Self::Stream {
        Box::pin(QueryEngine::submit_message(
            self,
            prompt_text,
            QuerySource::ReplMainThread,
        ))
    }
}

/// Spawn a query turn as a background tokio task.
pub fn spawn_query_turn(
    engine: Arc<QueryEngine>,
    prompt_text: String,
    message_id: String,
    suggestion_svc: Arc<Mutex<PromptSuggestionService>>,
    sink: FrontendSink,
) {
    cc_ipc_client::query_runner::spawn_query_turn(
        engine,
        prompt_text,
        message_id,
        suggestion_svc,
        sink,
        handle_sdk_message,
    );
}
