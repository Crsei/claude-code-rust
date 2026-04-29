//! IPC protocol types for headless mode.
//!
//! Defines the JSON-lines protocol between the Rust backend and a separate
//! UI process (e.g. an Ink/React terminal frontend).
//!
//! Both enums use `#[serde(tag = "type")]` so each JSON line carries a
//! discriminator field, e.g. `{"type":"submit_prompt","text":"hello","id":"…"}`.
//!
//! ## Module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `base.rs` | Shared supporting types (`ToolResultContentInfo`, `ConversationMessage`) |
//! | `subsystem.rs` | Subsystem protocol serde tests |
//! | `agent.rs` | Agent protocol serde tests |
//! | `team.rs` | Team protocol serde tests |

mod base;

mod agent;
mod subsystem;
mod team;

pub use base::{ConversationMessage, ToolResultContentInfo};

use serde::{Deserialize, Serialize};

use crate::types::plan_workflow::PlanWorkflowRecord;

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
    /// User responds to a [`BackendMessage::QuestionRequest`].
    QuestionResponse {
        /// Must match the `id` from the corresponding `QuestionRequest`.
        id: String,
        /// The user's answer text.
        text: String,
    },
    /// User wants to exit.
    Quit,

    // ── Subsystem commands ───────────────────────────────────────
    /// LSP lifecycle command.
    LspCommand {
        command: super::subsystem_events::LspCommand,
    },
    /// MCP lifecycle command.
    McpCommand {
        command: super::subsystem_events::McpCommand,
    },
    /// Plugin lifecycle command.
    PluginCommand {
        command: super::subsystem_events::PluginCommand,
    },
    /// Skill management command.
    SkillCommand {
        command: super::subsystem_events::SkillCommand,
    },
    /// IDE-integration lifecycle command.
    IdeCommand {
        command: super::subsystem_events::IdeCommand,
    },
    /// Agent-definition settings command (backs the `/agents` editor UI).
    AgentSettingsCommand {
        command: super::subsystem_events::AgentSettingsCommand,
    },
    /// Query all subsystem statuses.
    QuerySubsystemStatus,

    // ── Agent / Team commands ────────────────────────────────────
    /// Agent management commands.
    AgentCommand {
        command: super::agent_events::AgentCommand,
    },
    /// Team management commands.
    TeamCommand {
        command: super::agent_events::TeamCommand,
    },

    /// Run a bounded ripgrep-backed file search across the workspace.
    ///
    /// Frontend-facing counterpart of the upstream
    /// `ui/examples/upstream-patterns/src/utils/ripgrep.ts`
    /// helper — cc-rust's frontend has no direct filesystem access, so
    /// the backend runs `rg` on its behalf. The backend responds with a
    /// single [`BackendMessage::FileSearchResult`] keyed on
    /// `request_id`; long searches are truncated once
    /// `max_results` is reached.
    SearchFiles {
        request_id: String,
        pattern: String,
        /// Search root — defaults to the engine's cwd when `None`.
        #[serde(default)]
        cwd: Option<String>,
        /// Case-insensitive search. Defaults to `true` to match the
        /// upstream dialog's `-i` flag.
        #[serde(default = "default_true")]
        case_insensitive: bool,
        /// Upper bound on results returned in a single response. The
        /// handler caps at 500 regardless of this value to keep the
        /// payload reasonable.
        #[serde(default)]
        max_results: Option<usize>,
    },
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Backend → Frontend (serialized to stdout)
// ---------------------------------------------------------------------------

/// Messages sent by the Rust backend to the UI process.
#[allow(dead_code)]
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendMessage {
    // ── Core conversation ────────────────────────────────────────
    /// Backend has initialized and is ready to accept prompts.
    Ready {
        session_id: String,
        model: String,
        cwd: String,
        permission_mode: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        plan_workflow: Option<PlanWorkflowRecord>,
        #[serde(skip_serializing_if = "Option::is_none")]
        editor_mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        view_mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        keybindings: Option<serde_json::Value>,
    },
    /// Assistant started streaming a new content block.
    StreamStart { message_id: String },
    /// Streaming text delta for an in-progress content block.
    StreamDelta { message_id: String, text: String },
    /// Streaming thinking delta for an in-progress thinking block.
    ThinkingDelta {
        message_id: String,
        thinking: String,
    },
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
        /// Structured content blocks when the result includes non-text data
        /// (e.g. images from Computer Use screenshot).
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ToolResultContentInfo>>,
    },
    /// Intermediate progress report from a long-running tool invocation.
    ///
    /// Primarily emitted by the Bash tool so the frontend can render a
    /// live "Running… (12s)" indicator plus a tail of recent output.
    /// `output` holds the tail-capped snapshot the UI should display;
    /// `total_lines` / `total_bytes` are the full-stream counters so the
    /// UI can show `+N lines` / `~N lines` and total bytes; `timeout_ms`
    /// surfaces the configured timeout.
    ToolProgress {
        tool_use_id: String,
        /// Tool name (e.g. `"Bash"`) — lets UI-side routing pick the
        /// right progress renderer without a second lookup.
        tool: String,
        /// Tail-capped snapshot suitable for display.
        output: String,
        /// Whole-seconds elapsed since the tool started.
        elapsed_seconds: u64,
        /// Total output lines observed so far.
        #[serde(skip_serializing_if = "Option::is_none")]
        total_lines: Option<u64>,
        /// Total output bytes observed so far.
        #[serde(skip_serializing_if = "Option::is_none")]
        total_bytes: Option<u64>,
        /// Configured tool timeout in milliseconds (if any).
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Ask the UI to show a permission dialog for a tool call.
    PermissionRequest {
        tool_use_id: String,
        tool: String,
        command: String,
        options: Vec<String>,
    },
    /// Ask the user a question.  The frontend should display the question
    /// and send back a [`FrontendMessage::QuestionResponse`] with the same `id`.
    QuestionRequest {
        /// Unique question identifier.
        id: String,
        /// The question text to display.
        text: String,
    },
    /// Durable plan workflow state changed.
    PlanWorkflowEvent {
        event: String,
        summary: String,
        record: PlanWorkflowRecord,
    },
    /// A system-level informational message.
    SystemInfo {
        text: String,
        /// One of "info", "warning", "error".
        level: String,
    },
    /// Replace the full visible conversation history in the frontend.
    ConversationReplaced { messages: Vec<ConversationMessage> },
    /// Token usage update.
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    },
    /// Scriptable status-line snapshot (issue #11).
    ///
    /// `payload` is the same JSON the Rust TUI pipes to the user's
    /// `statusLine.command` — frontends that want to run their own script
    /// can use this. `lines` carries the already-rendered stdout (split
    /// on `\n`) so frontends that trust the Rust runner can skip spawning
    /// a second process. `error` is populated when the most recent run
    /// failed; in that case `lines` is typically empty and the frontend
    /// falls back to its built-in footer.
    StatusLineUpdate {
        /// Full payload — see `StatusLinePayload`.
        payload: serde_json::Value,
        /// Rendered stdout, one entry per line, trimmed to at most a few
        /// lines.
        #[serde(default)]
        lines: Vec<String>,
        /// Non-empty when the last run failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
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
    AutonomousStart { source: String, time: String },
    /// Push notification sent.
    NotificationSent { title: String, level: String },

    // ── Subsystem events ─────────────────────────────────────────
    /// LSP subsystem event.
    LspEvent {
        event: super::subsystem_events::LspEvent,
    },
    /// MCP subsystem event.
    McpEvent {
        event: super::subsystem_events::McpEvent,
    },
    /// Plugin subsystem event.
    PluginEvent {
        event: super::subsystem_events::PluginEvent,
    },
    /// Skill subsystem event.
    SkillEvent {
        event: super::subsystem_events::SkillEvent,
    },
    /// IDE-integration subsystem event.
    IdeEvent {
        event: super::subsystem_events::IdeEvent,
    },
    /// Agent-definition settings event.
    AgentSettingsEvent {
        event: super::subsystem_events::AgentSettingsEvent,
    },
    /// Aggregated subsystem status snapshot.
    SubsystemStatus {
        status: super::subsystem_types::SubsystemStatusSnapshot,
    },

    // ── Agent / Team events ──────────────────────────────────────
    /// Agent lifecycle + streaming events.
    AgentEvent {
        event: super::agent_events::AgentEvent,
    },
    /// Team events.
    TeamEvent {
        event: super::agent_events::TeamEvent,
    },

    /// Response to a [`FrontendMessage::SearchFiles`] request.
    ///
    /// `request_id` echoes the client's id so the frontend can correlate
    /// responses to in-flight requests. `truncated` is `true` when the
    /// handler hit the result cap before ripgrep finished — the
    /// frontend should surface a "+ more" indicator to the user.
    FileSearchResult {
        request_id: String,
        matches: Vec<FileSearchMatch>,
        truncated: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// A single file-search hit. Matches the upstream
/// `{ file, line, text }` shape from
/// `ui/examples/upstream-patterns/src/components/GlobalSearchDialog.tsx`.
#[derive(Serialize, Debug, Clone)]
pub struct FileSearchMatch {
    /// Path relative to the search root.
    pub file: String,
    /// 1-based line number.
    pub line: u64,
    /// Line text, trimmed of trailing whitespace.
    pub text: String,
}
