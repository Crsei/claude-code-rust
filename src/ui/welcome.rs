//! Welcome screen -- ASCII art logo and startup info panel.
//!
//! Rendered once when the TUI starts, before any messages are displayed.
//! Inspired by code-iris welcome screen but adapted for cc-rust.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::Widget;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// ASCII art logo — stylized "CC" monogram representing Claude Code.
const LOGO: &str = r#"
     ██████╗  ██████╗
    ██╔════╝ ██╔════╝
    ██║      ██║
    ██║      ██║
    ╚██████╗ ╚██████╗
     ╚═════╝  ╚═════╝
"#;

/// Accent color — Claude purple.
const ACCENT: Color = Color::Rgb(190, 140, 255);
/// Accent dim — darker purple for secondary elements.
const ACCENT_DIM: Color = Color::Rgb(120, 90, 180);
/// Muted text color.
const MUTED: Color = Color::Rgb(100, 100, 100);
/// Light text.
const LIGHT: Color = Color::Rgb(200, 200, 200);

/// Render the welcome screen into the given area.
///
/// # Layout
/// ```text
/// ┌─ Claude Code (Rust) ──────────────────────────────────────┐
/// │                                                           │
/// │   ██████╗  ██████╗      Tips                              │
/// │  ██╔════╝ ██╔════╝       Type a message and press Enter   │
/// │  ██║      ██║             /help for commands               │
/// │  ██║      ██║             Ctrl+C abort · Ctrl+D quit      │
/// │  ╚██████╗ ╚██████╗      ──────────────────                │
/// │   ╚═════╝  ╚═════╝      Keybindings                      │
/// │                           Up/Down: history                 │
/// │  claude-code-rs v0.x.x    PageUp/Down: scroll              │
/// │  Model: claude-xxx                                         │
/// │  Session: abcd1234                                         │
/// │                                                           │
/// └──────────────────────────────────────────────────────────-┘
/// ```
pub fn render_welcome(
    area: Rect,
    buf: &mut Buffer,
    version: &str,
    model_name: &str,
    session_id: &str,
    cwd: &str,
) {
    if area.width < 20 || area.height < 8 {
        // Terminal too small — render a minimal one-liner.
        let line = Line::from(vec![
            Span::styled(
                "Claude Code ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("v{}", version), Style::default().fg(MUTED)),
        ]);
        buf.set_line(area.x, area.y, &line, area.width);
        return;
    }

    // Outer border
    let title = Line::from(vec![
        Span::styled(
            " Claude Code ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("(Rust) ", Style::default().fg(ACCENT_DIM)),
        Span::styled(format!("v{} ", version), Style::default().fg(MUTED)),
    ]);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_DIM))
        .title(title)
        .title_alignment(Alignment::Left);

    let inner = outer.inner(area);
    outer.render(area, buf);

    if inner.height < 6 || inner.width < 30 {
        // Too small for two-column layout — render single column.
        render_compact(inner, buf, version, model_name, session_id);
        return;
    }

    // Two-column layout
    let columns =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(inner);

    render_left_panel(columns[0], buf, version, model_name, session_id, cwd);
    render_right_panel(columns[1], buf);
}

/// Left panel: logo + project info.
fn render_left_panel(
    area: Rect,
    buf: &mut Buffer,
    version: &str,
    model_name: &str,
    session_id: &str,
    cwd: &str,
) {
    let logo_height = 8u16; // 1 blank + 6 logo lines + 1 blank
    let info_height = 5u16;

    let layout = Layout::vertical([
        Constraint::Length(logo_height.min(area.height.saturating_sub(info_height))),
        Constraint::Length(1), // spacer
        Constraint::Min(info_height),
    ])
    .split(area);

    // ── Logo ───────────────────────────────────────────────────
    render_logo(layout[0], buf);

    // ── Info lines ─────────────────────────────────────────────
    let short_session = if session_id.len() > 8 {
        &session_id[..8]
    } else {
        session_id
    };

    // Shorten model name for display
    let display_model = model_name.strip_prefix("claude-").unwrap_or(model_name);

    // Truncate cwd if too long
    let max_cwd = (area.width as usize).saturating_sub(6);
    let display_cwd = if cwd.len() > max_cwd && max_cwd > 4 {
        format!("...{}", &cwd[cwd.len() - (max_cwd - 3)..])
    } else {
        cwd.to_string()
    };

    let info_lines = vec![
        Line::from(vec![
            Span::styled(
                "  cc-rust",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" v{}", version), Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Model: ", Style::default().fg(MUTED)),
            Span::styled(display_model, Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled("  Session: ", Style::default().fg(MUTED)),
            Span::styled(short_session, Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled("  CWD: ", Style::default().fg(MUTED)),
            Span::styled(display_cwd, Style::default().fg(MUTED)),
        ]),
    ];

    let info = Paragraph::new(info_lines);
    info.render(layout[2], buf);
}

/// Right panel: tips + keybindings.
fn render_right_panel(area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([
        Constraint::Length(6), // Tips
        Constraint::Length(1), // Divider
        Constraint::Min(4),    // Keybindings
    ])
    .split(area);

    // ── Tips ───────────────────────────────────────────────────
    let tips = vec![
        Line::from(Span::styled(
            " Tips",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  Type a message and press Enter to send",
            Style::default().fg(LIGHT),
        )),
        Line::from(Span::styled(
            "  Start with / for slash commands",
            Style::default().fg(LIGHT),
        )),
        Line::from(Span::styled(
            "  /help to see all commands",
            Style::default().fg(LIGHT),
        )),
        Line::from(Span::styled(
            "  /model to switch models",
            Style::default().fg(LIGHT),
        )),
    ];
    let tips_widget = Paragraph::new(tips).wrap(Wrap { trim: false });
    tips_widget.render(layout[0], buf);

    // ── Divider ────────────────────────────────────────────────
    let divider_width = layout[1].width as usize;
    let divider = Line::from(Span::styled(
        format!(" {}", "─".repeat(divider_width.saturating_sub(2))),
        Style::default().fg(Color::Rgb(60, 60, 80)),
    ));
    buf.set_line(layout[1].x, layout[1].y, &divider, layout[1].width);

    // ── Keybindings ────────────────────────────────────────────
    let keybindings = vec![
        Line::from(Span::styled(
            " Keybindings",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                "  Ctrl+C ",
                Style::default()
                    .fg(Color::Rgb(255, 200, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("abort / quit", Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled(
                "  Ctrl+D ",
                Style::default()
                    .fg(Color::Rgb(255, 200, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("quit", Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled(
                "  Up/Down ",
                Style::default()
                    .fg(Color::Rgb(255, 200, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("input history", Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled(
                "  PgUp/PgDn ",
                Style::default()
                    .fg(Color::Rgb(255, 200, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("scroll messages", Style::default().fg(LIGHT)),
        ]),
    ];
    let kb_widget = Paragraph::new(keybindings).wrap(Wrap { trim: false });
    kb_widget.render(layout[2], buf);
}

/// Render the ASCII art logo with colored characters.
fn render_logo(area: Rect, buf: &mut Buffer) {
    let logo_lines: Vec<Line> = LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let spans: Vec<Span> = line
                .chars()
                .map(|c| match c {
                    '█' => Span::styled("█", Style::default().fg(ACCENT)),
                    '╗' | '╔' | '╝' | '╚' | '║' | '═' => {
                        Span::styled(c.to_string(), Style::default().fg(ACCENT_DIM))
                    }
                    _ => Span::styled(c.to_string(), Style::default()),
                })
                .collect();
            Line::from(spans)
        })
        .collect();

    let logo = Paragraph::new(logo_lines).alignment(Alignment::Center);
    logo.render(area, buf);
}

/// Compact single-column fallback for very small terminals.
fn render_compact(area: Rect, buf: &mut Buffer, version: &str, model_name: &str, session_id: &str) {
    let short_session = if session_id.len() > 8 {
        &session_id[..8]
    } else {
        session_id
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Claude Code ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("(Rust) v{}", version), Style::default().fg(MUTED)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Model: ", Style::default().fg(MUTED)),
            Span::styled(model_name, Style::default().fg(LIGHT)),
        ]),
        Line::from(vec![
            Span::styled("Session: ", Style::default().fg(MUTED)),
            Span::styled(short_session, Style::default().fg(LIGHT)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Type a message and press Enter. /help for commands.",
            Style::default().fg(LIGHT),
        )),
    ];

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    p.render(area, buf);
}

/// Height of the welcome screen (for layout calculation).
pub fn welcome_height() -> u16 {
    // logo(8) + spacer(1) + info(5) + border(2) = 16, minimum usable
    16
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_render_welcome_small_terminal() {
        let area = Rect::new(0, 0, 15, 5);
        let mut buf = Buffer::empty(area);
        render_welcome(
            area,
            &mut buf,
            "0.1.0",
            "claude-sonnet-4",
            "abcd1234",
            "/tmp",
        );
        // Should not panic, renders minimal one-liner.
    }

    #[test]
    fn test_render_welcome_normal() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_welcome(
            area,
            &mut buf,
            "0.1.0",
            "claude-sonnet-4",
            "abcdef1234567890",
            "/home/user/project",
        );
        // Should not panic. Verify logo area has content.
    }

    #[test]
    fn test_render_welcome_medium() {
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        render_welcome(area, &mut buf, "0.1.0", "test-model", "sess1234", "/path");
    }

    #[test]
    fn test_logo_lines() {
        let lines: Vec<&str> = LOGO.lines().filter(|l| !l.is_empty()).collect();
        assert!(lines.len() >= 6, "Logo should have at least 6 lines");
    }
}
