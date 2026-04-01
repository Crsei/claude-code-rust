//! Transcript recording.
//!
//! Provides an append-friendly write mechanism for recording the full
//! conversation transcript. Unlike session storage (which overwrites the
//! whole file), the transcript log appends newline-delimited JSON entries
//! so that partial conversations are preserved even on crash.

#![allow(unused)]

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;

use crate::types::message::Message;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single transcript entry written as one line of NDJSON.
#[derive(Debug, Serialize)]
struct TranscriptEntry {
    /// Unix timestamp (milliseconds).
    timestamp: i64,
    /// Session this entry belongs to.
    session_id: String,
    /// Message type tag (user, assistant, system, ...).
    msg_type: String,
    /// Message UUID.
    uuid: String,
    /// Condensed payload (text or tool use summary).
    payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the directory for transcript files (`~/.cc-rust/transcripts/`).
fn get_transcript_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".cc-rust").join("transcripts")
}

/// Return the transcript file path for a specific session.
fn get_transcript_file(session_id: &str) -> PathBuf {
    get_transcript_dir().join(format!("{}.ndjson", session_id))
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// Append the given messages to the transcript log for `session_id`.
///
/// Each message is written as a single line of JSON (NDJSON format) so that
/// the file can be tailed in real time and is resilient to partial writes.
pub fn record_transcript(session_id: &str, messages: &[Message]) -> Result<()> {
    let dir = get_transcript_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create transcript directory {}", dir.display()))?;

    let path = get_transcript_file(session_id);

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open transcript file {}", path.display()))?;

    let now = Utc::now().timestamp_millis();

    for msg in messages {
        let (msg_type, payload) = message_to_payload(msg);
        let entry = TranscriptEntry {
            timestamp: now,
            session_id: session_id.to_string(),
            msg_type,
            uuid: msg.uuid().to_string(),
            payload,
        };

        let line = serde_json::to_string(&entry)
            .context("Failed to serialize transcript entry")?;

        writeln!(file, "{}", line)
            .with_context(|| format!("Failed to write to transcript {}", path.display()))?;
    }

    Ok(())
}

/// Flush the transcript file for `session_id` by syncing to disk.
///
/// This is a no-op on most systems (the OS flushes on close), but provides
/// an explicit sync point for durability guarantees.
pub fn flush_transcript(session_id: &str) -> Result<()> {
    let path = get_transcript_file(session_id);
    if !path.exists() {
        return Ok(());
    }

    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .with_context(|| format!("Failed to open transcript for flush {}", path.display()))?;

    file.sync_all()
        .with_context(|| format!("Failed to sync transcript {}", path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a compact payload from a `Message` for transcript storage.
fn message_to_payload(msg: &Message) -> (String, serde_json::Value) {
    match msg {
        Message::User(u) => {
            let text = match &u.content {
                crate::types::message::MessageContent::Text(t) => t.clone(),
                crate::types::message::MessageContent::Blocks(blocks) => {
                    format!("[{} content blocks]", blocks.len())
                }
            };
            ("user".into(), serde_json::json!({ "text": text }))
        }
        Message::Assistant(a) => {
            let text_blocks: Vec<String> = a
                .content
                .iter()
                .filter_map(|block| match block {
                    crate::types::message::ContentBlock::Text { text } => Some(text.clone()),
                    crate::types::message::ContentBlock::ToolUse { name, .. } => {
                        Some(format!("[tool_use: {}]", name))
                    }
                    _ => None,
                })
                .collect();
            (
                "assistant".into(),
                serde_json::json!({
                    "content_summary": text_blocks,
                    "stop_reason": a.stop_reason,
                }),
            )
        }
        Message::System(s) => (
            "system".into(),
            serde_json::json!({ "content": s.content }),
        ),
        Message::Progress(p) => (
            "progress".into(),
            serde_json::json!({ "tool_use_id": p.tool_use_id }),
        ),
        Message::Attachment(a) => (
            "attachment".into(),
            serde_json::json!({ "attachment": a.attachment }),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcript_dir() {
        let dir = get_transcript_dir();
        assert!(dir.to_string_lossy().contains("transcripts"));
    }

    #[test]
    fn test_transcript_file_extension() {
        let path = get_transcript_file("test-session");
        assert!(path.to_string_lossy().ends_with(".ndjson"));
    }
}
