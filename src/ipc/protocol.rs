//! IPC protocol types for headless mode.
//!
//! Defines the JSON-lines protocol between the Rust backend and a separate
//! UI process (e.g. an Ink/React terminal frontend).
//!
//! Both enums use `#[serde(tag = "type")]` so each JSON line carries a
//! discriminator field, e.g. `{"type":"submit_prompt","text":"hello","id":"…"}`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Frontend → Backend (deserialized from stdin)
// ---------------------------------------------------------------------------

/// Messages sent by the UI process to the Rust backend.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrontendMessage {
    /// User submits a prompt.
    SubmitPrompt { text: String, id: String },
    /// User hits Ctrl+C to abort the current query.
    AbortQuery,
    /// User responds to a permission dialog.
    PermissionResponse {
        tool_use_id: String,
        /// One of "allow", "deny", "always_allow".
        decision: String,
    },
    /// User typed a slash command.
    SlashCommand { raw: String },
    /// Terminal was resized.
    Resize { cols: u16, rows: u16 },
    /// User wants to exit.
    Quit,
}

// ---------------------------------------------------------------------------
// Backend → Frontend (serialized to stdout)
// ---------------------------------------------------------------------------

/// Messages sent by the Rust backend to the UI process.
#[allow(dead_code)] // Protocol variants used by future phases (permission system, tool results, suggestions)
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendMessage {
    /// Backend has initialized and is ready to accept prompts.
    Ready {
        session_id: String,
        model: String,
        cwd: String,
    },
    /// Assistant started streaming a new content block.
    StreamStart { message_id: String },
    /// Streaming text delta for an in-progress content block.
    StreamDelta { message_id: String, text: String },
    /// Streaming for a content block has finished.
    StreamEnd { message_id: String },
    /// Final assistant message (content is the serialized Vec<ContentBlock>).
    AssistantMessage {
        id: String,
        content: serde_json::Value,
        cost_usd: f64,
    },
    /// A tool invocation.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Result of a tool invocation.
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    /// Ask the UI to show a permission dialog for a tool call.
    PermissionRequest {
        tool_use_id: String,
        tool: String,
        command: String,
        options: Vec<String>,
    },
    /// A system-level informational message.
    SystemInfo {
        text: String,
        /// One of "info", "warning", "error".
        level: String,
    },
    /// Token usage update.
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    },
    /// Prompt suggestions for the UI to display.
    Suggestions { items: Vec<String> },
    /// An error occurred.
    Error { message: String, recoverable: bool },
    /// A background agent has completed execution.
    BackgroundAgentComplete {
        agent_id: String,
        description: String,
        /// Truncated preview of the result (for UI display).
        result_preview: String,
        had_error: bool,
        duration_ms: u64,
    },

    /// Brief mode message from the model (via BriefTool).
    BriefMessage {
        message: String,
        status: String,
        attachments: Vec<String>,
    },

    /// Autonomous action started (proactive tick).
    AutonomousStart {
        source: String,
        time: String,
    },

    /// Push notification sent.
    NotificationSent {
        title: String,
        level: String,
    },
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Write a [`BackendMessage`] as a JSON line to stdout.
pub fn send_to_frontend(msg: &BackendMessage) -> std::io::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", json)?;
    stdout.flush()
}
