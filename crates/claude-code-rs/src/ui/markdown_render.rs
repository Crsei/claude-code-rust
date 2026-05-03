//! Markdown-to-ratatui rendering facade.

use std::path::Path;

use ratatui::text::{Line, Span, Text};

use super::markdown::markdown_to_lines;
use super::theme::Theme;

pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_theme(input, &Theme::default())
}

pub fn render_markdown_text_with_theme(input: &str, theme: &Theme) -> Text<'static> {
    Text::from(markdown_to_lines(input, theme))
}

pub(crate) fn render_markdown_text_with_width(input: &str, width: Option<usize>) -> Text<'static> {
    render_markdown_text_with_width_and_cwd(input, width, None)
}

pub(crate) fn render_markdown_text_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    _cwd: Option<&Path>,
) -> Text<'static> {
    let theme = Theme::default();
    let lines = markdown_to_lines(input, &theme);
    match width {
        Some(width) if width > 0 => Text::from(wrap_lines(lines, width)),
        _ => Text::from(lines),
    }
}

fn wrap_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for line in lines {
        let plain = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        if plain.is_empty() {
            out.push(Line::default());
            continue;
        }
        for wrapped in textwrap::wrap(&plain, width.max(1)) {
            out.push(Line::from(Span::raw(wrapped.into_owned())));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_markdown_lines() {
        let text = render_markdown_text("**bold**");
        assert!(!text.lines.is_empty());
    }
}
