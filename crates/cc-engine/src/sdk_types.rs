//! SDK output message types that QueryEngine yields to external consumers
//! (SDK, Desktop, REPL).
//!
//! Corresponds to TypeScript: SDKMessage (agentSdkTypes.ts)

use serde::Serialize;
use uuid::Uuid;

use crate::lifecycle::{PermissionDenial, UsageTracking};
use crate::types::message::{AssistantMessage, CompactMetadata, StreamEvent};

// ---------------------------------------------------------------------------
// Top-level SDK message enum
// ---------------------------------------------------------------------------

/// SDK output message -- the type yielded by `QueryEngine::submit_message()`.
///
/// Corresponds to TypeScript: SDKMessage (agentSdkTypes.ts)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SdkMessage {
    /// System initialisation (tool list, model info).
    SystemInit(SystemInitMessage),
    /// Assistant message (text, tool calls).
    Assistant(SdkAssistantMessage),
    /// User message replay (for SDK consumers to confirm receipt).
    UserReplay(SdkUserReplay),
    /// Streaming event (real-time text deltas for TUI display).
    StreamEvent(SdkStreamEvent),
    /// Compact boundary (produced after context compaction).
    CompactBoundary(SdkCompactBoundary),
    /// API retry notification.
    ApiRetry(SdkApiRetry),
    /// Tool-use summary.
    ToolUseSummary(SdkToolUseSummary),
    /// Tombstone for an assistant message that was abandoned by fallback retry.
    Tombstone(SdkTombstone),
    /// Final result (every `submit_message` call ends with exactly one of these).
    Result(SdkResult),
}

impl SdkMessage {
    pub fn event_name(&self) -> &'static str {
        match self {
            SdkMessage::SystemInit(_) => "system_init",
            SdkMessage::Assistant(_) => "assistant",
            SdkMessage::UserReplay(_) => "user_replay",
            SdkMessage::StreamEvent(_) => "stream_event",
            SdkMessage::CompactBoundary(_) => "compact_boundary",
            SdkMessage::ApiRetry(_) => "api_retry",
            SdkMessage::ToolUseSummary(_) => "tool_use_summary",
            SdkMessage::Tombstone(_) => "tombstone",
            SdkMessage::Result(_) => "result",
        }
    }
}

// ---------------------------------------------------------------------------
// Individual message structs
// ---------------------------------------------------------------------------

/// System initialisation payload.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInitMessage {
    pub tools: Vec<String>,
    pub model: String,
    pub permission_mode: String,
    pub session_id: String,
    pub uuid: Uuid,
}

/// Assistant message wrapper for SDK output.
#[derive(Debug, Clone, Serialize)]
pub struct SdkAssistantMessage {
    pub message: crate::types::message::AssistantMessage,
    pub session_id: String,
    pub parent_tool_use_id: Option<String>,
}

/// User message replay (echoed back to the SDK consumer).
#[derive(Debug, Clone, Serialize)]
pub struct SdkUserReplay {
    pub content: String,
    pub session_id: String,
    pub uuid: Uuid,
    pub timestamp: i64,
    pub is_replay: bool,
    pub is_synthetic: bool,
    /// Human-readable tool result preview used by TUI/transcript renderers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<String>,
    /// Assistant message that originated the tool call, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool_assistant_uuid: Option<Uuid>,
    /// Structured content blocks (tool results, etc.) — present when
    /// the user message carries `MessageContent::Blocks`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<crate::types::message::ContentBlock>>,
}

/// Streaming event wrapper for SDK output.
#[derive(Debug, Clone, Serialize)]
pub struct SdkStreamEvent {
    pub event: StreamEvent,
    pub session_id: String,
    pub uuid: Uuid,
}

/// Compact boundary marker for SDK output.
#[derive(Debug, Clone, Serialize)]
pub struct SdkCompactBoundary {
    pub session_id: String,
    pub uuid: Uuid,
    pub compact_metadata: Option<CompactMetadata>,
}

/// API retry notification for SDK output.
#[derive(Debug, Clone, Serialize)]
pub struct SdkApiRetry {
    pub attempt: u32,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub error_status: Option<u16>,
    pub error: String,
    pub session_id: String,
    pub uuid: Uuid,
}

/// Tool-use summary for SDK output.
#[derive(Debug, Clone, Serialize)]
pub struct SdkToolUseSummary {
    pub summary: String,
    pub preceding_tool_use_ids: Vec<String>,
    pub session_id: String,
    pub uuid: Uuid,
}

/// Tombstone for an orphaned assistant message.
#[derive(Debug, Clone, Serialize)]
pub struct SdkTombstone {
    pub message: AssistantMessage,
    pub session_id: String,
    pub uuid: Uuid,
}

// ---------------------------------------------------------------------------
// Final result
// ---------------------------------------------------------------------------

/// Final result -- every `submit_message` invocation terminates with exactly
/// one `SdkResult`.
#[derive(Debug, Clone, Serialize)]
pub struct SdkResult {
    pub subtype: ResultSubtype,
    pub is_error: bool,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: usize,
    pub result: String,
    pub stop_reason: Option<String>,
    pub session_id: String,
    pub total_cost_usd: f64,
    pub usage: UsageTracking,
    pub permission_denials: Vec<PermissionDenial>,
    pub structured_output: Option<serde_json::Value>,
    pub uuid: Uuid,
    pub errors: Vec<String>,
}

/// Subtype of the final SDK result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultSubtype {
    Success,
    ErrorDuringExecution,
    ErrorMaxTurns,
    ErrorMaxBudgetUsd,
    #[allow(dead_code)]
    ErrorMaxStructuredOutputRetries,
}
