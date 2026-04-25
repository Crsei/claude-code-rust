use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::theme::Theme;

/// A single-line text input widget with cursor support.
///
/// Handles common editing key bindings (arrows, home/end, ctrl shortcuts)
/// and returns `Some(text)` from [`handle_key`] when the user presses Enter.
pub struct PromptInput {
    /// Current input text.
    pub input: String,
    /// Byte-offset cursor position within `input`.
    ///
    /// Always kept on a char boundary.
    pub cursor_position: usize,
    /// Whether this widget is focused / accepting input.
    pub is_active: bool,
}

impl PromptInput {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            is_active: true,
        }
    }

    /// Handle a key event. Returns `Some(submitted_text)` when the user
    /// presses Enter with a non-empty input, clearing the internal buffer.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if !self.is_active {
            return None;
        }

        match (key.modifiers, key.code) {
            // ── Submit ──────────────────────────────────────────────
            (KeyModifiers::NONE, KeyCode::Enter) | (KeyModifiers::SHIFT, KeyCode::Enter) => {
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    return None;
                }
                self.input.clear();
                self.cursor_position = 0;
                return Some(text);
            }

            // ── Ctrl shortcuts ──────────────────────────────────────
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                // Clear entire line
                self.input.clear();
                self.cursor_position = 0;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                // Move to start of line (select-all semantics are tricky in
                // a terminal; we just move the cursor to the beginning).
                self.cursor_position = 0;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                // Move to end of line
                self.cursor_position = self.input.len();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                // Delete word backwards
                self.delete_word_backwards();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                // Kill to end of line
                self.input.truncate(self.cursor_position);
            }

            // ── Navigation ──────────────────────────────────────────
            (_, KeyCode::Left) => {
                self.move_cursor_left();
            }
            (_, KeyCode::Right) => {
                self.move_cursor_right();
            }
            (_, KeyCode::Home) => {
                self.cursor_position = 0;
            }
            (_, KeyCode::End) => {
                self.cursor_position = self.input.len();
            }

            // ── Deletion ────────────────────────────────────────────
            (_, KeyCode::Backspace) => {
                if self.cursor_position > 0 {
                    // Find the previous char boundary
                    let prev = self.prev_char_boundary();
                    self.input.drain(prev..self.cursor_position);
                    self.cursor_position = prev;
                }
            }
            (_, KeyCode::Delete) => {
                if self.cursor_position < self.input.len() {
                    let next = self.next_char_boundary();
                    self.input.drain(self.cursor_position..next);
                }
            }

            // ── Character input ─────────────────────────────────────
            (_, KeyCode::Char(c)) => {
                self.input.insert(self.cursor_position, c);
                self.cursor_position += c.len_utf8();
            }

            _ => {}
        }

        None
    }

    /// Insert `text` at the current cursor position and advance the
    /// cursor past it. Used by voice dictation (issue #13) so
    /// transcribed text lands wherever the user was typing instead of
    /// being appended at the end.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.input.insert_str(self.cursor_position, text);
        self.cursor_position += text.len();
    }

    /// Render the prompt input widget.
    ///
    /// Shows a "> " prompt prefix followed by the input text with a visible
    /// cursor indicator. The visible window scrolls horizontally when the
    /// cursor would move off-screen.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.render_with_hint(area, buf, theme, None);
    }

    /// Render the prompt input widget with a dim inline hint after the text.
    pub fn render_with_hint(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hint: Option<&str>,
    ) {
        if area.height == 0 || area.width < 4 {
            return;
        }

        let prompt_str = "> ";
        let prompt_span = Span::styled(prompt_str, theme.prompt);
        let prompt_width = 2u16; // "> " is always 2 columns

        let available_width = (area.width.saturating_sub(prompt_width)) as usize;

        // Compute the visible window of the input text. We track the cursor
        // as a *character* offset for display purposes.
        let char_cursor = self.input[..self.cursor_position].chars().count();
        let input_chars: Vec<char> = self.input.chars().collect();

        // Determine scroll offset so the cursor is always visible.
        let scroll = if char_cursor >= available_width {
            char_cursor - available_width + 1
        } else {
            0
        };

        let visible_end = (scroll + available_width).min(input_chars.len());
        let visible_text: String = input_chars[scroll..visible_end].iter().collect();

        // Build the cursor position within the visible region.
        let cursor_in_visible = char_cursor.saturating_sub(scroll);

        // Split visible text around the cursor to insert styling.
        let before_cursor: String = visible_text.chars().take(cursor_in_visible).collect();
        let cursor_char: String = visible_text
            .chars()
            .nth(cursor_in_visible)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string());
        let after_cursor: String = visible_text.chars().skip(cursor_in_visible + 1).collect();

        let mut spans = vec![prompt_span];

        if self.is_active {
            spans.push(Span::raw(before_cursor));
            spans.push(Span::styled(
                cursor_char,
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Black)
                    .bg(ratatui::style::Color::White),
            ));
            spans.push(Span::raw(after_cursor));
            if let Some(hint) = hint.filter(|_| cursor_in_visible >= visible_text.len()) {
                spans.push(Span::styled(format!(" {hint}"), theme.dim));
            }
        } else {
            spans.push(Span::styled(visible_text, theme.dim));
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    // ── Private helpers ─────────────────────────────────────────────

    fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position = self.prev_char_boundary();
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.len() {
            self.cursor_position = self.next_char_boundary();
        }
    }

    /// Find the byte offset of the previous character boundary.
    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.cursor_position;
        if pos == 0 {
            return 0;
        }
        pos -= 1;
        while pos > 0 && !self.input.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    /// Find the byte offset of the next character boundary.
    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor_position;
        if pos >= self.input.len() {
            return self.input.len();
        }
        pos += 1;
        while pos < self.input.len() && !self.input.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }

    /// Delete the word before the cursor (Ctrl+W behaviour).
    fn delete_word_backwards(&mut self) {
        if self.cursor_position == 0 {
            return;
        }
        let bytes = self.input.as_bytes();
        let mut end = self.cursor_position;
        // Skip trailing whitespace
        while end > 0 && bytes[end - 1] == b' ' {
            end -= 1;
        }
        // Skip non-whitespace (the word)
        let start = {
            let mut s = end;
            while s > 0 && bytes[s - 1] != b' ' {
                s -= 1;
            }
            s
        };
        self.input.drain(start..self.cursor_position);
        self.cursor_position = start;
    }
}

impl Default for PromptInput {
    fn default() -> Self {
        Self::new()
    }
}
