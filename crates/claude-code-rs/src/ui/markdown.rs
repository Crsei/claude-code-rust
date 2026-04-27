use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use lru::LruCache;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::theme::Theme;

// ---------------------------------------------------------------------------
// LRU cache (thread-local, 256 entries)
// ---------------------------------------------------------------------------

thread_local! {
    static MD_CACHE: RefCell<LruCache<u64, Vec<Line<'static>>>> =
        RefCell::new(LruCache::new(NonZeroUsize::new(256).unwrap()));
}

fn cache_key(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a markdown string into a vector of styled ratatui [`Line`]s.
///
/// Results are LRU-cached so repeated calls with the same content skip
/// re-parsing (e.g. when scrolling back through history).
pub fn markdown_to_lines(text: &str, theme: &Theme) -> Vec<Line<'static>> {
    let key = cache_key(text);
    let cached = MD_CACHE.with(|c| c.borrow_mut().get(&key).cloned());
    if let Some(lines) = cached {
        return lines;
    }
    let lines = markdown_to_lines_inner(text, theme);
    MD_CACHE.with(|c| c.borrow_mut().put(key, lines.clone()));
    lines
}

// ---------------------------------------------------------------------------
// Inner implementation (unchanged logic)
// ---------------------------------------------------------------------------

/// Supported elements:
/// - Headings (rendered bold + underlined)
/// - Bold / strong emphasis
/// - Italic / emphasis
/// - Inline code (rendered with `theme.code` style)
/// - Fenced / indented code blocks (each line rendered with `theme.code`)
/// - Unordered lists (prefixed with "  - ")
/// - Ordered lists (prefixed with "  N. ")
/// - Links (rendered underlined)
/// - Paragraphs (separated by blank lines)
fn markdown_to_lines_inner(text: &str, theme: &Theme) -> Vec<Line<'static>> {
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
                    pulldown_cmark::HeadingLevel::H2 => theme.heading.add_modifier(Modifier::BOLD),
                    _ => theme.bold,
                };
                style_stack.push(heading_style);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_line(&mut current_spans, &mut lines);
                style_stack.pop();
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
                style_stack.push(Style::default());
                current_spans.push(Span::styled("", Style::default()));
                current_spans.pop();
                current_spans.push(Span::raw("["));
                let _ = dest_url;
                style_stack.pop();
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
                current_spans.push(Span::styled(format!("`{}`", code), theme.code));
            }

            // ── Text ────────────────────────────────────────────────
            Event::Text(text) => {
                let style = current_style(&style_stack);
                if in_code_block {
                    for (i, line_text) in text.split('\n').enumerate() {
                        if i > 0 {
                            flush_line(&mut current_spans, &mut lines);
                        }
                        if !line_text.is_empty() {
                            current_spans.push(Span::styled(line_text.to_string(), style));
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

            _ => {}
        }
    }

    flush_line(&mut current_spans, &mut lines);

    // Remove trailing blank lines.
    while lines
        .last()
        .is_some_and(|l| l.spans.is_empty() || line_is_empty(l))
    {
        lines.pop();
    }

    lines
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

fn flush_line(current_spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
    if !current_spans.is_empty() {
        let spans = std::mem::take(current_spans);
        lines.push(Line::from(spans));
    }
}

fn line_is_empty(line: &Line) -> bool {
    line.spans.iter().all(|s| s.content.trim().is_empty())
}

enum ListKind {
    Unordered,
    Ordered(usize),
}
