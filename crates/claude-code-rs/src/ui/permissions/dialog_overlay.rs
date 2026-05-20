use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)] // Phase 1: ratatui Style reserved for future dialog styling
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::ui::approval_overlay::ApprovalKind;
use crate::ui::permissions::permission_request_router::PermissionRequestRouter;
use crate::ui::theme::Theme;

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
        let routed = PermissionRequestRouter::route(tool_name, input, message, 0);
        let kind = approval_kind(tool_name, input, message);
        let options = if routed.options.is_empty() {
            default_options()
        } else {
            routed.options
        };
        Self {
            tool_name: tool_name.to_string(),
            tool_input: input.to_string(),
            message: message.to_string(),
            kind,
            options,
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
            let body_lines =
                self.body_lines(request_inner.width as usize, request_inner.height, theme);

            Paragraph::new(body_lines)
                .wrap(Wrap { trim: true })
                .render_ref(request_inner, buf);
        }

        let labels = self.normalized_options();
        let footer_width = chunks[2].width.saturating_sub(2) as usize;
        let button_spans = button_spans_for_width(&labels, self.selected, footer_width, theme);
        let button_line = Line::from(button_spans);
        let button_y = chunks[2].y + (chunks[2].height.saturating_sub(2)) / 2;
        buf.set_line(chunks[2].x + 1, button_y, &button_line, footer_width as u16);

        let hint = Line::from(vec![Span::styled(
            truncate_str("Arrows/hotkeys. Enter confirms. Esc denies.", footer_width),
            theme.dim,
        )]);
        buf.set_line(
            chunks[2].x + 1,
            chunks[2].y + chunks[2].height.saturating_sub(1),
            &hint,
            footer_width as u16,
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

    fn body_lines(&self, max_width: usize, max_height: u16, theme: &Theme) -> Vec<Line<'static>> {
        let routed = PermissionRequestRouter::route(
            &self.tool_name,
            &self.tool_input,
            &self.message,
            self.selected,
        );
        let routed_lines = routed_detail_lines(&routed.rendered)
            .into_iter()
            .take(max_height.saturating_sub(1) as usize)
            .map(|line| {
                Line::from(vec![Span::styled(
                    truncate_str(&line, max_width),
                    theme_style_for_routed_line(&line, theme),
                )])
            })
            .collect::<Vec<_>>();

        if !routed_lines.is_empty() {
            return routed_lines;
        }

        let body_label = body_label_for_kind(&self.kind);
        let body_text = self.body_text();
        let mut body_lines = vec![Line::from(vec![
            Span::styled(format!("{body_label}: "), theme.dim),
            Span::styled(truncate_str(&body_text, max_width), theme.warning),
        ])];

        if !self.message.trim().is_empty() && self.message.trim() != body_text.trim() {
            body_lines.push(Line::from(vec![Span::styled(
                truncate_str(&self.message, max_width),
                theme.dim,
            )]));
        }
        body_lines
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

fn theme_style_for_routed_line(line: &str, theme: &Theme) -> Style {
    if line.contains("risk:") || line.contains("network access") || line.contains("unknown tool") {
        theme.warning.add_modifier(Modifier::BOLD)
    } else {
        theme.warning
    }
}

fn routed_detail_lines(rendered: &str) -> Vec<String> {
    rendered
        .lines()
        .map(panel_line_to_text)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_router_chrome_line(line))
        .take(8)
        .collect()
}

fn panel_line_to_text(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with('+') || trimmed.starts_with("|---") {
        return String::new();
    }
    if trimmed.starts_with('|') {
        let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
        let chars = inner.chars().collect::<Vec<_>>();
        let nav_start = 1;
        let nav_end = nav_start + 22;
        let right_column_start = nav_end + 1;
        if chars.len() > right_column_start {
            let left = chars[nav_start..nav_end.min(chars.len())]
                .iter()
                .collect::<String>();
            if !is_panel_nav_cell(&left) {
                return String::new();
            }
            return chars[right_column_start..]
                .iter()
                .collect::<String>()
                .trim()
                .to_string();
        }
        return String::new();
    }
    trimmed.to_string()
}

fn is_panel_nav_cell(cell: &str) -> bool {
    let label = cell.trim().trim_start_matches('>').trim();
    label.is_empty() || matches!(label, "Request" | "Context" | "Decisions")
}

fn is_router_chrome_line(line: &str) -> bool {
    line.eq_ignore_ascii_case("Request")
        || line.eq_ignore_ascii_case("Context")
        || line.eq_ignore_ascii_case("Decision")
        || line.eq_ignore_ascii_case("Decisions")
        || line.contains("Request") && line.contains("Decision")
        || line.starts_with("tool=")
        || line.starts_with("Up/Down decision")
        || line.contains("scope=")
}

#[cfg(test)]
mod tests {
    use super::{routed_detail_lines, PermissionChoice, PermissionDialog};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use crate::ui::permissions::web_fetch_permission_request::web_fetch_permission_request::render_web_fetch_permission_request;
    use crate::ui::theme::Theme;

    #[test]
    fn routed_detail_lines_keep_detail_column_without_panel_chrome() {
        let rendered = render_web_fetch_permission_request("https://example.com/docs", "GET", 0);
        let lines = routed_detail_lines(&rendered);

        assert!(lines.iter().any(|line| line.contains("https://example.com/docs")));
        assert!(lines.iter().any(|line| line.contains("method: GET")));
        assert!(!lines.iter().any(|line| line.contains("Web fetch permission")));
        assert!(!lines.iter().any(|line| line.contains("Request") && line.contains("Decision")));
        assert!(!lines.iter().any(|line| line == "Decision"));
        assert!(!lines.iter().any(|line| line == "work access"));
        assert!(!lines.iter().any(|line| line.starts_with("tool=")));
        assert!(!lines.iter().any(|line| line.starts_with("risk=")));
        assert!(!lines.iter().any(|line| line.contains("scope=")));
        assert!(!lines.iter().any(|line| line.contains("Up/Down")));
    }

    #[test]
    fn rendered_dialog_omits_routed_panel_chrome() {
        let dialog = PermissionDialog::new("WebFetch", r#"{"url":"https://example.com/docs"}"#, "");
        let rendered = render_dialog_text(&dialog);

        assert!(rendered.contains("Permission Required"));
        assert!(rendered.contains("https://example.com/docs"));
        assert!(rendered.contains("method: GET"));
        assert!(!rendered.contains("Web fetch permission"));
        assert!(!rendered.contains("tool=web_fetch"));
        assert!(!rendered.contains("risk=network access"));
        assert!(!rendered.contains("scope="));
        assert!(!rendered.contains("Up/Down decision"));
        assert!(!rendered.lines().any(|line| line.trim() == "Decision"));
        assert!(!rendered.lines().any(|line| line.trim() == "work access"));
    }

    #[test]
    fn rendered_dialog_extracts_structured_bash_json() {
        let dialog = PermissionDialog::new("Bash", r#"{"command":"cargo test"}"#, "");
        let rendered = render_dialog_text(&dialog);

        assert!(rendered.contains("command: cargo test"));
        assert!(!rendered.contains(r#"{"command":"#));
    }

    #[test]
    fn rendered_dialog_extracts_structured_file_json() {
        let dialog = PermissionDialog::new("Edit", r#"{"file_path":"src/lib.rs"}"#, "");
        let rendered = render_dialog_text(&dialog);

        assert!(rendered.contains("src/lib.rs"));
        assert!(!rendered.contains(r#"{"file_path":"#));
    }

    #[test]
    fn file_permission_labels_navigate_to_choices() {
        let mut dialog = PermissionDialog::new("Edit", r#"{"file_path":"src/lib.rs"}"#, "");
        let rendered = render_dialog_text(&dialog);

        assert!(rendered.contains("Allow edit"));
        assert!(rendered.contains("Deny edit"));
        assert!(rendered.contains("Always allow path"));

        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(PermissionChoice::Allow)
        );

        let mut dialog = PermissionDialog::new("Edit", r#"{"file_path":"src/lib.rs"}"#, "");
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(PermissionChoice::Deny)
        );

        let mut dialog = PermissionDialog::new("Edit", r#"{"file_path":"src/lib.rs"}"#, "");
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(PermissionChoice::AlwaysAllow)
        );
    }

    fn render_dialog_text(dialog: &PermissionDialog) -> String {
        let area = Rect::new(0, 0, 120, 24);
        let mut buffer = Buffer::empty(area);
        dialog.render(area, &mut buffer, &Theme::default());
        buffer_text(&buffer, area)
    }

    fn buffer_text(buffer: &Buffer, area: Rect) -> String {
        let mut lines = Vec::new();
        for y in area.y..area.y + area.height {
            let mut line = String::new();
            for x in area.x..area.x + area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
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

fn button_spans_for_width(
    labels: &[String],
    selected: usize,
    max_width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut spans = button_spans(labels, selected, max_width, theme, true);
    if spans_width(&spans) <= max_width {
        return spans;
    }

    spans = button_spans(labels, selected, max_width, theme, false);
    if spans_width(&spans) <= max_width {
        return spans;
    }

    vec![Span::styled(
        truncate_str(&spans_text(&spans), max_width),
        theme.unselected,
    )]
}

fn button_spans(
    labels: &[String],
    selected: usize,
    max_width: usize,
    theme: &Theme,
    padded: bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, label) in labels.iter().enumerate() {
        let style = if idx == selected {
            theme.selected
        } else {
            theme.unselected
        };
        let label = button_bar_label(label);
        let shortcut = shortcut_for_label(label);
        let text = match (padded, shortcut) {
            (true, Some(shortcut)) => format!(" {label} ({shortcut}) "),
            (true, None) => format!(" {label} "),
            (false, Some(shortcut)) => format!("{label}({shortcut})"),
            (false, None) => label.to_string(),
        };
        spans.push(Span::styled(truncate_str(&text, max_width), style));
        if idx + 1 < labels.len() {
            spans.push(Span::raw(if padded { "  " } else { " " }));
        }
    }
    spans
}

fn button_bar_label(label: &str) -> &'static str {
    let lower = label.to_ascii_lowercase();
    if lower.contains("always") && lower.contains("exact") {
        "Always exact"
    } else if lower.contains("always") && lower.contains("path") {
        "Always path"
    } else if lower.contains("always") {
        "Always"
    } else if lower.contains("allow") {
        "Allow"
    } else if lower.contains("deny") || lower.contains("reject") || lower == "no" {
        "Deny"
    } else if lower == "yes" {
        "Allow"
    } else {
        "Select"
    }
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn spans_text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
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
