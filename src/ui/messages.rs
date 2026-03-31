use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::types::message::{
    ContentBlock, InfoLevel, Message, MessageContent, SystemSubtype, ToolResultContent,
};

use super::markdown::markdown_to_lines;
use super::theme::Theme;

/// Render a scrollable list of messages into the given buffer area.
///
/// `scroll` is the number of *rendered lines* to skip from the top. The
/// caller should track this value and adjust it in response to scroll events.
pub fn render_messages(
    messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    scroll: usize,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // First, build all rendered lines for every message.
    let all_lines = render_all_message_lines(messages, area.width as usize, theme);

    // Apply scroll offset and render visible lines.
    let visible_lines = all_lines.iter().skip(scroll).take(area.height as usize);

    for (i, line) in visible_lines.enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        buf.set_line(area.x, y, line, area.width);
    }
}

/// Compute the total number of rendered lines for the given messages.
///
/// Useful for the caller to know the scroll bounds.
pub fn total_rendered_lines(
    messages: &[Message],
    width: usize,
    theme: &Theme,
) -> usize {
    render_all_message_lines(messages, width, theme).len()
}

// ── Internal helpers ────────────────────────────────────────────────────

/// Render every message into a flat list of `Line`s (including blank
/// separator lines between messages).
fn render_all_message_lines<'a>(
    messages: &[Message],
    #[allow(unused)] width: usize,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        let msg_lines = render_single_message(msg, theme);
        lines.extend(msg_lines);

        // Add a blank line between messages (but not after the last one).
        if idx < messages.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    lines
}

/// Render a single message into one or more `Line`s.
fn render_single_message<'a>(msg: &Message, theme: &Theme) -> Vec<Line<'a>> {
    match msg {
        Message::User(user_msg) => render_user_message(user_msg, theme),
        Message::Assistant(assistant_msg) => render_assistant_message(assistant_msg, theme),
        Message::System(system_msg) => render_system_message(system_msg, theme),
        Message::Progress(progress_msg) => render_progress_message(progress_msg, theme),
        Message::Attachment(attachment_msg) => render_attachment_message(attachment_msg, theme),
    }
}

// ── User messages ───────────────────────────────────────────────────────

fn render_user_message<'a>(
    msg: &crate::types::message::UserMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let content_text = match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };

    // First line includes the "You: " prefix.
    let content_lines: Vec<&str> = content_text.lines().collect();
    if content_lines.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("You: ", theme.user_name),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("You: ", theme.user_name),
            Span::raw(content_lines[0].to_string()),
        ]));
        for extra in &content_lines[1..] {
            lines.push(Line::from(format!("     {}", extra)));
        }
    }

    lines
}

// ── Assistant messages ──────────────────────────────────────────────────

fn render_assistant_message<'a>(
    msg: &crate::types::message::AssistantMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Name prefix on the first line.
    let prefix = Span::styled("Claude: ", theme.assistant_name);
    let mut first_block = true;

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                let md_lines = markdown_to_lines(text, theme);
                if md_lines.is_empty() {
                    if first_block {
                        lines.push(Line::from(vec![prefix.clone()]));
                    }
                } else {
                    for (i, md_line) in md_lines.into_iter().enumerate() {
                        if i == 0 && first_block {
                            // Prepend the "Claude: " prefix to the first line.
                            let mut spans = vec![prefix.clone()];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        } else {
                            // Indent continuation lines to align with text after "Claude: "
                            let mut spans = vec![Span::raw("        ")];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        }
                    }
                }
                first_block = false;
            }

            ContentBlock::ToolUse { id: _, name, input } => {
                // Show tool invocation: tool name + abbreviated input.
                let input_summary = abbreviate_json(input, 80);
                let tool_line = Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled(format!("[{}] ", name), theme.tool_name),
                    Span::styled(input_summary, theme.dim),
                ]);
                lines.push(tool_line);
                first_block = false;
            }

            ContentBlock::ToolResult {
                tool_use_id: _,
                content,
                is_error,
            } => {
                let style = if *is_error {
                    theme.error
                } else {
                    theme.tool_result
                };
                let text = match content {
                    ToolResultContent::Text(t) => t.clone(),
                    ToolResultContent::Blocks(blocks) => blocks
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                let prefix_str = if *is_error { "  Error: " } else { "  Result: " };
                // Show first few lines of result.
                let result_lines: Vec<&str> = text.lines().take(5).collect();
                for (i, rl) in result_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::raw("        "),
                            Span::styled(prefix_str.to_string(), style),
                            Span::styled(rl.to_string(), style),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("                  "),
                            Span::styled(rl.to_string(), style),
                        ]));
                    }
                }
                let total_lines = text.lines().count();
                if total_lines > 5 {
                    lines.push(Line::from(vec![
                        Span::raw("                  "),
                        Span::styled(
                            format!("... {} more lines", total_lines - 5),
                            theme.dim,
                        ),
                    ]));
                }
                first_block = false;
            }

            ContentBlock::Thinking {
                thinking,
                signature: _,
            } => {
                // Render thinking in dim/italic, collapsible.
                if !thinking.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw(if first_block { "" } else { "        " }),
                        Span::styled("[thinking] ", theme.thinking),
                    ]));
                    // Show first 3 lines of thinking content.
                    for tl in thinking.lines().take(3) {
                        lines.push(Line::from(vec![
                            Span::raw("          "),
                            Span::styled(tl.to_string(), theme.thinking),
                        ]));
                    }
                    let thinking_line_count = thinking.lines().count();
                    if thinking_line_count > 3 {
                        lines.push(Line::from(vec![
                            Span::raw("          "),
                            Span::styled(
                                format!("... {} more lines", thinking_line_count - 3),
                                theme.dim,
                            ),
                        ]));
                    }
                }
                first_block = false;
            }

            ContentBlock::RedactedThinking { .. } => {
                lines.push(Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled("[redacted thinking]", theme.thinking),
                ]));
                first_block = false;
            }

            ContentBlock::Image { .. } => {
                lines.push(Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled("[image]", theme.dim),
                ]));
                first_block = false;
            }
        }
    }

    // If there was no content at all, at least show the name.
    if lines.is_empty() {
        lines.push(Line::from(vec![prefix]));
    }

    // Show cost if non-zero.
    if msg.cost_usd > 0.0 {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(format!("(${:.4})", msg.cost_usd), theme.dim),
        ]));
    }

    lines
}

// ── System messages ─────────────────────────────────────────────────────

fn render_system_message<'a>(
    msg: &crate::types::message::SystemMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let (prefix, style) = match &msg.subtype {
        SystemSubtype::CompactBoundary { .. } => ("--- context compacted ---", theme.dim),
        SystemSubtype::ApiError { error, .. } => {
            let _ = error;
            ("API Error: ", theme.error)
        }
        SystemSubtype::Informational { level } => match level {
            InfoLevel::Info => ("Info: ", theme.info),
            InfoLevel::Warning => ("Warning: ", theme.warning),
            InfoLevel::Error => ("Error: ", theme.error),
        },
        SystemSubtype::LocalCommand { .. } => ("$ ", theme.system_name),
        SystemSubtype::Warning => ("Warning: ", theme.warning),
    };

    if matches!(&msg.subtype, SystemSubtype::CompactBoundary { .. }) {
        lines.push(Line::from(vec![Span::styled(
            prefix.to_string(),
            style,
        )]));
    } else {
        let content_lines: Vec<&str> = msg.content.lines().collect();
        if content_lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(prefix.to_string(), style)]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(content_lines[0].to_string(), style),
            ]));
            for extra in &content_lines[1..] {
                lines.push(Line::from(Span::styled(
                    format!("  {}", extra),
                    style,
                )));
            }
        }
    }

    lines
}

// ── Progress messages ───────────────────────────────────────────────────

fn render_progress_message<'a>(
    msg: &crate::types::message::ProgressMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    // Show a spinner-like indicator with the tool_use_id and any data summary.
    let data_summary = if msg.data.is_object() {
        msg.data
            .as_object()
            .and_then(|o| o.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        msg.data.to_string()
    };

    vec![Line::from(vec![
        Span::styled("  ... ", theme.dim),
        Span::styled(data_summary, theme.dim),
    ])]
}

// ── Attachment messages ─────────────────────────────────────────────────

fn render_attachment_message<'a>(
    msg: &crate::types::message::AttachmentMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    use crate::types::message::Attachment;
    let text = match &msg.attachment {
        Attachment::EditedTextFile { path } => format!("[edited: {}]", path),
        Attachment::QueuedCommand { prompt, .. } => format!("[queued: {}]", prompt),
        Attachment::MaxTurnsReached {
            max_turns,
            turn_count,
        } => format!("[max turns reached: {}/{}]", turn_count, max_turns),
        Attachment::StructuredOutput { .. } => "[structured output]".to_string(),
        Attachment::HookStoppedContinuation => "[hook stopped continuation]".to_string(),
        Attachment::NestedMemory { path, .. } => format!("[memory: {}]", path),
        Attachment::SkillDiscovery { skills } => format!("[skills: {}]", skills.join(", ")),
    };
    vec![Line::from(Span::styled(text, theme.dim))]
}

// ── Utility ─────────────────────────────────────────────────────────────

/// Create an abbreviated string representation of a JSON value, capped at
/// `max_chars` characters.
fn abbreviate_json(value: &serde_json::Value, max_chars: usize) -> String {
    let full = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => value.to_string(),
    };
    if full.len() <= max_chars {
        full
    } else if max_chars > 3 {
        format!("{}...", &full[..max_chars - 3])
    } else {
        full[..max_chars].to_string()
    }
}
