//! SdkMessage -> SSE event conversion.

use std::convert::Infallible;
use std::pin::Pin;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::{Stream, StreamExt};

use crate::engine::sdk_types::SdkMessage;

/// Convert a QueryEngine SdkMessage stream into an SSE event stream.
pub fn sdk_stream_to_sse(
    stream: Pin<Box<dyn Stream<Item = SdkMessage> + Send>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let sse_stream = stream.map(|msg| {
        let event_name = msg.event_name();
        let data = serde_json::to_string(&msg)
            .unwrap_or_else(|e| format!(r#"{{"error":"serialization failed: {}"}}"#, e));
        Ok(Event::default().event(event_name).data(data))
    });
    Sse::new(sse_stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

impl SdkMessage {
    /// Return the SSE event name for this message variant.
    pub fn event_name(&self) -> &'static str {
        match self {
            SdkMessage::SystemInit(_) => "system_init",
            SdkMessage::Assistant(_) => "assistant",
            SdkMessage::UserReplay(_) => "user_replay",
            SdkMessage::StreamEvent(_) => "stream_event",
            SdkMessage::CompactBoundary(_) => "compact_boundary",
            SdkMessage::ApiRetry(_) => "api_retry",
            SdkMessage::ToolUseSummary(_) => "tool_use_summary",
            SdkMessage::Result(_) => "result",
        }
    }
}
