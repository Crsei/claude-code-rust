//! Unified query turn spawning.
//!
//! Both `FrontendMessage::SubmitPrompt` and `CommandResult::Query` need to
//! start a query turn against the engine, stream the results through the SDK
//! mapper, and forward [`BackendMessage`]s to the frontend.  This module
//! provides the single entry point so the two paths cannot drift apart.

use std::sync::Arc;

use futures::StreamExt;
use parking_lot::Mutex;
use tracing::error;

use crate::engine::lifecycle::QueryEngine;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::config::QuerySource;

use super::sdk_mapper::handle_sdk_message;
use super::sink::FrontendSink;

/// Spawn a query turn as a background tokio task.
///
/// The engine's abort flag is reset, a streaming query is submitted, and each
/// [`SdkMessage`] is mapped to the frontend protocol via [`handle_sdk_message`].
///
/// This is the **only** place query turns should be spawned from the headless
/// event loop — both user prompts and slash-command–generated queries call here.
pub fn spawn_query_turn(
    engine: Arc<QueryEngine>,
    prompt_text: String,
    message_id: String,
    suggestion_svc: Arc<Mutex<PromptSuggestionService>>,
    sink: FrontendSink,
) {
    tokio::spawn(async move {
        engine.reset_abort();

        let stream = engine.submit_message(&prompt_text, QuerySource::ReplMainThread);
        let mut stream = std::pin::pin!(stream);

        while let Some(sdk_msg) = stream.next().await {
            if let Err(e) =
                handle_sdk_message(&sdk_msg, &message_id, &engine, &suggestion_svc, &sink)
            {
                error!("query_runner: send to frontend failed: {}", e);
                break;
            }
        }
    });
}
