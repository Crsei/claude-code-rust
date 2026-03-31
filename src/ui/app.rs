use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, cursor};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
#[allow(unused_imports)]
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};

use crate::types::message::Message;

use super::messages::{render_messages, total_rendered_lines};
use super::permissions::{PermissionChoice, PermissionDialog};
use super::prompt_input::PromptInput;
use super::spinner::SpinnerState;
use super::theme::Theme;

/// Actions produced by the app in response to user input.
///
/// The caller (event loop owner) should inspect these to drive side effects
/// such as sending a query to the API, aborting a stream, or quitting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// No externally-visible action required.
    None,
    /// The user submitted text input.
    Submit(String),
    /// The user pressed Ctrl+C while a stream is active (abort current query).
    Abort,
    /// The user wants to quit the application.
    Quit,
    /// Scroll the message history up.
    ScrollUp,
    /// Scroll the message history down.
    ScrollDown,
    /// The user made a permission choice.
    PermissionResponse(PermissionChoice),
}

/// Main TUI application state.
///
/// Owns the message history, input state, spinner, optional permission dialog
/// overlay, and rendering theme. The struct is designed to be driven by an
/// external event loop that calls [`handle_key_event`] and [`render`].
pub struct App {
    /// Conversation message history.
    messages: Vec<Message>,
    /// The text input widget.
    prompt: PromptInput,
    /// Vertical scroll offset (in rendered lines) for the message area.
    scroll_offset: usize,
    /// Whether an API stream is currently active.
    is_streaming: bool,
    /// Animated spinner shown during streaming / tool execution.
    spinner_state: SpinnerState,
    /// Optional permission dialog overlay.
    permission_dialog: Option<PermissionDialog>,
    /// Flag: the application should exit.
    should_quit: bool,
    /// The active color theme.
    theme: Theme,
    /// Cached total line count of the rendered message history (invalidated
    /// when messages change). `None` means the cache is stale.
    cached_total_lines: Option<usize>,
    /// Width used when computing `cached_total_lines`. If the terminal is
    /// resized the cache is invalidated.
    cached_width: u16,
}

impl App {
    /// Create a new `App` with default settings.
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
            cached_total_lines: None,
            cached_width: 0,
        }
    }

    // ── Public API ──────────────────────────────────────────────────

    /// Append a message to the conversation history and auto-scroll to the
    /// bottom.
    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        self.invalidate_line_cache();
        // Auto-scroll to bottom when a new message arrives.
        self.scroll_to_bottom_deferred();
    }

    /// Replace the last message (useful for streaming token-by-token updates).
    pub fn replace_last_message(&mut self, msg: Message) {
        if let Some(last) = self.messages.last_mut() {
            *last = msg;
        } else {
            self.messages.push(msg);
        }
        self.invalidate_line_cache();
        self.scroll_to_bottom_deferred();
    }

    /// Get the current messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Set the streaming flag and start / stop the spinner accordingly.
    pub fn set_streaming(&mut self, streaming: bool) {
        self.is_streaming = streaming;
        if streaming {
            self.spinner_state.start(Some("Thinking...".to_string()));
            self.prompt.is_active = false;
        } else {
            self.spinner_state.stop();
            self.prompt.is_active = true;
        }
    }

    /// Show a permission dialog overlay.
    pub fn show_permission_dialog(&mut self, tool_name: &str, input: &str, message: &str) {
        self.permission_dialog = Some(PermissionDialog::new(tool_name, input, message));
    }

    /// Dismiss the permission dialog (if any).
    pub fn dismiss_permission_dialog(&mut self) {
        self.permission_dialog = None;
    }

    /// Whether the application should quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Tick the spinner animation (call on a regular timer, e.g. every 80ms).
    pub fn tick(&mut self) {
        self.spinner_state.tick();
    }

    // ── Event handling ──────────────────────────────────────────────

    /// Process a key event and return an [`AppAction`] describing what the
    /// caller should do.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        // If a permission dialog is open, route keys there first.
        if let Some(ref mut dialog) = self.permission_dialog {
            if let Some(choice) = dialog.handle_key(key) {
                self.permission_dialog = None;
                return AppAction::PermissionResponse(choice);
            }
            return AppAction::None;
        }

        match (key.modifiers, key.code) {
            // ── Quit / Abort ────────────────────────────────────────
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                if self.is_streaming {
                    return AppAction::Abort;
                } else {
                    self.should_quit = true;
                    return AppAction::Quit;
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.should_quit = true;
                return AppAction::Quit;
            }

            // ── Scrolling ───────────────────────────────────────────
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

            // ── Mouse scroll (handled elsewhere) / default ─────────
            _ => {}
        }

        // Route to the prompt input widget.
        if let Some(submitted) = self.prompt.handle_key(key) {
            return AppAction::Submit(submitted);
        }

        AppAction::None
    }

    // ── Rendering ───────────────────────────────────────────────────

    /// Render the full application UI into the given frame.
    ///
    /// Layout:
    /// ```text
    /// ┌─────────────────────────┐
    /// │   Message history       │  (most of the screen)
    /// │   (scrollable)          │
    /// ├─────────────────────────┤
    /// │ > input prompt          │  (1–3 lines)
    /// └─────────────────────────┘
    /// ```
    /// An optional permission dialog is rendered as a centered overlay.
    pub fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        if size.width < 10 || size.height < 4 {
            return;
        }

        // Determine the height for the input area (prompt + optional spinner).
        let input_height = if self.is_streaming { 2u16 } else { 1u16 };
        let status_height = 1u16; // bottom status bar

        let chunks = Layout::vertical([
            Constraint::Min(1),                           // messages
            Constraint::Length(input_height + status_height), // input + status
        ])
        .split(size);

        let message_area = chunks[0];
        let bottom_area = chunks[1];

        // ── Messages ────────────────────────────────────────────────
        self.ensure_line_cache(message_area.width);
        let total = self.cached_total_lines.unwrap_or(0);
        // Clamp scroll so we never scroll past the end.
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
        );

        // ── Bottom area: spinner + input + status ───────────────────
        let bottom_chunks = Layout::vertical([
            Constraint::Length(if self.is_streaming { 1 } else { 0 }), // spinner
            Constraint::Length(1),                                      // input
            Constraint::Length(status_height),                          // status
        ])
        .split(bottom_area);

        if self.is_streaming && bottom_chunks[0].height > 0 {
            self.spinner_state
                .render(bottom_chunks[0], frame.buffer_mut(), &self.theme);
        }

        self.prompt
            .render(bottom_chunks[1], frame.buffer_mut(), &self.theme);

        // ── Status bar ──────────────────────────────────────────────
        self.render_status_bar(bottom_chunks[2], frame.buffer_mut());

        // ── Permission dialog overlay ───────────────────────────────
        if let Some(ref dialog) = self.permission_dialog {
            dialog.render(size, frame.buffer_mut(), &self.theme);
        }
    }

    // ── Running the event loop ──────────────────────────────────────

    /// Initialize the terminal, run the main event loop, and restore the
    /// terminal on exit.
    ///
    /// `on_action` is called for every non-`None` action. Return `true` from
    /// the callback to continue the loop, or `false` to quit.
    ///
    /// The spinner is ticked every ~80ms while waiting for events.
    pub fn run<F>(mut self, mut on_action: F) -> io::Result<()>
    where
        F: FnMut(&mut Self, AppAction) -> bool,
    {
        // Setup terminal.
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let tick_rate = Duration::from_millis(80);
        let mut last_tick = Instant::now();

        loop {
            // Draw.
            terminal.draw(|frame| self.render(frame))?;

            // Wait for an event or the tick timeout.
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    let action = self.handle_key_event(key);
                    if action != AppAction::None {
                        let should_continue = on_action(&mut self, action.clone());
                        if !should_continue || self.should_quit {
                            break;
                        }
                    }
                }
            }

            // Tick the spinner on each iteration.
            if last_tick.elapsed() >= tick_rate {
                self.tick();
                last_tick = Instant::now();
            }

            if self.should_quit {
                break;
            }
        }

        // Restore terminal.
        terminal::disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            cursor::Show
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    // ── Private helpers ─────────────────────────────────────────────

    fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
        // Clamping happens in render().
    }

    /// Mark the line-count cache as stale.
    fn invalidate_line_cache(&mut self) {
        self.cached_total_lines = None;
    }

    /// Recompute the line cache if it is stale or the width changed.
    fn ensure_line_cache(&mut self, width: u16) {
        if self.cached_total_lines.is_none() || self.cached_width != width {
            self.cached_total_lines =
                Some(total_rendered_lines(&self.messages, width as usize, &self.theme));
            self.cached_width = width;
        }
    }

    /// Request an auto-scroll to the bottom. The actual clamping happens
    /// during the next render pass (since we need the viewport height).
    fn scroll_to_bottom_deferred(&mut self) {
        // Set to a very large value; render() will clamp.
        self.scroll_offset = usize::MAX;
    }

    /// Render a thin status bar at the very bottom.
    fn render_status_bar(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }

        let msg_count = self.messages.len();
        let mode = if self.is_streaming {
            "streaming"
        } else {
            "ready"
        };
        let status_text = format!(" {} messages | {} | Ctrl+C to {}", msg_count, mode,
            if self.is_streaming { "abort" } else { "quit" });

        let line = Line::from(vec![Span::styled(status_text, self.theme.dim)]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
