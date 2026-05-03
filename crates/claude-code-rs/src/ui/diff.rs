use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use similar::{ChangeTag, TextDiff};
use unicode_width::UnicodeWidthChar;

use super::theme::Theme;

/// A single line from a unified diff.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Whether this line was added, removed, or is context.
    pub tag: ChangeTag,
    /// The text content of this line (without the leading +/- prefix).
    pub content: String,
}

/// Compute a line-level diff between `old` and `new` and return a list of
/// [`DiffLine`] entries.
pub fn format_diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();

    for change in diff.iter_all_changes() {
        lines.push(DiffLine {
            tag: change.tag(),
            content: change.value().trim_end_matches('\n').to_string(),
        });
    }

    lines
}

/// Render a unified diff between `old` and `new` into the given buffer area.
///
/// Each line is prefixed with `+`, `-`, or a space and colored accordingly.
/// The output is clipped to the available area height.
pub fn render_diff(old: &str, new: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let diff_lines = format_diff_lines(old, new);
    let max_lines = area.height as usize;
    let max_width = area.width as usize;

    // Render a header line if there is room.
    let mut y_offset: u16 = 0;
    let mut old_line_no = 1usize;
    let mut new_line_no = 1usize;

    if !diff_lines.is_empty() {
        // Count additions and deletions for the header.
        let additions = diff_lines
            .iter()
            .filter(|l| l.tag == ChangeTag::Insert)
            .count();
        let deletions = diff_lines
            .iter()
            .filter(|l| l.tag == ChangeTag::Delete)
            .count();
        let header = format!("--- diff: +{} -{} lines ---", additions, deletions);
        let header_line = Line::from(Span::styled(header, theme.diff_header));
        buf.set_line(area.x, area.y + y_offset, &header_line, area.width);
        y_offset += 1;
    }

    let max_body_lines = max_lines.saturating_sub(1);
    for diff_line in diff_lines.iter().take(max_body_lines) {
        if y_offset >= area.height {
            break;
        }

        let (prefix, style, line_no_spans) = match diff_line.tag {
            ChangeTag::Insert => {
                let spans = vec![
                    Span::styled(format!("{:>4}", ""), theme.dim),
                    Span::styled(format!("{:>4}", new_line_no), theme.dim),
                ];
                new_line_no += 1;
                ("+", theme.diff_add, spans)
            }
            ChangeTag::Delete => {
                let spans = vec![
                    Span::styled(format!("{:>4}", old_line_no), theme.dim),
                    Span::styled(format!("{:>4}", ""), theme.dim),
                ];
                old_line_no += 1;
                ("-", theme.diff_remove, spans)
            }
            ChangeTag::Equal => {
                let spans = vec![
                    Span::styled(format!("{:>4}", old_line_no), theme.dim),
                    Span::styled(format!("{:>4}", new_line_no), theme.dim),
                ];
                old_line_no += 1;
                new_line_no += 1;
                (" ", theme.diff_context, spans)
            }
        };

        // Truncate content to fit within the available width without
        // splitting multi-byte characters.
        let prefix_width = 12usize;
        let content_width = max_width.saturating_sub(prefix_width);
        let content = truncate_visible(&diff_line.content, content_width);

        let line = Line::from(vec![
            line_no_spans[0].clone(),
            Span::raw(" "),
            line_no_spans[1].clone(),
            Span::raw(" "),
            Span::styled(prefix.to_string(), style),
            Span::styled(" ".to_string(), style),
            Span::styled(content, style),
        ]);

        buf.set_line(area.x, area.y + y_offset, &line, area.width);
        y_offset += 1;
    }

    // If there are more lines than fit, show a truncation notice.
    let total_content_lines = diff_lines.len();
    let shown = (max_lines.saturating_sub(1)).min(total_content_lines);
    if shown < total_content_lines && y_offset < area.height {
        let remaining = total_content_lines - shown;
        let notice = format!("  ... {} more lines ...", remaining);
        let notice_line = Line::from(Span::styled(notice, theme.dim));
        buf.set_line(area.x, area.y + y_offset, &notice_line, area.width);
    }
}

fn truncate_visible(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut width = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}
