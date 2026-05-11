//! Query-turn client path helper.

use std::sync::Arc;

use futures::{Stream, StreamExt};
use parking_lot::Mutex;
use tracing::error;

use crate::sink::FrontendSink;

/// Minimal host capability needed by the IPC client query path.
pub trait QueryTurnHost<SdkMessage>: Send + Sync + 'static {
    type Stream: Stream<Item = SdkMessage> + Send + 'static;

    fn reset_abort(&self);
    fn submit_client_message(&self, prompt_text: &str) -> Self::Stream;
}

/// Spawn a query turn as a background task and map each SDK message to IPC.
pub fn spawn_query_turn<E, Svc, SdkMessage, F>(
    engine: Arc<E>,
    prompt_text: String,
    message_id: String,
    suggestion_svc: Arc<Mutex<Svc>>,
    sink: FrontendSink,
    mut handle_sdk_message: F,
) where
    E: QueryTurnHost<SdkMessage>,
    Svc: Send + 'static,
    SdkMessage: Send + 'static,
    F: FnMut(&SdkMessage, &str, &Arc<E>, &Arc<Mutex<Svc>>, &FrontendSink) -> std::io::Result<()>
        + Send
        + 'static,
{
    tokio::spawn(async move {
        engine.reset_abort();

        let stream = engine.submit_client_message(&prompt_text);
        let mut stream = std::pin::pin!(stream);

        while let Some(sdk_msg) = stream.next().await {
            if let Err(err) =
                handle_sdk_message(&sdk_msg, &message_id, &engine, &suggestion_svc, &sink)
            {
                error!("query_runner: send to frontend failed: {}", err);
                break;
            }
        }
    });
}
