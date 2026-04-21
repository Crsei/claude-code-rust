use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::theme::Theme;

/// The user's response to a permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChoice {
    /// Allow this single invocation.
    Allow,
    /// Deny this single invocation.
    Deny,
    /// Always allow this tool (add a permanent rule).
    AlwaysAllow,
}

/// Number of choices in the dialog.
const NUM_CHOICES: usize = 3;

/// Labels for each choice, in order.
const CHOICE_LABELS: [&str; NUM_CHOICES] = ["Allow", "Deny", "Always Allow"];

/// An overlay dialog that asks the user whether to permit a tool invocation.
pub struct PermissionDialog {
    /// Name of the tool requesting permission.
    pub tool_name: String,
    /// Abbreviated / formatted tool input.
    pub tool_input: String,
    /// Human-readable description of what the tool wants to do.
    pub message: String,
    /// Currently highlighted choice (0 = Allow, 1 = Deny, 2 = Always Allow).
    pub selected: usize,
}

impl PermissionDialog {
    pub fn new(tool_name: &str, input: &str, message: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            tool_input: input.to_string(),
            message: message.to_string(),
            selected: 0,
        }
    }

    /// Handle a key event. Returns `Some(choice)` when the user confirms a
    /// selection with Enter, or makes a direct choice via a keyboard shortcut.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PermissionChoice> {
        match (key.modifiers, key.code) {
            // ── Navigation ──────────────────────────────────────────
            (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            (_, KeyCode::Right) | (_, KeyCode::Char('l')) => {
                if self.selected < NUM_CHOICES - 1 {
                    self.selected += 1;
                }
            }
            (_, KeyCode::Tab) => {
                self.selected = (self.selected + 1) % NUM_CHOICES;
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                self.selected = if self.selected == 0 {
                    NUM_CHOICES - 1
                } else {
                    self.selected - 1
                };
            }

            // ── Confirm ─────────────────────────────────────────────
            (_, KeyCode::Enter) => {
                return Some(self.choice());
            }

            // ── Quick keys ──────────────────────────────────────────
            (_, KeyCode::Char('y')) | (_, KeyCode::Char('Y')) => {
                return Some(PermissionChoice::Allow);
            }
            (_, KeyCode::Char('n')) | (_, KeyCode::Char('N')) => {
                return Some(PermissionChoice::Deny);
            }
            (_, KeyCode::Char('a')) | (_, KeyCode::Char('A')) => {
                return Some(PermissionChoice::AlwaysAllow);
            }
            (_, KeyCode::Esc) => {
                return Some(PermissionChoice::Deny);
            }

            _ => {}
        }
        None
    }

    /// Render the permission dialog as a centered overlay.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Compute a centered dialog area (60% width, capped height).
        let dialog_width = (area.width * 60 / 100).max(40).min(area.width);
        let dialog_height = 12u16.min(area.height);
        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
        let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

        // Clear the background behind the dialog.
        Clear.render_ref(dialog_area, buf);

        // Draw the outer border.
        let block = Block::default()
            .title(" Permission Required ")
            .borders(Borders::ALL)
            .border_style(theme.warning)
            .style(Style::default());
        let inner = block.inner(dialog_area);
        block.render_ref(dialog_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Split inner area into: tool info, message, buttons.
        let chunks = Layout::vertical([
            Constraint::Length(2), // tool name + input
            Constraint::Min(2),    // message
            Constraint::Length(2), // button row
        ])
        .split(inner);

        // ── Tool info ───────────────────────────────────────────────
        let tool_info = vec![
            Line::from(vec![
                Span::styled("Tool: ", theme.dim),
                Span::styled(self.tool_name.clone(), theme.tool_name),
            ]),
            Line::from(vec![
                Span::styled("Input: ", theme.dim),
                Span::styled(
                    truncate_str(&self.tool_input, chunks[0].width as usize - 8),
                    theme.dim,
                ),
            ]),
        ];
        let tool_para = Paragraph::new(tool_info);
        tool_para.render_ref(chunks[0], buf);

        // ── Message ─────────────────────────────────────────────────
        let msg_para = Paragraph::new(self.message.clone())
            .style(theme.warning)
            .wrap(Wrap { trim: true });
        msg_para.render_ref(chunks[1], buf);

        // ── Buttons ─────────────────────────────────────────────────
        let button_spans: Vec<Span> = CHOICE_LABELS
            .iter()
            .enumerate()
            .flat_map(|(i, label)| {
                let style = if i == self.selected {
                    theme.selected
                } else {
                    theme.unselected
                };
                let shortcut = match i {
                    0 => "(y)",
                    1 => "(n)",
                    2 => "(a)",
                    _ => "",
                };
                let mut spans = vec![Span::styled(format!(" {} {} ", label, shortcut), style)];
                if i < NUM_CHOICES - 1 {
                    spans.push(Span::raw("  "));
                }
                spans
            })
            .collect();

        let button_line = Line::from(button_spans);
        // Render the buttons centered in their area.
        let buttons_y = chunks[2].y + (chunks[2].height.saturating_sub(1)) / 2;
        buf.set_line(chunks[2].x + 1, buttons_y, &button_line, chunks[2].width);
    }

    // ── Private ─────────────────────────────────────────────────────

    fn choice(&self) -> PermissionChoice {
        match self.selected {
            0 => PermissionChoice::Allow,
            1 => PermissionChoice::Deny,
            2 => PermissionChoice::AlwaysAllow,
            _ => PermissionChoice::Allow,
        }
    }
}

/// Helper to render a ratatui widget via its `render_ref` method on a `WidgetRef`
/// or via direct buffer writes. We use a small extension trait here so that
/// standard ratatui widgets (Block, Paragraph, Clear) can be used uniformly.
trait RenderRef {
    fn render_ref(&self, area: Rect, buf: &mut Buffer);
}

impl RenderRef for Clear {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
    }
}

impl RenderRef for Block<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self.clone(), area, buf);
    }
}

impl RenderRef for Paragraph<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self.clone(), area, buf);
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if
/// truncation occurred.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars < 4 {
        return s.chars().take(max_chars).collect();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars - 3].iter().collect();
        format!("{}...", truncated)
    }
}
