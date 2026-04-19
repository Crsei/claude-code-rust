//! Transcript / focus view state machine (issue #12).
//!
//! The TUI cycles through three modes with `Ctrl+O`:
//!
//! 1. [`ViewMode::Prompt`] — the default REPL view with a live input.
//! 2. [`ViewMode::Transcript`] — a *less*-style read-only page through the
//!    conversation with incremental search and `e`-to-edit export.
//! 3. [`ViewMode::Focus`] — the same transcript body but with the spinner
//!    / status-bar chrome hidden, for screenshots / screen readers.
//!
//! The [`TranscriptState`] structure holds the extra state that only the
//! transcript/focus modes need (scroll offset, search query, match list,
//! current match index). Keeping it out of the main `App` struct means
//! prompt-mode frames are still cheap.
//!
//! # Rendering
//!
//! Transcript mode reuses [`super::messages::render_messages`] on a body
//! pane and adds a one-line header + one-line footer:
//!
//! ```text
//! ─── Transcript · 12 messages · search: "tool_use" (3/5) ─────
//!     (message body rendered by render_messages)
//!     ...
//! [Esc] prompt · [/] search · [n]/[N] next/prev · [e] editor · [q] quit
//! ```
//!
//! # Search
//!
//! Search is case-insensitive substring matching against the flattened
//! plaintext of each message. When a query is non-empty, matches are
//! recomputed lazily and the viewport snaps to the first hit. `n` / `N`
//! cycle through hits, wrapping at the ends.

use crate::types::message::{ContentBlock, Message, MessageContent, SystemSubtype};

/// The three view modes the TUI can be in. Cycling is `Prompt →
/// Transcript → Focus → Prompt` via `Ctrl+O`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Prompt,
    Transcript,
    Focus,
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::Prompt
    }
}

impl ViewMode {
    /// Advance to the next mode in the `Prompt → Transcript → Focus`
    /// cycle.
    pub fn next(self) -> Self {
        match self {
            ViewMode::Prompt => ViewMode::Transcript,
            ViewMode::Transcript => ViewMode::Focus,
            ViewMode::Focus => ViewMode::Prompt,
        }
    }

    /// True in any read-only / fullscreen mode.
    pub fn is_transcript_like(self) -> bool {
        matches!(self, ViewMode::Transcript | ViewMode::Focus)
    }

    /// Human-readable label — used in the transcript header.
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Prompt => "Prompt",
            ViewMode::Transcript => "Transcript",
            ViewMode::Focus => "Focus",
        }
    }
}

/// Whether the transcript is waiting for search input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptInputMode {
    /// Normal navigation: j/k, g/G, PgUp/PgDn, n/N, e, q, /.
    Normal,
    /// Typing a search query after `/`.
    Search,
}

impl Default for TranscriptInputMode {
    fn default() -> Self {
        TranscriptInputMode::Normal
    }
}

/// Per-match descriptor returned by [`search_messages`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// Index into the messages slice.
    pub message_index: usize,
}

/// All transient state needed for transcript/focus modes.
#[derive(Debug, Clone, Default)]
pub struct TranscriptState {
    /// Current scroll offset in global line space.
    pub scroll_offset: usize,
    /// Input mode — normal nav vs. typing a search.
    pub input_mode: TranscriptInputMode,
    /// Current search query. Empty when there's no active search.
    pub query: String,
    /// Committed match list (recomputed lazily when the query changes).
    pub matches: Vec<SearchMatch>,
    /// Index into [`Self::matches`] currently focused. `None` when no
    /// matches.
    pub focused: Option<usize>,
}

impl TranscriptState {
    /// Clear any search state. Called when leaving transcript mode or
    /// pressing `Esc` during search.
    pub fn clear_search(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.focused = None;
        self.input_mode = TranscriptInputMode::Normal;
    }

    /// Replace the match list after a query change and point `focused` at
    /// the first hit (or `None` if the new list is empty).
    pub fn set_matches(&mut self, matches: Vec<SearchMatch>) {
        self.focused = if matches.is_empty() { None } else { Some(0) };
        self.matches = matches;
    }

    /// Advance `focused` to the next match, wrapping at the end.
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            self.focused = None;
            return;
        }
        self.focused = Some(match self.focused {
            None => 0,
            Some(i) => (i + 1) % self.matches.len(),
        });
    }

    /// Move `focused` to the previous match, wrapping at the beginning.
    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            self.focused = None;
            return;
        }
        self.focused = Some(match self.focused {
            None => self.matches.len() - 1,
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
        });
    }

    /// Currently-focused match, if any.
    pub fn current_match(&self) -> Option<SearchMatch> {
        self.focused.and_then(|i| self.matches.get(i).copied())
    }
}

/// Flatten a message into plain text suitable for `contains`-style search.
/// Mirrors what the user sees in the terminal: we strip tool-use input
/// JSON down to the tool name because users almost never search for
/// bracket-laden JSON payloads.
pub fn message_plaintext(message: &Message) -> String {
    match message {
        Message::User(u) => match &u.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks_to_text(blocks),
        },
        Message::Assistant(a) => blocks_to_text(&a.content),
        Message::System(s) => match &s.subtype {
            SystemSubtype::LocalCommand { content } => content.clone(),
            SystemSubtype::Informational { .. }
            | SystemSubtype::Warning
            | SystemSubtype::CompactBoundary { .. }
            | SystemSubtype::ApiError { .. } => s.content.clone(),
        },
        // Non-conversation meta messages (progress/attachments) are
        // surfaced inline elsewhere; the transcript doesn't search or
        // export them.
        _ => String::new(),
    }
}

fn blocks_to_text(blocks: &[ContentBlock]) -> String {
    let mut out = String::new();
    for b in blocks {
        match b {
            ContentBlock::Text { text } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
            ContentBlock::Thinking { thinking, .. } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(thinking);
            }
            ContentBlock::ToolUse { name, .. } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str("tool_use: ");
                out.push_str(name);
            }
            ContentBlock::ToolResult { content, .. } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                match content {
                    crate::types::message::ToolResultContent::Text(t) => out.push_str(t),
                    crate::types::message::ToolResultContent::Blocks(inner) => {
                        out.push_str(&blocks_to_text(inner));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Search every message for `query` (case-insensitive substring).
/// Returns matches in message-index order; empty when the query is blank.
pub fn search_messages(messages: &[Message], query: &str) -> Vec<SearchMatch> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let needle = q.to_ascii_lowercase();
    let mut hits = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        let body = message_plaintext(m).to_ascii_lowercase();
        if body.contains(&needle) {
            hits.push(SearchMatch { message_index: i });
        }
    }
    hits
}

/// Render the entire conversation as plain-text markdown suitable for
/// dumping into `$EDITOR` (issue #12 acceptance criterion).
pub fn render_markdown_dump(messages: &[Message]) -> String {
    let mut out = String::new();
    out.push_str("# Conversation transcript\n\n");
    for (i, m) in messages.iter().enumerate() {
        match m {
            Message::User(_) => out.push_str(&format!("## [{}] User\n\n", i)),
            Message::Assistant(_) => out.push_str(&format!("## [{}] Assistant\n\n", i)),
            Message::System(_) => out.push_str(&format!("## [{}] System\n\n", i)),
            // Unexpected variants get a generic header so the dump stays
            // well-formed markdown; body fills in via `message_plaintext`.
            _ => out.push_str(&format!("## [{}] Other\n\n", i)),
        }
        let body = message_plaintext(m);
        if body.is_empty() {
            out.push_str("_(empty)_\n");
        } else {
            out.push_str(&body);
            if !body.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{
        AssistantMessage, InfoLevel, MessageContent, SystemMessage, SystemSubtype, UserMessage,
    };

    fn user(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".into(),
            content: MessageContent::Text(text.into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    fn assistant(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    fn system(level: InfoLevel, text: &str) -> Message {
        Message::System(SystemMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            subtype: SystemSubtype::Informational { level },
            content: text.into(),
        })
    }

    #[test]
    fn view_mode_cycles_through_three() {
        assert_eq!(ViewMode::Prompt.next(), ViewMode::Transcript);
        assert_eq!(ViewMode::Transcript.next(), ViewMode::Focus);
        assert_eq!(ViewMode::Focus.next(), ViewMode::Prompt);
    }

    #[test]
    fn is_transcript_like_excludes_prompt() {
        assert!(!ViewMode::Prompt.is_transcript_like());
        assert!(ViewMode::Transcript.is_transcript_like());
        assert!(ViewMode::Focus.is_transcript_like());
    }

    #[test]
    fn search_finds_case_insensitive_substrings() {
        let msgs = vec![
            user("What is the FIZZBUZZ algorithm?"),
            assistant("fizzbuzz prints Fizz for multiples of 3."),
            system(InfoLevel::Info, "Aborted by user"),
        ];
        let hits = search_messages(&msgs, "fizz");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].message_index, 0);
        assert_eq!(hits[1].message_index, 1);
    }

    #[test]
    fn search_empty_query_yields_no_hits() {
        let msgs = vec![user("anything")];
        assert!(search_messages(&msgs, "").is_empty());
        assert!(search_messages(&msgs, "   ").is_empty());
    }

    #[test]
    fn match_navigation_wraps_around() {
        let mut st = TranscriptState::default();
        st.set_matches(vec![
            SearchMatch { message_index: 0 },
            SearchMatch { message_index: 2 },
            SearchMatch { message_index: 5 },
        ]);
        assert_eq!(st.focused, Some(0));
        st.next_match();
        assert_eq!(st.focused, Some(1));
        st.next_match();
        st.next_match(); // wrap
        assert_eq!(st.focused, Some(0));
        st.prev_match(); // wrap back to end
        assert_eq!(st.focused, Some(2));
    }

    #[test]
    fn match_navigation_is_safe_with_empty_matches() {
        let mut st = TranscriptState::default();
        st.next_match();
        st.prev_match();
        assert_eq!(st.focused, None);
    }

    #[test]
    fn clear_search_resets_all_search_fields() {
        let mut st = TranscriptState::default();
        st.query = "query".into();
        st.set_matches(vec![SearchMatch { message_index: 0 }]);
        st.input_mode = TranscriptInputMode::Search;
        st.clear_search();
        assert_eq!(st.query, "");
        assert!(st.matches.is_empty());
        assert_eq!(st.focused, None);
        assert_eq!(st.input_mode, TranscriptInputMode::Normal);
    }

    #[test]
    fn markdown_dump_includes_every_role_and_body() {
        let msgs = vec![user("hello"), assistant("hi there")];
        let dump = render_markdown_dump(&msgs);
        assert!(dump.contains("# Conversation transcript"));
        assert!(dump.contains("[0] User"));
        assert!(dump.contains("hello"));
        assert!(dump.contains("[1] Assistant"));
        assert!(dump.contains("hi there"));
    }
}
