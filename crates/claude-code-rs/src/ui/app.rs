#[allow(dead_code)]
#[path = "app/agent_navigation.rs"]
pub mod agent_navigation;
#[allow(dead_code)]
#[path = "app/app_backtrack.rs"]
pub mod app_backtrack;
#[allow(dead_code)]
#[path = "app/app_command.rs"]
pub mod app_command;
#[allow(dead_code)]
#[path = "app/app_event.rs"]
pub mod app_event;
#[allow(dead_code)]
#[path = "app/app_event_sender.rs"]
pub mod app_event_sender;
#[allow(dead_code)]
#[path = "app/app_server_adapter.rs"]
pub mod app_server_adapter;
#[allow(dead_code)]
#[path = "app/app_server_requests.rs"]
pub mod app_server_requests;
#[path = "app/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "app/loaded_threads.rs"]
pub mod loaded_threads;
#[path = "app/render.rs"]
mod render;
#[path = "app/status.rs"]
pub mod status;
#[cfg(test)]
#[path = "app/tests.rs"]
mod tests;
#[path = "app/transcript_mode.rs"]
mod transcript_mode;
#[path = "app/voice.rs"]
mod voice;
#[path = "app/workspace_trust.rs"]
mod workspace_trust;
use crate::config::settings::StatusLineSettings;
use crate::keybindings::KeybindingRegistry;
use crate::services::prompt_suggestion::PromptSuggestion;
use crate::types::message::Message;
use crate::voice::VoiceController;
use status::SessionUsageSnapshot;
use workspace_trust::is_workspace_trusted;

use super::command_palette::CommandPalette;
use super::command_surface::CommandSurface;
use super::history_search_dialog::{HistorySearchDialog, HistorySearchEntry};
use super::permissions::{PermissionChoice, PermissionDialog};
use super::prompt_input::PromptInput;
use super::spinner::SpinnerState;
use super::status_line::StatusLineRunner;
use super::terminal_env::TerminalEnvConfig;
use super::theme::Theme;
use super::transcript::{TranscriptState, ViewMode};
use super::vim::VimState;
use super::virtual_scroll::VirtualScroll;

/// Actions produced by the app in response to user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    None,
    Submit(String),
    Abort,
    Quit,
    ScrollUp,
    ScrollDown,
    PermissionResponse(PermissionChoice),
    LspRecommendationResponse {
        request_id: String,
        plugin_name: String,
        decision: String,
    },
    /// Transcript mode requested an export to `$EDITOR`. Carries the
    /// pre-rendered markdown body; the caller writes it to disk and
    /// spawns the editor so `App` stays free of IO.
    ExportTranscript(String),
    CopyMessage(String),
}

/// Main TUI application state.
pub struct App {
    messages: Vec<Message>,
    selected_message: Option<usize>,
    selected_message_expanded: bool,
    prompt: PromptInput,
    scroll_offset: usize,
    is_streaming: bool,
    spinner_state: SpinnerState,
    permission_dialog: Option<PermissionDialog>,
    should_quit: bool,
    theme: Theme,
    model_name: String,
    backend_name: String,
    session_id: String,
    cwd: String,
    output_style: Option<String>,
    permission_mode_label: String,
    sandbox_label: String,
    effort_label: Option<String>,
    remote_indicator_label: Option<String>,
    session_cost_usd: f64,
    /// Whether the welcome screen is currently shown.
    show_welcome: bool,
    /// Startup trust gate shown before the normal welcome panel.
    workspace_trust_pending: bool,
    workspace_trust_selection: usize,
    history: Vec<HistorySearchEntry>,
    history_index: Option<usize>,
    saved_input: String,
    command_palette: CommandPalette,
    command_surface: Option<CommandSurface>,
    history_search_dialog: Option<HistorySearchDialog>,

    // Prompt suggestions
    /// Next-prompt suggestions shown after an assistant turn completes.
    suggestions: Option<Vec<PromptSuggestion>>,

    // Optimizations
    /// Virtual scroll: per-message height cache + prefix-sum offsets.
    vscroll: VirtualScroll,
    /// Dirty flag; when false, the TUI skips `terminal.draw()`.
    dirty: bool,
    /// Tick counter for throttling spinner frame advances.
    tick_counter: u32,
    keybindings: KeybindingRegistry,
    pending_chord: Vec<crate::keybindings::keystroke::Keystroke>,
    vim: VimState,

    // Scriptable status line (issue #11)
    /// Resolved status-line configuration (command, padding, intervals).
    /// Populated from effective settings at TUI startup; further updates
    /// go through [`Self::update_status_line_settings`] when the user
    /// reloads config.
    status_line_settings: StatusLineSettings,
    /// Subprocess runner shared handle used by `/statusline` as well.
    status_line_runner: StatusLineRunner,
    /// Accumulated usage / cost for the current session (fed to the
    /// status-line payload). Updated from engine `Result` events.
    session_usage: SessionUsageSnapshot,

    // Transcript / focus view + terminal env (issue #12)
    /// Which view the user is currently in; cycled with `Ctrl+O`.
    view_mode: ViewMode,
    /// Extra state only used in transcript/focus modes.
    transcript_state: TranscriptState,
    /// Env-driven terminal config: `CLAUDE_CODE_NO_FLICKER`,
    /// `CLAUDE_CODE_ENABLE_MOUSE_CAPTURE`, `CLAUDE_CODE_DISABLE_MOUSE`,
    /// `CLAUDE_CODE_SCROLL_SPEED`.
    terminal_env: TerminalEnvConfig,

    // Voice dictation (issue #13)
    /// Push-to-talk controller. `None` until the TUI runner installs
    /// one via [`Self::set_voice_controller`]. Default construction
    /// (e.g. in tests) leaves it `None` so nothing accidentally records.
    voice: Option<VoiceController>,
    /// Snapshot of the effective `voiceEnabled` + `language` settings.
    /// Updated whenever `/voice` / `/config set` flips them so the
    /// push-to-talk key handler doesn't need to re-read AppState.
    voice_enabled: bool,
    /// Whether this build/runtime supports actual recording and STT.
    voice_supported: bool,
    /// Normalized STT language passed to the controller on press.
    voice_language: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            selected_message: None,
            selected_message_expanded: false,
            prompt: PromptInput::new(),
            scroll_offset: 0,
            is_streaming: false,
            spinner_state: SpinnerState::new(),
            permission_dialog: None,
            should_quit: false,
            theme: Theme::default(),
            model_name: String::new(),
            backend_name: String::new(),
            session_id: String::new(),
            cwd: String::new(),
            output_style: None,
            permission_mode_label: String::new(),
            sandbox_label: String::new(),
            effort_label: None,
            remote_indicator_label: None,
            session_cost_usd: 0.0,
            show_welcome: true,
            workspace_trust_pending: false,
            workspace_trust_selection: 0,
            suggestions: None,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            command_palette: CommandPalette::new(),
            command_surface: None,
            history_search_dialog: None,
            vscroll: VirtualScroll::new(),
            dirty: true,
            tick_counter: 0,
            keybindings: KeybindingRegistry::with_defaults(),
            pending_chord: Vec::new(),
            vim: VimState::new(),
            status_line_settings: StatusLineSettings::default(),
            status_line_runner: StatusLineRunner::new(),
            session_usage: SessionUsageSnapshot::default(),
            view_mode: ViewMode::default(),
            transcript_state: TranscriptState::default(),
            terminal_env: TerminalEnvConfig::default(),
            voice: None,
            voice_enabled: false,
            voice_supported: false,
            voice_language: "en".to_string(),
        }
    }

    // Dirty flag

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // Public API

    pub fn add_message(&mut self, msg: Message) {
        // Dismiss welcome screen on first user or assistant message.
        if self.show_welcome && matches!(msg, Message::User(_) | Message::Assistant(_)) {
            self.show_welcome = false;
        }
        self.messages.push(msg);
        self.clamp_selected_message();
        self.vscroll
            .invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn replace_last_message(&mut self, msg: Message) {
        if let Some(last) = self.messages.last_mut() {
            *last = msg;
        } else {
            self.messages.push(msg);
        }
        self.clamp_selected_message();
        self.vscroll
            .invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn remove_last_message(&mut self) {
        if self.messages.pop().is_some() {
            self.clamp_selected_message();
            self.vscroll.invalidate_from(self.messages.len());
            self.scroll_to_bottom_deferred();
            self.dirty = true;
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn selected_message(&self) -> Option<usize> {
        self.selected_message
    }

    fn clamp_selected_message(&mut self) {
        if self.messages.is_empty() {
            self.selected_message = None;
            self.selected_message_expanded = false;
        } else if let Some(idx) = self.selected_message {
            self.selected_message = Some(idx.min(self.messages.len() - 1));
        }
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.selected_message = None;
        self.selected_message_expanded = false;
        self.scroll_offset = 0;
        self.vscroll.invalidate_all();
        self.dirty = true;
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        if self.is_streaming != streaming {
            self.is_streaming = streaming;
            if streaming {
                self.spinner_state.start(Some("Thinking...".to_string()));
                self.prompt.is_active = false;
                self.suggestions = None; // clear stale suggestions
            } else {
                self.spinner_state.stop();
                self.prompt.is_active = true;
            }
            self.dirty = true;
        }
    }

    pub fn show_permission_dialog(&mut self, tool_name: &str, input: &str, message: &str) {
        let web_fetch_subject;
        let web_fetch_message;
        let (input, message) = if tool_name.eq_ignore_ascii_case("WebFetch") {
            let url = permission_subject(input, message);
            web_fetch_message =
                crate::ui::permissions::web_fetch_permission_request::web_fetch_permission_request::render_web_fetch_permission_request(
                    &url,
                    "GET",
                    0,
                );
            web_fetch_subject = url;
            (web_fetch_subject.as_str(), web_fetch_message.as_str())
        } else {
            (input, message)
        };
        self.permission_dialog = Some(PermissionDialog::new(tool_name, input, message));
        self.dirty = true;
    }

    pub fn dismiss_permission_dialog(&mut self) {
        self.permission_dialog = None;
        self.dirty = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Tick the spinner. Called at 16ms interval; spinner frame advances
    /// every 5th tick (~80ms) to keep a pleasant animation speed.
    pub fn tick(&mut self) {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        if self.spinner_state.active && self.tick_counter.is_multiple_of(5) {
            self.spinner_state.tick();
            self.dirty = true;
        }
    }

    pub fn set_model_name(&mut self, name: String) {
        self.model_name = name;
        self.dirty = true;
    }

    pub fn set_backend_name(&mut self, name: String) {
        self.backend_name = name;
        self.dirty = true;
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = id;
        self.dirty = true;
    }

    pub fn set_cwd(&mut self, cwd: String) {
        self.cwd = cwd;
        self.workspace_trust_pending = !self.cwd.is_empty() && !is_workspace_trusted(&self.cwd);
        self.workspace_trust_selection = 0;
        self.dirty = true;
    }

    pub fn set_output_style(&mut self, output_style: Option<String>) {
        self.output_style = output_style;
        self.dirty = true;
    }

    pub fn set_keybindings(&mut self, keybindings: KeybindingRegistry) {
        self.keybindings = keybindings;
        self.pending_chord.clear();
    }

    pub fn set_editor_mode(&mut self, editor_mode: Option<&str>) {
        self.vim = VimState::from_editor_mode(editor_mode);
        self.dirty = true;
    }

    pub fn update_session_cost(&mut self, cost_usd: f64) {
        self.session_cost_usd = cost_usd;
        self.dirty = true;
    }

    // Terminal env + transcript (issue #12)

    /// Install a resolved terminal-env config. Called by the TUI runner
    /// once at startup; `ScrollSpeed` is then consumed by the scroll
    /// helpers in both prompt and transcript modes.
    pub fn set_terminal_env(&mut self, cfg: TerminalEnvConfig) {
        self.terminal_env = cfg;
        self.dirty = true;
    }

    /// Current terminal-env config (read by the TUI runner to decide
    /// whether to emit synchronized-update escapes).
    pub fn terminal_env(&self) -> TerminalEnvConfig {
        self.terminal_env
    }

    /// Current view mode; tests and the TUI key binding use this to
    /// verify Ctrl+O cycling.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Advance the view mode (`Prompt -> Transcript -> Focus -> Prompt`).
    /// Exposed separately so `/` slash commands could also drive it in
    /// the future.
    pub fn cycle_view_mode(&mut self) {
        // Leaving transcript throws away search state; entering fresh
        // next time is less surprising than stale matches hanging around.
        if self.view_mode.is_transcript_like() {
            self.transcript_state.clear_search();
        }
        self.view_mode = self.view_mode.next();
        // Seed the transcript scroll offset to the bottom on entry so the
        // user starts reading the latest exchange.
        if self.view_mode.is_transcript_like() {
            self.transcript_state.scroll_offset = usize::MAX;
        }
        self.dirty = true;
    }

    /// Open a modal slash-command surface above the normal prompt.
    pub fn open_command_surface(&mut self, surface: CommandSurface) {
        self.command_surface = Some(surface);
        self.command_palette.close();
        self.dirty = true;
    }

    pub fn command_surface_active(&self) -> bool {
        self.command_surface.is_some()
    }

    pub fn history_search_active(&self) -> bool {
        self.history_search_dialog.is_some()
    }

    /// Current transcript state exposed read-only so tests can assert
    /// search invariants without going through the render path.
    pub fn transcript_state(&self) -> &TranscriptState {
        &self.transcript_state
    }

    pub fn set_spinner_message(&mut self, msg: String) {
        self.spinner_state.set_message(msg);
        self.dirty = true;
    }

    pub fn set_suggestions(&mut self, suggestions: Vec<PromptSuggestion>) {
        self.suggestions = Some(suggestions);
        self.dirty = true;
    }

    pub fn clear_suggestions(&mut self) {
        if self.suggestions.is_some() {
            self.suggestions = None;
            self.dirty = true;
        }
    }

    pub fn suggestions(&self) -> Option<&[PromptSuggestion]> {
        self.suggestions.as_deref()
    }

    pub fn push_history(&mut self, text: String) {
        if self.history.last().map(|entry| entry.display.as_str()) != Some(text.as_str()) {
            self.history
                .push(HistorySearchEntry::new(text, current_unix_secs()));
        }
        self.history_index = None;
        self.saved_input.clear();
    }

    /// Seed Ctrl+R prompt history from backend session storage.
    ///
    /// Callers pass entries newest-first (the shape returned by the session
    /// reader). `App` keeps history oldest-first so arrow-history and Ctrl+R
    /// continue to prefer live prompts appended during this TUI run.
    pub fn seed_persistent_history(&mut self, entries: Vec<HistorySearchEntry>) {
        for entry in entries.into_iter().rev() {
            if self
                .history
                .iter()
                .any(|existing| existing.display == entry.display)
            {
                continue;
            }
            self.history.push(entry);
        }
        self.history_index = None;
        self.dirty = true;
    }

    #[cfg(test)]
    pub(crate) fn history_len(&self) -> usize {
        self.history.len()
    }

    // Event handling
}

fn permission_subject(input: &str, message: &str) -> String {
    let source = if input.trim().is_empty() {
        message
    } else {
        input
    };
    source
        .split_whitespace()
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .unwrap_or(source)
        .trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ',' || ch == ')' || ch == '(')
        .to_string()
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn current_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
