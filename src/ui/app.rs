use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::types::message::Message;

use super::messages::render_messages;
use super::permissions::{PermissionChoice, PermissionDialog};
use super::prompt_input::PromptInput;
use super::spinner::SpinnerState;
use super::theme::Theme;
use super::virtual_scroll::VirtualScroll;
use super::welcome;

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

    // ── Optimizations ──────────────────────────────────────────────
    /// Virtual scroll: per-message height cache + prefix-sum offsets.
    vscroll: VirtualScroll,
    /// Dirty flag — when false, the TUI skips `terminal.draw()`.
    dirty: bool,
    /// Tick counter for throttling spinner frame advances.
    tick_counter: u32,
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
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            vscroll: VirtualScroll::new(),
            dirty: true,
            tick_counter: 0,
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
        self.vscroll.invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn replace_last_message(&mut self, msg: Message) {
        if let Some(last) = self.messages.last_mut() {
            *last = msg;
        } else {
            self.messages.push(msg);
        }
        self.vscroll.invalidate_from(self.messages.len().saturating_sub(1));
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

    pub fn set_spinner_message(&mut self, msg: String) {
        self.spinner_state.set_message(msg);
        self.dirty = true;
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

        let input_height = if self.is_streaming { 2u16 } else { 1u16 };
        let status_height = 1u16;

        let chunks = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(input_height + status_height),
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
            self.vscroll.ensure_up_to_date(&self.messages, message_area.width, &self.theme);
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

        // ── Bottom area: spinner + input + status ───────────────────
        let bottom_chunks = Layout::vertical([
            Constraint::Length(if self.is_streaming { 1 } else { 0 }),
            Constraint::Length(1),
            Constraint::Length(status_height),
        ])
        .split(bottom_area);

        if self.is_streaming && bottom_chunks[0].height > 0 {
            self.spinner_state
                .render(bottom_chunks[0], frame.buffer_mut(), &self.theme);
        }

        self.prompt
            .render(bottom_chunks[1], frame.buffer_mut(), &self.theme);

        self.render_status_bar(bottom_chunks[2], frame.buffer_mut());

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

    fn render_status_bar(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }

        let msg_count = self.messages.len();
        let mode = if self.is_streaming { "streaming" } else { "ready" };

        let mut parts = Vec::new();
        if !self.model_name.is_empty() {
            let short_model = self.model_name
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
