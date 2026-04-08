//! Session export — produce a structured, analyzable, replayable session record.
//!
//! Unlike the audit export (tamper-proof hash chain) or the Markdown export
//! (human-readable summary), the session export produces a JSON "data package"
//! that includes:
//!
//! - **Transcript**: all messages in their full JSON form + counts
//! - **Tool call timeline**: tool_use ↔ tool_result pairs reconstructed in order
//! - **Compression events**: compact boundaries, content replacements (detected
//!   from tool result preview markers)
//! - **Context snapshot**: token estimates, cost breakdown, tool usage statistics
//! - **Session metadata**: git branch, project path, model, timestamps
//!
//! Storage: `~/.cc-rust/exports/<session_id>.session.json`

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::bootstrap::PROCESS_STATE;
use crate::compact::auto_compact::get_context_window_size;
use crate::session::storage::{self, SessionFile};
use crate::types::message::{
    ContentBlock, Message, MessageContent, SystemSubtype, ToolResultContent,
};
use crate::utils::tokens::estimate_messages_tokens;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Top-level session export — the full analyzable data package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub session: SessionMeta,
    pub transcript: TranscriptData,
    pub tool_calls: Vec<ToolCallRecord>,
    pub compression: CompressionData,
    pub context: ContextSnapshot,
}

/// Session-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub project_path: Option<String>,
    pub git_branch: Option<String>,
    pub git_head_sha: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

/// Raw transcript data — full messages + breakdown counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptData {
    pub messages: Vec<serde_json::Value>,
    pub message_count: usize,
    pub user_message_count: usize,
    pub assistant_message_count: usize,
    pub system_message_count: usize,
}

/// A single tool call with its matched result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Sequential index in the tool call timeline.
    pub sequence: usize,
    /// UUID of the assistant message that initiated this tool call.
    pub assistant_uuid: String,
    /// Unique tool_use_id from the API.
    pub tool_use_id: String,
    /// Tool name (e.g. "Bash", "Read", "Edit").
    pub tool_name: String,
    /// Tool input parameters.
    pub input: serde_json::Value,
    /// Matched tool_result content (None if no result found).
    pub result: Option<serde_json::Value>,
    /// Whether the tool result was an error.
    pub is_error: bool,
    /// Timestamp of the assistant message (ISO-8601).
    pub timestamp: String,
    /// Timestamp of the user message carrying the tool_result.
    pub result_timestamp: Option<String>,
    /// Whether the tool result was replaced by tool_result_budget (large
    /// results saved to disk and replaced with a preview).
    pub was_content_replaced: bool,
}

/// Compression/compaction event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionData {
    pub compact_boundaries: Vec<CompactBoundaryRecord>,
    pub content_replacements: Vec<ContentReplacementRecord>,
    pub microcompact_replacements: Vec<MicrocompactRecord>,
    pub total_compactions: usize,
}

/// A compact boundary (conversation-level compaction event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundaryRecord {
    pub uuid: String,
    pub timestamp: String,
    pub pre_compact_tokens: Option<u64>,
    pub post_compact_tokens: Option<u64>,
    pub boundary_text: String,
}

/// A content replacement record (tool result saved to disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReplacementRecord {
    pub tool_use_id: String,
    pub original_size_hint: Option<usize>,
    pub file_path: Option<String>,
}

/// A microcompact replacement record (old large tool result truncated in-place).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrocompactRecord {
    pub tool_use_id: String,
    pub omitted_chars: usize,
}

/// Context snapshot — token and cost statistics at export time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub estimated_total_tokens: u64,
    pub context_window_size: u64,
    pub utilization_pct: f64,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub api_call_count: usize,
    pub tool_use_count: usize,
    pub unique_tools_used: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API — export
// ---------------------------------------------------------------------------

/// Export the current in-memory conversation as a structured session record.
/// Returns the path written and the export data (for summary display).
pub fn export_session(
    session_id: &str,
    messages: &[Message],
    cwd: &str,
    output_path: Option<&Path>,
) -> Result<(PathBuf, SessionExport)> {
    let export = build_session_export(session_id, messages, cwd);
    let path = write_session_export(&export, session_id, output_path)?;
    Ok((path, export))
}

/// Export a saved session (by ID) as a structured session record.
pub fn export_saved_session(
    session_id: &str,
    output_path: Option<&Path>,
) -> Result<(PathBuf, SessionExport)> {
    let session_file = load_session_file_raw(session_id)?;
    let messages = storage::load_session(session_id)?;
    let export = build_session_export(session_id, &messages, &session_file.cwd);
    let path = write_session_export(&export, session_id, output_path)?;
    Ok((path, export))
}

/// List all session export files.
pub fn list_session_exports() -> Result<Vec<PathBuf>> {
    let dir = get_export_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".session.json"))
        .collect();
    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// Core builder
// ---------------------------------------------------------------------------

/// Build the full SessionExport from in-memory messages.
pub fn build_session_export(session_id: &str, messages: &[Message], cwd: &str) -> SessionExport {
    let session_meta = build_session_meta(session_id, messages, cwd);
    let transcript = build_transcript_data(messages);
    let tool_calls = reconstruct_tool_timeline(messages);
    let compression = extract_compression_events(messages);
    let context = build_context_snapshot(messages);

    SessionExport {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        session: session_meta,
        transcript,
        tool_calls,
        compression,
        context,
    }
}

// ---------------------------------------------------------------------------
// Session metadata
// ---------------------------------------------------------------------------

fn build_session_meta(session_id: &str, messages: &[Message], cwd: &str) -> SessionMeta {
    let cwd_path = Path::new(cwd);

    let git_branch = crate::utils::git::current_branch(cwd_path).ok();
    let git_head_sha = crate::utils::git::head_sha(cwd_path).ok();

    let model = PROCESS_STATE
        .read()
        .effective_model()
        .map(|s| s.to_string());

    let project_path = {
        let p = PROCESS_STATE
            .read()
            .project_root
            .to_string_lossy()
            .to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };

    let started_at = messages.first().map(|m| format_ts_millis(m.timestamp()));
    let ended_at = messages.last().map(|m| format_ts_millis(m.timestamp()));

    SessionMeta {
        session_id: session_id.to_string(),
        project_path,
        git_branch,
        git_head_sha,
        model,
        started_at,
        ended_at,
    }
}

// ---------------------------------------------------------------------------
// Transcript data
// ---------------------------------------------------------------------------

fn build_transcript_data(messages: &[Message]) -> TranscriptData {
    let mut user_count = 0usize;
    let mut assistant_count = 0usize;
    let mut system_count = 0usize;

    let json_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|msg| {
            match msg {
                Message::User(_) => user_count += 1,
                Message::Assistant(_) => assistant_count += 1,
                Message::System(_) => system_count += 1,
                _ => {}
            }
            message_to_json(msg)
        })
        .collect();

    TranscriptData {
        message_count: json_messages.len(),
        messages: json_messages,
        user_message_count: user_count,
        assistant_message_count: assistant_count,
        system_message_count: system_count,
    }
}

// ---------------------------------------------------------------------------
// Tool timeline reconstruction
// ---------------------------------------------------------------------------

/// Reconstruct the tool call timeline by matching tool_use blocks in
/// assistant messages to tool_result blocks in subsequent user messages.
pub fn reconstruct_tool_timeline(messages: &[Message]) -> Vec<ToolCallRecord> {
    // Phase 1: collect all tool_use entries from assistant messages
    struct PendingToolUse {
        assistant_uuid: String,
        tool_use_id: String,
        tool_name: String,
        input: serde_json::Value,
        timestamp: i64,
    }

    let mut pending: HashMap<String, PendingToolUse> = HashMap::new();
    let mut timeline: Vec<ToolCallRecord> = Vec::new();
    let mut seq = 0usize;

    for msg in messages {
        match msg {
            Message::Assistant(a) => {
                for block in &a.content {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        pending.insert(
                            id.clone(),
                            PendingToolUse {
                                assistant_uuid: a.uuid.to_string(),
                                tool_use_id: id.clone(),
                                tool_name: name.clone(),
                                input: input.clone(),
                                timestamp: a.timestamp,
                            },
                        );
                    }
                }
            }
            Message::User(u) => {
                let blocks = match &u.content {
                    MessageContent::Blocks(b) => b.as_slice(),
                    _ => &[],
                };
                for block in blocks {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = block
                    {
                        let result_text = tool_result_content_to_json(content);
                        let was_replaced = detect_content_replacement_in_result(content);

                        if let Some(p) = pending.remove(tool_use_id) {
                            timeline.push(ToolCallRecord {
                                sequence: seq,
                                assistant_uuid: p.assistant_uuid,
                                tool_use_id: p.tool_use_id,
                                tool_name: p.tool_name,
                                input: p.input,
                                result: Some(result_text),
                                is_error: *is_error,
                                timestamp: format_ts_millis(p.timestamp),
                                result_timestamp: Some(format_ts_millis(u.timestamp)),
                                was_content_replaced: was_replaced,
                            });
                            seq += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Phase 2: any unmatched tool_uses (no result received — user interrupt, etc.)
    let mut unmatched: Vec<_> = pending.into_values().collect();
    unmatched.sort_by_key(|p| p.timestamp);
    for p in unmatched {
        timeline.push(ToolCallRecord {
            sequence: seq,
            assistant_uuid: p.assistant_uuid,
            tool_use_id: p.tool_use_id,
            tool_name: p.tool_name,
            input: p.input,
            result: None,
            is_error: false,
            timestamp: format_ts_millis(p.timestamp),
            result_timestamp: None,
            was_content_replaced: false,
        });
        seq += 1;
    }

    timeline
}

// ---------------------------------------------------------------------------
// Compression event extraction
// ---------------------------------------------------------------------------

/// Extract compression events from the message history.
pub fn extract_compression_events(messages: &[Message]) -> CompressionData {
    let mut compact_boundaries = Vec::new();
    let mut content_replacements = Vec::new();
    let mut microcompact_replacements = Vec::new();

    for msg in messages {
        // Extract compact boundaries from system messages
        if let Message::System(s) = msg {
            if let SystemSubtype::CompactBoundary { compact_metadata } = &s.subtype {
                compact_boundaries.push(CompactBoundaryRecord {
                    uuid: s.uuid.to_string(),
                    timestamp: format_ts_millis(s.timestamp),
                    pre_compact_tokens: compact_metadata
                        .as_ref()
                        .map(|m| m.pre_compact_token_count),
                    post_compact_tokens: compact_metadata
                        .as_ref()
                        .map(|m| m.post_compact_token_count),
                    boundary_text: s.content.clone(),
                });
            }
        }

        // Detect content replacements and microcompact events in user messages
        if let Message::User(u) = msg {
            let blocks = match &u.content {
                MessageContent::Blocks(b) => b.as_slice(),
                _ => &[],
            };
            for block in blocks {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } = block
                {
                    if let Some(record) = detect_content_replacement(tool_use_id, content) {
                        content_replacements.push(record);
                    }
                    if let Some(record) = detect_microcompact(tool_use_id, content) {
                        microcompact_replacements.push(record);
                    }
                }
            }
        }
    }

    let total_compactions = compact_boundaries.len();

    CompressionData {
        compact_boundaries,
        content_replacements,
        microcompact_replacements,
        total_compactions,
    }
}

/// Detect if a tool_result was replaced by the tool_result_budget system.
///
/// Looks for the pattern:
/// `[... N characters omitted. Full output saved to: <path> ...]`
pub fn detect_content_replacement(
    tool_use_id: &str,
    content: &ToolResultContent,
) -> Option<ContentReplacementRecord> {
    let text = match content {
        ToolResultContent::Text(t) => t.as_str(),
        ToolResultContent::Blocks(blocks) => {
            // Check each text block
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if let Some(record) = parse_replacement_marker(tool_use_id, text) {
                        return Some(record);
                    }
                }
            }
            return None;
        }
    };
    parse_replacement_marker(tool_use_id, text)
}

/// Parse the replacement marker from text content.
fn parse_replacement_marker(tool_use_id: &str, text: &str) -> Option<ContentReplacementRecord> {
    // Match: [... N characters omitted. Full output saved to: <path> ...]
    let re = Regex::new(
        r"\[\.\.\.\s+(\d+)\s+characters omitted\.\s+Full output saved to:\s+(.+?)\s*\.\.\.\]",
    )
    .ok()?;

    let caps = re.captures(text)?;
    let omitted_chars: usize = caps.get(1)?.as_str().parse().ok()?;
    let file_path = caps.get(2)?.as_str().to_string();

    Some(ContentReplacementRecord {
        tool_use_id: tool_use_id.to_string(),
        original_size_hint: Some(omitted_chars),
        file_path: Some(file_path),
    })
}

/// Detect if a tool result was microcompacted (old large result truncated in-place).
///
/// Looks for the pattern:
/// `[... N characters omitted (microcompacted) ...]`
fn detect_microcompact(
    tool_use_id: &str,
    content: &ToolResultContent,
) -> Option<MicrocompactRecord> {
    let text = match content {
        ToolResultContent::Text(t) => t.as_str(),
        ToolResultContent::Blocks(blocks) => {
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if let Some(record) = parse_microcompact_marker(tool_use_id, text) {
                        return Some(record);
                    }
                }
            }
            return None;
        }
    };
    parse_microcompact_marker(tool_use_id, text)
}

/// Parse the microcompact marker from text content.
fn parse_microcompact_marker(tool_use_id: &str, text: &str) -> Option<MicrocompactRecord> {
    let re =
        Regex::new(r"\[\.\.\.\s+(\d+)\s+characters omitted \(microcompacted\)\s*\.\.\.\]").ok()?;

    let caps = re.captures(text)?;
    let omitted_chars: usize = caps.get(1)?.as_str().parse().ok()?;

    Some(MicrocompactRecord {
        tool_use_id: tool_use_id.to_string(),
        omitted_chars,
    })
}

/// Check if a tool result content has any replacement marker (used by
/// tool timeline reconstruction).
fn detect_content_replacement_in_result(content: &ToolResultContent) -> bool {
    let text = match content {
        ToolResultContent::Text(t) => t.clone(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };
    text.contains("characters omitted. Full output saved to:")
        || text.contains("characters omitted (truncated in place)")
        || text.contains("characters omitted (microcompacted)")
}

// ---------------------------------------------------------------------------
// Context snapshot
// ---------------------------------------------------------------------------

/// Build a context snapshot with token/cost/tool statistics.
pub fn build_context_snapshot(messages: &[Message]) -> ContextSnapshot {
    let estimated_total_tokens = estimate_messages_tokens(messages);

    let model = PROCESS_STATE
        .read()
        .effective_model()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let context_window = get_context_window_size(&model);

    let utilization_pct = if context_window > 0 {
        (estimated_total_tokens as f64 / context_window as f64) * 100.0
    } else {
        0.0
    };

    let mut total_cost = 0.0_f64;
    let mut total_input = 0_u64;
    let mut total_output = 0_u64;
    let mut cache_read = 0_u64;
    let mut api_calls = 0_usize;
    let mut tool_use_count = 0_usize;
    let mut tools_seen: HashSet<String> = HashSet::new();

    for msg in messages {
        if let Message::Assistant(a) = msg {
            total_cost += a.cost_usd;
            if let Some(ref usage) = a.usage {
                total_input += usage.input_tokens;
                total_output += usage.output_tokens;
                cache_read += usage.cache_read_input_tokens;
            }
            // Count this as an API call if it has usage data or non-empty content
            if a.usage.is_some() || !a.content.is_empty() {
                api_calls += 1;
            }

            for block in &a.content {
                if let ContentBlock::ToolUse { name, .. } = block {
                    tool_use_count += 1;
                    tools_seen.insert(name.clone());
                }
            }
        }
    }

    let mut unique_tools: Vec<String> = tools_seen.into_iter().collect();
    unique_tools.sort();

    ContextSnapshot {
        estimated_total_tokens,
        context_window_size: context_window,
        utilization_pct,
        total_cost_usd: total_cost,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        cache_read_tokens: cache_read,
        api_call_count: api_calls,
        tool_use_count,
        unique_tools_used: unique_tools,
    }
}

// ---------------------------------------------------------------------------
// Message serialization
// ---------------------------------------------------------------------------

/// Convert a Message to a full JSON value (preserving all data).
fn message_to_json(msg: &Message) -> serde_json::Value {
    match msg {
        Message::User(u) => {
            let content_value = match &u.content {
                MessageContent::Text(t) => serde_json::json!(t),
                MessageContent::Blocks(blocks) => serde_json::json!(blocks),
            };
            serde_json::json!({
                "type": "user",
                "uuid": u.uuid.to_string(),
                "timestamp": u.timestamp,
                "role": "user",
                "content": content_value,
                "is_meta": u.is_meta,
                "tool_use_result": u.tool_use_result,
                "source_tool_assistant_uuid": u.source_tool_assistant_uuid,
            })
        }
        Message::Assistant(a) => serde_json::json!({
            "type": "assistant",
            "uuid": a.uuid.to_string(),
            "timestamp": a.timestamp,
            "role": "assistant",
            "content": a.content,
            "usage": a.usage,
            "stop_reason": a.stop_reason,
            "is_api_error_message": a.is_api_error_message,
            "api_error": a.api_error,
            "cost_usd": a.cost_usd,
        }),
        Message::System(s) => serde_json::json!({
            "type": "system",
            "uuid": s.uuid.to_string(),
            "timestamp": s.timestamp,
            "subtype": format!("{:?}", s.subtype),
            "content": s.content,
        }),
        Message::Progress(p) => serde_json::json!({
            "type": "progress",
            "uuid": p.uuid.to_string(),
            "timestamp": p.timestamp,
            "tool_use_id": p.tool_use_id,
            "data": p.data,
        }),
        Message::Attachment(a) => serde_json::json!({
            "type": "attachment",
            "uuid": a.uuid.to_string(),
            "timestamp": a.timestamp,
            "attachment": a.attachment,
        }),
    }
}

/// Convert ToolResultContent to a JSON value.
fn tool_result_content_to_json(content: &ToolResultContent) -> serde_json::Value {
    match content {
        ToolResultContent::Text(t) => serde_json::json!(t),
        ToolResultContent::Blocks(blocks) => serde_json::json!(blocks),
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn get_export_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".cc-rust").join("exports")
}

fn write_session_export(
    export: &SessionExport,
    session_id: &str,
    output_path: Option<&Path>,
) -> Result<PathBuf> {
    let path = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = get_export_dir();
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create export directory {}", dir.display()))?;
            dir.join(format!("{}.session.json", session_id))
        }
    };

    let json =
        serde_json::to_string_pretty(export).context("Failed to serialize session export")?;

    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write session export {}", path.display()))?;

    Ok(path)
}

fn load_session_file_raw(session_id: &str) -> Result<SessionFile> {
    let path = storage::get_session_file(session_id);
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read session file {}", path.display()))?;
    let file: SessionFile = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse session file {}", path.display()))?;
    Ok(file)
}

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

fn format_ts_millis(ts: i64) -> String {
    let secs = ts / 1000;
    let nanos = ((ts % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("{}", ts))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::*;
    use uuid::Uuid;

    fn make_user_msg(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1700000000000,
            role: "user".into(),
            content: MessageContent::Text(text.into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    fn make_assistant_msg(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1700000001000,
            role: "assistant".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
            usage: Some(Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: 10,
                cache_creation_input_tokens: 0,
            }),
            stop_reason: Some("end_turn".into()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.001,
        })
    }

    fn make_assistant_with_tool_use(tool_use_id: &str, tool_name: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1700000002000,
            role: "assistant".into(),
            content: vec![ContentBlock::ToolUse {
                id: tool_use_id.into(),
                name: tool_name.into(),
                input: serde_json::json!({"command": "ls"}),
            }],
            usage: Some(Usage {
                input_tokens: 200,
                output_tokens: 30,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
            stop_reason: Some("tool_use".into()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.002,
        })
    }

    fn make_tool_result_msg(tool_use_id: &str, result: &str, is_error: bool) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1700000003000,
            role: "user".into(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: ToolResultContent::Text(result.into()),
                is_error,
            }]),
            is_meta: true,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    fn make_compact_boundary(pre: u64, post: u64) -> Message {
        Message::System(SystemMessage {
            uuid: Uuid::new_v4(),
            timestamp: 1700000004000,
            subtype: SystemSubtype::CompactBoundary {
                compact_metadata: Some(CompactMetadata {
                    pre_compact_token_count: pre,
                    post_compact_token_count: post,
                }),
            },
            content: format!("[Compacted: {} → {} tokens]", pre, post),
        })
    }

    #[test]
    fn test_reconstruct_tool_timeline_basic() {
        let messages = vec![
            make_user_msg("hello"),
            make_assistant_with_tool_use("tu_1", "Bash"),
            make_tool_result_msg("tu_1", "file1.rs\nfile2.rs", false),
            make_assistant_msg("Here are the files."),
        ];

        let timeline = reconstruct_tool_timeline(&messages);
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].tool_name, "Bash");
        assert_eq!(timeline[0].tool_use_id, "tu_1");
        assert!(timeline[0].result.is_some());
        assert!(!timeline[0].is_error);
        assert!(!timeline[0].was_content_replaced);
    }

    #[test]
    fn test_reconstruct_multiple_tool_calls() {
        let messages = vec![
            make_user_msg("read two files"),
            make_assistant_with_tool_use("tu_1", "Read"),
            make_tool_result_msg("tu_1", "contents of file1", false),
            make_assistant_with_tool_use("tu_2", "Read"),
            make_tool_result_msg("tu_2", "contents of file2", false),
            make_assistant_msg("Done."),
        ];

        let timeline = reconstruct_tool_timeline(&messages);
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].sequence, 0);
        assert_eq!(timeline[1].sequence, 1);
    }

    #[test]
    fn test_reconstruct_unmatched_tool_use() {
        let messages = vec![
            make_user_msg("do something"),
            make_assistant_with_tool_use("tu_orphan", "Bash"),
            // No tool_result — user interrupted
            make_user_msg("never mind"),
        ];

        let timeline = reconstruct_tool_timeline(&messages);
        assert_eq!(timeline.len(), 1);
        assert!(timeline[0].result.is_none());
        assert_eq!(timeline[0].tool_use_id, "tu_orphan");
    }

    #[test]
    fn test_extract_compact_boundaries() {
        let messages = vec![
            make_user_msg("hello"),
            make_assistant_msg("hi"),
            make_compact_boundary(150000, 50000),
            make_user_msg("after compact"),
        ];

        let compression = extract_compression_events(&messages);
        assert_eq!(compression.compact_boundaries.len(), 1);
        assert_eq!(compression.total_compactions, 1);
        assert_eq!(
            compression.compact_boundaries[0].pre_compact_tokens,
            Some(150000)
        );
        assert_eq!(
            compression.compact_boundaries[0].post_compact_tokens,
            Some(50000)
        );
    }

    #[test]
    fn test_detect_content_replacement() {
        let marker = "head text\n\n[... 50000 characters omitted. Full output saved to: /tmp/tool-results/tu_big.txt ...]\n\ntail text";
        let content = ToolResultContent::Text(marker.into());
        let record = detect_content_replacement("tu_big", &content);
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.tool_use_id, "tu_big");
        assert_eq!(r.original_size_hint, Some(50000));
        assert_eq!(r.file_path.as_deref(), Some("/tmp/tool-results/tu_big.txt"));
    }

    #[test]
    fn test_detect_no_replacement() {
        let content = ToolResultContent::Text("normal output".into());
        assert!(detect_content_replacement("tu_1", &content).is_none());
    }

    #[test]
    fn test_detect_microcompact() {
        let marker = "head text\n\n[... 1500 characters omitted (microcompacted) ...]\n\ntail text";
        let content = ToolResultContent::Text(marker.into());
        let record = detect_microcompact("tu_mc", &content);
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.tool_use_id, "tu_mc");
        assert_eq!(r.omitted_chars, 1500);
    }

    #[test]
    fn test_detect_microcompact_not_present() {
        let content = ToolResultContent::Text("normal output".into());
        assert!(detect_microcompact("tu_1", &content).is_none());
    }

    #[test]
    fn test_detect_content_replacement_in_result_microcompact() {
        let content = ToolResultContent::Text(
            "head\n\n[... 800 characters omitted (microcompacted) ...]\n\ntail".into(),
        );
        assert!(detect_content_replacement_in_result(&content));
    }

    #[test]
    fn test_build_context_snapshot() {
        let messages = vec![
            make_user_msg("hello"),
            make_assistant_with_tool_use("tu_1", "Bash"),
            make_tool_result_msg("tu_1", "output", false),
            make_assistant_msg("done"),
        ];

        let ctx = build_context_snapshot(&messages);
        assert!(ctx.estimated_total_tokens > 0);
        assert_eq!(ctx.context_window_size, 200_000);
        assert!(ctx.utilization_pct >= 0.0);
        assert_eq!(ctx.tool_use_count, 1);
        assert!(ctx.unique_tools_used.contains(&"Bash".to_string()));
        assert!(ctx.total_cost_usd > 0.0);
        assert_eq!(ctx.api_call_count, 2); // assistant with tool_use + assistant with text
    }

    #[test]
    fn test_build_transcript_data() {
        let messages = vec![
            make_user_msg("hello"),
            make_assistant_msg("hi"),
            make_compact_boundary(100000, 30000),
        ];

        let transcript = build_transcript_data(&messages);
        assert_eq!(transcript.message_count, 3);
        assert_eq!(transcript.user_message_count, 1);
        assert_eq!(transcript.assistant_message_count, 1);
        assert_eq!(transcript.system_message_count, 1);
        assert_eq!(transcript.messages.len(), 3);
    }

    #[test]
    fn test_export_dir() {
        let dir = get_export_dir();
        assert!(dir.to_string_lossy().contains("exports"));
    }

    #[test]
    fn test_format_ts_millis() {
        let ts = format_ts_millis(1700000000000);
        assert!(ts.contains("2023"));
    }
}
