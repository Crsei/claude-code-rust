use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::config::settings::StatusLineSettings;
use crate::services::prompt_suggestion::PromptSuggestion;
use crate::types::message::Message;

use super::messages::render_messages;
use super::permissions::{PermissionChoice, PermissionDialog};
use super::prompt_input::PromptInput;
use super::spinner::SpinnerState;
use super::status_line::{
    ContextWindowStatus, CostStatus, ModelInfo, StatusLinePayload, StatusLineRunner,
    WorkspaceStatus,
};
use super::theme::Theme;
use super::virtual_scroll::VirtualScroll;
use super::welcome;

/// Upper cap on the number of stdout lines the status-line runner is
/// allowed to take up. Arbitrary but small so a runaway script can't
/// eat the messages pane.
const STATUS_LINE_MAX_LINES: usize = 3;

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
}

/// Main TUI application state.
pub struct App {
    messages: Vec<Message>,
    prompt: PromptInput,
    scroll_offset: usize,
    is_streaming: bool,
    spinner_state: SpinnerState,
    permission_dialog: Option<PermissionDialog>,
    should_quit: bool,
    theme: Theme,
    model_name: String,
    session_id: String,
    cwd: String,
    session_cost_usd: f64,
    /// Whether the welcome screen is currently shown.
    show_welcome: bool,
    history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,

    // ── Prompt suggestions ───────────────────────────────────────────
    /// Next-prompt suggestions shown after an assistant turn completes.
    suggestions: Option<Vec<PromptSuggestion>>,

    // ── Optimizations ──────────────────────────────────────────────
    /// Virtual scroll: per-message height cache + prefix-sum offsets.
    vscroll: VirtualScroll,
    /// Dirty flag — when false, the TUI skips `terminal.draw()`.
    dirty: bool,
    /// Tick counter for throttling spinner frame advances.
    tick_counter: u32,

    // ── Scriptable status line (issue #11) ─────────────────────────
    /// Resolved status-line configuration (command, padding, intervals).
    /// Populated from effective settings at TUI startup; further updates
    /// go through [`Self::update_status_line_settings`] when the user
    /// reloads config.
    status_line_settings: StatusLineSettings,
    /// Subprocess runner — shared handle used by `/statusline` as well.
    status_line_runner: StatusLineRunner,
    /// Accumulated usage / cost for the current session (fed to the
    /// status-line payload). Updated from engine `Result` events.
    session_usage: SessionUsageSnapshot,
}

/// Subset of engine usage-tracking relevant to the status-line payload.
/// Populated by [`App::update_session_usage`].
#[derive(Debug, Clone, Default)]
struct SessionUsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub api_calls: u64,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            prompt: PromptInput::new(),
            scroll_offset: 0,
            is_streaming: false,
            spinner_state: SpinnerState::new(),
            permission_dialog: None,
            should_quit: false,
            theme: Theme::default(),
            model_name: String::new(),
            session_id: String::new(),
            cwd: String::new(),
            session_cost_usd: 0.0,
            show_welcome: true,
            suggestions: None,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            vscroll: VirtualScroll::new(),
            dirty: true,
            tick_counter: 0,
            status_line_settings: StatusLineSettings::default(),
            status_line_runner: StatusLineRunner::new(),
            session_usage: SessionUsageSnapshot::default(),
        }
    }

    // ── Dirty flag ─────────────────────────────────────────────────

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // ── Public API ──────────────────────────────────────────────────

    pub fn add_message(&mut self, msg: Message) {
        // Dismiss welcome screen on first user or assistant message.
        if self.show_welcome {
            if matches!(msg, Message::User(_) | Message::Assistant(_)) {
                self.show_welcome = false;
            }
        }
        self.messages.push(msg);
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
        self.vscroll
            .invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
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
        if self.spinner_state.active && self.tick_counter % 5 == 0 {
            self.spinner_state.tick();
            self.dirty = true;
        }
    }

    pub fn set_model_name(&mut self, name: String) {
        self.model_name = name;
        self.dirty = true;
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = id;
        self.dirty = true;
    }

    pub fn set_cwd(&mut self, cwd: String) {
        self.cwd = cwd;
        self.dirty = true;
    }

    pub fn update_session_cost(&mut self, cost_usd: f64) {
        self.session_cost_usd = cost_usd;
        self.dirty = true;
    }

    /// Update the accumulated session usage (tokens + api calls) used by
    /// the scriptable status-line payload.
    pub fn update_session_usage(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
        api_calls: u64,
    ) {
        self.session_usage = SessionUsageSnapshot {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            api_calls,
        };
        // No dirty flip — `update_session_cost` already ran and marked it.
    }

    /// Replace the resolved status-line settings (e.g. after `/statusline`
    /// edits the config).
    pub fn set_status_line_settings(&mut self, settings: StatusLineSettings) {
        self.status_line_settings = settings;
        // Drop any stale output so we fall back immediately if the user
        // disabled or cleared the command.
        if !self.status_line_settings.is_command_mode() {
            self.status_line_runner.reset();
        }
        self.dirty = true;
    }

    /// Shared handle to the status-line runner. `/statusline` calls this
    /// to inspect / reset the runner without owning the App.
    pub fn status_line_runner(&self) -> StatusLineRunner {
        self.status_line_runner.clone()
    }

    /// Adopt a pre-existing runner (e.g. the one stored on [`AppState`])
    /// so every UI surface and the `/statusline` command observe the same
    /// subprocess state.
    pub fn set_status_line_runner(&mut self, runner: StatusLineRunner) {
        self.status_line_runner = runner;
    }

    /// Build the current status-line payload from app state.
    fn build_status_payload(&self) -> StatusLinePayload {
        let mut p = StatusLinePayload::new();
        if !self.session_id.is_empty() {
            p.session_id = Some(self.session_id.clone());
        }
        if !self.model_name.is_empty() {
            let short = self
                .model_name
                .strip_prefix("claude-")
                .unwrap_or(&self.model_name);
            let short = short.split('-').take(2).collect::<Vec<_>>().join("-");
            p.model = Some(ModelInfo {
                id: self.model_name.clone(),
                display_name: Some(short),
                backend: None,
            });
        }
        if !self.cwd.is_empty() {
            p.workspace = Some(WorkspaceStatus {
                cwd: self.cwd.clone(),
                ..Default::default()
            });
        }
        // Context window — max_tokens currently unknown at this layer; the
        // IPC / daemon path fills it from the model registry.
        p.context = Some(ContextWindowStatus {
            input_tokens: self.session_usage.input_tokens,
            output_tokens: self.session_usage.output_tokens,
            cache_read_tokens: self.session_usage.cache_read_tokens,
            cache_creation_tokens: self.session_usage.cache_creation_tokens,
            max_tokens: None,
            used_fraction: None,
        });
        if self.session_cost_usd > 0.0 || self.session_usage.api_calls > 0 {
            p.cost = Some(CostStatus {
                total_usd: self.session_cost_usd,
                api_calls: self.session_usage.api_calls,
                session_duration_secs: None,
            });
        }
        p.streaming = self.is_streaming;
        p.message_count = self.messages.len();
        p
    }

    /// Kick the runner. Throttling / cancellation lives inside the runner.
    fn trigger_status_refresh(&self) {
        if !self.status_line_settings.is_command_mode() {
            return;
        }
        let payload = self.build_status_payload();
        let _ = self
            .status_line_runner
            .refresh(&self.status_line_settings, &payload);
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
        if self.history.last().map_or(true, |last| last != &text) {
            self.history.push(text);
        }
        self.history_index = None;
        self.saved_input.clear();
    }

    // ── Event handling ──────────────────────────────────────────────

    pub fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        if key.kind != KeyEventKind::Press {
            return AppAction::None;
        }

        // Any key press is likely to cause a visual change.
        self.dirty = true;

        if let Some(ref mut dialog) = self.permission_dialog {
            if let Some(choice) = dialog.handle_key(key) {
                self.permission_dialog = None;
                return AppAction::PermissionResponse(choice);
            }
            return AppAction::None;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                if self.is_streaming {
                    return AppAction::Abort;
                } else {
                    self.should_quit = true;
                    return AppAction::Quit;
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d'))
                if self.prompt.is_active || !self.is_streaming =>
            {
                self.should_quit = true;
                return AppAction::Quit;
            }

            (_, KeyCode::PageUp) | (KeyModifiers::SHIFT, KeyCode::Up) => {
                self.scroll_up(5);
                return AppAction::ScrollUp;
            }
            (_, KeyCode::PageDown) | (KeyModifiers::SHIFT, KeyCode::Down) => {
                self.scroll_down(5);
                return AppAction::ScrollDown;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) if !self.prompt.is_active => {
                self.scroll_up(10);
                return AppAction::ScrollUp;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d'))
                if !self.prompt.is_active && self.is_streaming =>
            {
                self.scroll_down(10);
                return AppAction::ScrollDown;
            }

            (_, KeyCode::Up) if self.prompt.is_active && !self.is_streaming => {
                self.history_up();
                return AppAction::None;
            }
            (_, KeyCode::Down) if self.prompt.is_active && !self.is_streaming => {
                self.history_down();
                return AppAction::None;
            }

            _ => {}
        }

        if let Some(submitted) = self.prompt.handle_key(key) {
            return AppAction::Submit(submitted);
        }

        AppAction::None
    }

    // ── Rendering ───────────────────────────────────────────────────

    pub fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        if size.width < 10 || size.height < 4 {
            return;
        }

        // Kick the status-line runner before computing layout so this
        // frame already has a chance to show a refreshed output. The
        // runner throttles refreshes internally.
        self.trigger_status_refresh();
        let status_output = self.status_line_runner.latest();
        let custom_lines: Vec<String> = if status_output.is_usable()
            && self.status_line_settings.is_command_mode()
        {
            status_output.lines(STATUS_LINE_MAX_LINES)
        } else {
            Vec::new()
        };

        let spinner_height = if self.is_streaming { 1u16 } else { 0 };
        let suggestion_height = if !self.is_streaming && self.suggestions.is_some() {
            1u16
        } else {
            0
        };
        let input_height = 1u16;
        let status_height = if custom_lines.is_empty() {
            1u16
        } else {
            custom_lines.len().min(STATUS_LINE_MAX_LINES) as u16
        };
        let bottom_height = spinner_height + suggestion_height + input_height + status_height;
        let min_message_height = if self.show_welcome {
            welcome::welcome_height()
        } else {
            1
        };

        let chunks = Layout::vertical([
            Constraint::Min(min_message_height),
            Constraint::Length(bottom_height),
        ])
        .split(size);

        let message_area = chunks[0];
        let bottom_area = chunks[1];

        if self.show_welcome {
            // ── Welcome screen ──────────────────────────────────────
            welcome::render_welcome(
                message_area,
                frame.buffer_mut(),
                env!("CARGO_PKG_VERSION"),
                &self.model_name,
                &self.session_id,
                &self.cwd,
            );
        } else {
            // ── Messages (virtual scroll) ───────────────────────────
            self.vscroll
                .ensure_up_to_date(&self.messages, message_area.width, &self.theme);
            let total = self.vscroll.total_lines();
            let max_scroll = total.saturating_sub(message_area.height as usize);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }

            render_messages(
                &self.messages,
                message_area,
                frame.buffer_mut(),
                &self.theme,
                self.scroll_offset,
                &self.vscroll,
            );
        }

        // ── Bottom area: spinner + suggestions + input + status ──────
        let has_suggestions = !self.is_streaming && self.suggestions.is_some();
        let bottom_chunks = Layout::vertical([
            Constraint::Length(if self.is_streaming { 1 } else { 0 }),
            Constraint::Length(if has_suggestions { 1 } else { 0 }),
            Constraint::Length(1),
            Constraint::Length(status_height),
        ])
        .split(bottom_area);

        if self.is_streaming && bottom_chunks[0].height > 0 {
            self.spinner_state
                .render(bottom_chunks[0], frame.buffer_mut(), &self.theme);
        }

        if has_suggestions {
            self.render_suggestions(bottom_chunks[1], frame.buffer_mut());
        }

        self.prompt
            .render(bottom_chunks[2], frame.buffer_mut(), &self.theme);

        self.render_status_bar(bottom_chunks[3], frame.buffer_mut(), &custom_lines);

        if let Some(ref dialog) = self.permission_dialog {
            dialog.render(size, frame.buffer_mut(), &self.theme);
        }
    }

    // ── Private helpers ─────────────────────────────────────────────

    fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    fn scroll_to_bottom_deferred(&mut self) {
        self.scroll_offset = usize::MAX;
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.saved_input = self.prompt.input.clone();
            self.history_index = Some(self.history.len() - 1);
        } else if let Some(idx) = self.history_index {
            if idx > 0 {
                self.history_index = Some(idx - 1);
            } else {
                return;
            }
        }
        if let Some(idx) = self.history_index {
            self.prompt.input = self.history[idx].clone();
            self.prompt.cursor_position = self.prompt.input.len();
        }
    }

    fn history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx < self.history.len() - 1 {
                self.history_index = Some(idx + 1);
                self.prompt.input = self.history[idx + 1].clone();
                self.prompt.cursor_position = self.prompt.input.len();
            } else {
                self.history_index = None;
                self.prompt.input = self.saved_input.clone();
                self.prompt.cursor_position = self.prompt.input.len();
            }
        }
    }

    fn render_suggestions(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }
        if let Some(suggestions) = &self.suggestions {
            let hint: String = suggestions
                .iter()
                .take(3)
                .enumerate()
                .map(|(i, s)| format!("[{}{}] {}", s.category.icon(), i + 1, s.text))
                .collect::<Vec<_>>()
                .join("  ");
            let line = Line::from(Span::styled(
                hint,
                ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
            ));
            buf.set_line(area.x, area.y, &line, area.width);
        }
    }

    fn render_status_bar(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        custom_lines: &[String],
    ) {
        if area.height == 0 {
            return;
        }

        // 1. Custom scriptable status-line (issue #11) — when present, take
        //    full priority over the built-in footer. Padding from settings.
        if !custom_lines.is_empty() {
            let padding = self.status_line_settings.padding.unwrap_or(0) as usize;
            let pad_str: String = " ".repeat(padding);
            for (i, text) in custom_lines.iter().enumerate() {
                if (i as u16) >= area.height {
                    break;
                }
                let line = Line::from(vec![Span::styled(
                    format!("{}{}", pad_str, text),
                    self.theme.dim,
                )]);
                buf.set_line(area.x, area.y + i as u16, &line, area.width);
            }
            return;
        }

        // 2. Built-in default footer — also the fallback when the runner
        //    errors or the script is disabled.
        let msg_count = self.messages.len();
        let mode = if self.is_streaming {
            "streaming"
        } else {
            "ready"
        };

        let mut parts = Vec::new();
        if !self.model_name.is_empty() {
            let short_model = self
                .model_name
                .strip_prefix("claude-")
                .unwrap_or(&self.model_name);
            let short_model = short_model.split('-').take(2).collect::<Vec<_>>().join("-");
            parts.push(short_model);
        }
        parts.push(format!("{} msgs", msg_count));
        if self.session_cost_usd > 0.0 {
            parts.push(format!("${:.4}", self.session_cost_usd));
        }
        parts.push(mode.to_string());
        parts.push(format!(
            "Ctrl+C {}",
            if self.is_streaming { "abort" } else { "quit" }
        ));

        // If the runner reported an error, surface a quiet marker so the
        // user knows to run `/statusline status` to see why.
        let latest = self.status_line_runner.latest();
        if latest.error.is_some() && self.status_line_settings.is_command_mode() {
            parts.push("statusline:err".to_string());
        }

        let status_text = format!(" {}", parts.join(" | "));
        let line = Line::from(vec![Span::styled(status_text, self.theme.dim)]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
