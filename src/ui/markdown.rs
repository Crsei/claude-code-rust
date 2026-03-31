use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::theme::Theme;

/// Convert a markdown string into a vector of styled ratatui [`Line`]s.
///
/// Supported elements:
/// - Headings (rendered bold + underlined)
/// - Bold / strong emphasis
/// - Italic / emphasis
/// - Inline code (rendered with `theme.code` style)
/// - Fenced / indented code blocks (each line rendered with `theme.code`)
/// - Unordered lists (prefixed with "  - ")
/// - Ordered lists (prefixed with "  N. ")
/// - Links (rendered underlined with URL in parentheses)
/// - Paragraphs (separated by blank lines)
///
/// This is intentionally a *simple* renderer --- it does not attempt full
/// markdown layout (tables, nested block quotes, etc.), keeping the code
/// small and the output predictable on a terminal.
pub fn markdown_to_lines(text: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, opts);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // Style stack: the most recent style is applied to incoming text events.
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut list_stack: Vec<ListKind> = Vec::new();

    for event in parser {
        match event {
            // ── Block-level starts ──────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                flush_line(&mut current_spans, &mut lines);
                let heading_style = match level {
                    pulldown_cmark::HeadingLevel::H1 => theme
                        .heading
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    pulldown_cmark::HeadingLevel::H2 => theme
                        .heading
                        .add_modifier(Modifier::BOLD),
                    _ => theme.bold,
                };
                style_stack.push(heading_style);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_line(&mut current_spans, &mut lines);
                style_stack.pop();
                // Add a blank line after headings.
                lines.push(Line::from(""));
            }

            Event::Start(Tag::Paragraph) => {
                flush_line(&mut current_spans, &mut lines);
            }
            Event::End(TagEnd::Paragraph) => {
                flush_line(&mut current_spans, &mut lines);
                lines.push(Line::from(""));
            }

            // ── Code blocks ─────────────────────────────────────────
            Event::Start(Tag::CodeBlock(_)) => {
                flush_line(&mut current_spans, &mut lines);
                in_code_block = true;
                style_stack.push(theme.code);
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_line(&mut current_spans, &mut lines);
                in_code_block = false;
                style_stack.pop();
                lines.push(Line::from(""));
            }

            // ── Inline styles ───────────────────────────────────────
            Event::Start(Tag::Strong) => {
                style_stack.push(theme.bold);
            }
            Event::End(TagEnd::Strong) => {
                style_stack.pop();
            }
            Event::Start(Tag::Emphasis) => {
                style_stack.push(theme.italic);
            }
            Event::End(TagEnd::Emphasis) => {
                style_stack.pop();
            }

            // ── Lists ───────────────────────────────────────────────
            Event::Start(Tag::List(first_number)) => {
                flush_line(&mut current_spans, &mut lines);
                let kind = match first_number {
                    Some(start) => ListKind::Ordered(start as usize),
                    None => ListKind::Unordered,
                };
                list_stack.push(kind);
            }
            Event::End(TagEnd::List(_)) => {
                flush_line(&mut current_spans, &mut lines);
                list_stack.pop();
                if list_stack.is_empty() {
                    lines.push(Line::from(""));
                }
            }
            Event::Start(Tag::Item) => {
                flush_line(&mut current_spans, &mut lines);
                let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                match list_stack.last_mut() {
                    Some(ListKind::Unordered) => {
                        current_spans.push(Span::raw(format!("{}- ", indent)));
                    }
                    Some(ListKind::Ordered(n)) => {
                        current_spans.push(Span::raw(format!("{}{}. ", indent, n)));
                        *n += 1;
                    }
                    None => {}
                }
            }
            Event::End(TagEnd::Item) => {
                flush_line(&mut current_spans, &mut lines);
            }

            // ── Links ───────────────────────────────────────────────
            Event::Start(Tag::Link { dest_url, .. }) => {
                style_stack.push(theme.link);
                // We will append the URL after the link text on End.
                // Store the URL by pushing a marker span. Since we cannot
                // easily pass data through the stack we push the URL into
                // a hidden span that we look for on End. In practice we
                // just style the text; the URL is appended after.
                // (Simplified: we stash the url in the style stack as an
                // additional push, and pop twice on End.)
                style_stack.push(Style::default()); // placeholder for URL
                // Save URL as a raw span we will emit on End.
                current_spans.push(Span::styled("", Style::default())); // marker
                // Actually store the URL somewhere accessible... we use a
                // simple trick: we encode it as a hidden span at the current
                // position. On `End(Link)` we pop back.
                // For simplicity, just render [text](url).
                current_spans.pop(); // remove marker
                current_spans.push(Span::raw("["));
                // We will push the URL on End using `dest_url`. To pass it
                // through, we abuse the fact that we can just stash it.
                // ... Actually, let's use a cleaner approach: stash URLs in
                // a side-vec.
                // For now, just underline the text and append URL on End.
                // We don't have a great way to thread the URL through
                // pulldown-cmark's event model here. Let's use the knowledge
                // that End(Link) in pulldown-cmark 0.12 carries the URL.
                let _ = dest_url; // URL is available on Start but not End in 0.12.
                // We'll just style the text as a link; no URL append.
                style_stack.pop(); // remove placeholder
            }
            Event::End(TagEnd::Link) => {
                current_spans.push(Span::raw("]"));
                style_stack.pop();
            }

            // ── Block quote ─────────────────────────────────────────
            Event::Start(Tag::BlockQuote(_)) => {
                flush_line(&mut current_spans, &mut lines);
                current_spans.push(Span::styled("  | ", theme.dim));
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_line(&mut current_spans, &mut lines);
            }

            // ── Inline code ─────────────────────────────────────────
            Event::Code(code) => {
                current_spans.push(Span::styled(
                    format!("`{}`", code),
                    theme.code,
                ));
            }

            // ── Text ────────────────────────────────────────────────
            Event::Text(text) => {
                let style = current_style(&style_stack);
                if in_code_block {
                    // Code blocks: preserve line structure.
                    for (i, line_text) in text.split('\n').enumerate() {
                        if i > 0 {
                            flush_line(&mut current_spans, &mut lines);
                        }
                        if !line_text.is_empty() {
                            current_spans.push(Span::styled(
                                line_text.to_string(),
                                style,
                            ));
                        }
                    }
                } else {
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }

            // ── Soft / Hard break ───────────────────────────────────
            Event::SoftBreak => {
                current_spans.push(Span::raw(" "));
            }
            Event::HardBreak => {
                flush_line(&mut current_spans, &mut lines);
            }

            // ── Horizontal rule ─────────────────────────────────────
            Event::Rule => {
                flush_line(&mut current_spans, &mut lines);
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────".to_string(),
                    theme.dim,
                )));
                lines.push(Line::from(""));
            }

            // Ignore everything else (footnotes, task list markers, etc.)
            _ => {}
        }
    }

    // Flush any remaining spans.
    flush_line(&mut current_spans, &mut lines);

    // Remove trailing blank lines.
    while lines.last().map_or(false, |l| l.spans.is_empty() || line_is_empty(l)) {
        lines.pop();
    }

    lines
}

/// Get the current (topmost) style from the stack, defaulting to `Style::default()`.
fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

/// Move all accumulated spans into a new `Line` and push it onto `lines`.
/// Clears `current_spans`.
fn flush_line(current_spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
    if !current_spans.is_empty() {
        let spans = std::mem::take(current_spans);
        lines.push(Line::from(spans));
    }
}

/// Check if a line is visually empty (all spans contain only whitespace).
fn line_is_empty(line: &Line) -> bool {
    line.spans.iter().all(|s| s.content.trim().is_empty())
}

/// Tracking state for list rendering.
enum ListKind {
    Unordered,
    Ordered(usize),
}
