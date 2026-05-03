use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::approval_overlay::ApprovalKind;
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

const DEFAULT_OPTIONS: [&str; 3] = ["Allow", "Deny", "Always Allow"];

/// An overlay dialog that asks the user whether to permit a tool invocation.
pub struct PermissionDialog {
    /// Name of the tool requesting permission.
    pub tool_name: String,
    /// Abbreviated / formatted tool input.
    pub tool_input: String,
    /// Human-readable description of what the tool wants to do.
    pub message: String,
    /// Current permission category inferred from the tool name.
    kind: ApprovalKind,
    /// Button labels shown at the bottom of the dialog.
    options: Vec<String>,
    /// Currently highlighted choice.
    pub selected: usize,
}

impl PermissionDialog {
    pub fn new(tool_name: &str, input: &str, message: &str) -> Self {
        let kind = approval_kind(tool_name, input, message);
        Self {
            tool_name: tool_name.to_string(),
            tool_input: input.to_string(),
            message: message.to_string(),
            kind,
            options: default_options(),
            selected: 0,
        }
    }

    /// Handle a key event. Returns `Some(choice)` when the user confirms a
    /// selection with Enter, or makes a direct choice via a keyboard shortcut.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PermissionChoice> {
        let choice_count = self.options.len().max(DEFAULT_OPTIONS.len());

        match (key.modifiers, key.code) {
            // Navigation
            (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {
                self.selected = self.selected.saturating_sub(1);
            }
            (_, KeyCode::Right) | (_, KeyCode::Char('l')) => {
                if self.selected + 1 < choice_count {
                    self.selected += 1;
                }
            }
            (_, KeyCode::Tab) => {
                self.selected = (self.selected + 1) % choice_count;
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                self.selected = if self.selected == 0 {
                    choice_count - 1
                } else {
                    self.selected - 1
                };
            }

            // Confirm
            (_, KeyCode::Enter) => {
                return Some(self.choice_for_selected());
            }

            // Quick keys
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
        let dialog_width = (area.width * 68 / 100).max(48).min(area.width);
        let dialog_height = 13u16.min(area.height).max(8);
        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
        let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

        Clear.render_ref(dialog_area, buf);

        let block = Block::default()
            .title(format!(" Permission Required - {} ", self.kind.title()))
            .borders(Borders::ALL)
            .border_style(theme.warning)
            .style(Style::default());
        let inner = block.inner(dialog_area);
        block.render_ref(dialog_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(2), // tool info
            Constraint::Min(2),    // request body
            Constraint::Length(2), // buttons + hint
        ])
        .split(inner);

        let tool_info = vec![
            Line::from(vec![
                Span::styled("Tool: ", theme.dim),
                Span::styled(self.tool_name.clone(), theme.tool_name),
            ]),
            Line::from(vec![
                Span::styled("Kind: ", theme.dim),
                Span::styled(
                    self.kind.title().trim_end_matches('?').to_string(),
                    theme.info,
                ),
            ]),
        ];
        Paragraph::new(tool_info).render_ref(chunks[0], buf);

        let request_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme.warning);
        let request_inner = request_block.inner(chunks[1]);
        request_block.render_ref(chunks[1], buf);
        if request_inner.height > 0 && request_inner.width > 0 {
            let body_label = body_label_for_kind(&self.kind);
            let body_text = self.body_text();
            let mut body_lines = vec![Line::from(vec![
                Span::styled(format!("{body_label}: "), theme.dim),
                Span::styled(
                    truncate_str(&body_text, request_inner.width as usize),
                    theme.warning,
                ),
            ])];

            if !self.message.trim().is_empty() && self.message.trim() != body_text.trim() {
                body_lines.push(Line::from(vec![Span::styled(
                    truncate_str(&self.message, request_inner.width as usize),
                    theme.dim,
                )]));
            }

            Paragraph::new(body_lines)
                .wrap(Wrap { trim: true })
                .render_ref(request_inner, buf);
        }

        let labels = self.normalized_options();
        let button_spans: Vec<Span> = labels
            .iter()
            .enumerate()
            .flat_map(|(idx, label)| {
                let style = if idx == self.selected {
                    theme.selected
                } else {
                    theme.unselected
                };
                let shortcut = shortcut_for_label(label);
                let mut spans = vec![Span::styled(format!(" {} ", label), style)];
                if let Some(shortcut) = shortcut {
                    spans.push(Span::styled(format!("({shortcut})"), theme.dim));
                }
                if idx + 1 < labels.len() {
                    spans.push(Span::raw("  "));
                }
                spans
            })
            .collect();

        let button_line = Line::from(button_spans);
        let button_y = chunks[2].y + (chunks[2].height.saturating_sub(2)) / 2;
        buf.set_line(chunks[2].x + 1, button_y, &button_line, chunks[2].width);

        let hint = Line::from(vec![Span::styled(
            "Arrow keys or hotkeys. Enter confirms. Esc denies.",
            theme.dim,
        )]);
        buf.set_line(
            chunks[2].x + 1,
            chunks[2].y + chunks[2].height.saturating_sub(1),
            &hint,
            chunks[2].width,
        );
    }

    fn body_text(&self) -> String {
        if !self.tool_input.trim().is_empty() {
            self.tool_input.clone()
        } else if !self.message.trim().is_empty() {
            self.message.clone()
        } else {
            "(no details supplied)".to_string()
        }
    }

    fn normalized_options(&self) -> Vec<String> {
        if self.options.is_empty() {
            default_options()
        } else {
            self.options.clone()
        }
    }

    fn choice_for_selected(&self) -> PermissionChoice {
        let labels = self.normalized_options();
        labels
            .get(self.selected.min(labels.len().saturating_sub(1)))
            .map(|label| choice_for_label(label).unwrap_or(PermissionChoice::Allow))
            .unwrap_or(PermissionChoice::Allow)
    }
}

fn approval_kind(tool_name: &str, input: &str, message: &str) -> ApprovalKind {
    let joined = format!("{tool_name} {input} {message}").to_ascii_lowercase();
    let subject = if !input.trim().is_empty() {
        input.trim().to_string()
    } else if !message.trim().is_empty() {
        message.trim().to_string()
    } else {
        tool_name.to_string()
    };

    if joined.contains("bash")
        || joined.contains("shell")
        || joined.contains("terminal")
        || joined.contains("powershell")
    {
        ApprovalKind::Bash { command: subject }
    } else if joined.contains("write") || joined.contains("edit") || joined.contains("patch") {
        ApprovalKind::FileEdit { path: subject }
    } else if joined.contains("fetch") || joined.contains("web") || joined.contains("url") {
        ApprovalKind::WebFetch { url: subject }
    } else if joined.contains("mcp") {
        ApprovalKind::Mcp {
            server: tool_name.to_string(),
            action: subject,
        }
    } else if joined.contains("ask") || joined.contains("question") {
        ApprovalKind::UserInput { prompt: subject }
    } else {
        ApprovalKind::Fallback { summary: subject }
    }
}

fn body_label_for_kind(kind: &ApprovalKind) -> &'static str {
    match kind {
        ApprovalKind::Bash { .. } => "Command",
        ApprovalKind::FileEdit { .. } => "File",
        ApprovalKind::WebFetch { .. } => "URL",
        ApprovalKind::Mcp { .. } => "Action",
        ApprovalKind::UserInput { .. } => "Prompt",
        ApprovalKind::Fallback { .. } => "Details",
    }
}

fn default_options() -> Vec<String> {
    DEFAULT_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn choice_for_label(label: &str) -> Option<PermissionChoice> {
    let lower = label.to_ascii_lowercase();
    if lower.contains("always") {
        Some(PermissionChoice::AlwaysAllow)
    } else if lower.contains("allow") && !lower.contains("always") {
        Some(PermissionChoice::Allow)
    } else if lower.contains("deny") || lower.contains("reject") || lower == "no" {
        Some(PermissionChoice::Deny)
    } else {
        None
    }
}

fn shortcut_for_label(label: &str) -> Option<&'static str> {
    match choice_for_label(label) {
        Some(PermissionChoice::Allow) => Some("y"),
        Some(PermissionChoice::Deny) => Some("n"),
        Some(PermissionChoice::AlwaysAllow) => Some("a"),
        None => None,
    }
}

// ── Helper rendering traits ───────────────────────────────────────────────

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
