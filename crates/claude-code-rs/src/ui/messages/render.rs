use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::types::message::{
    ContentBlock, InfoLevel, Message, MessageContent, SystemSubtype, ToolResultContent,
};
use crate::ui::markdown::markdown_to_lines;
use crate::ui::theme::Theme;
use crate::ui::virtual_scroll::VirtualScroll;

use super::file_edit_tool_updated_message::{
    FileEditMessageStyle, FileEditToolUpdatedView, render_file_edit_tool_updated_message,
};
use super::wrap::wrap_line_to_width;

/// Render only the visible messages into the given buffer area using virtual
/// scrolling.
///
/// `vscroll` must have been updated via `ensure_up_to_date()` before calling.
/// `scroll` is the number of rendered lines to skip from the top.
pub fn render_messages(
    messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    streaming: bool,
    scroll: usize,
    vscroll: &VirtualScroll,
) {
    if area.height == 0 || area.width == 0 || messages.is_empty() {
        return;
    }

    let viewport_h = area.height as usize;
    let (start, end) = vscroll.visual_range(scroll, viewport_h);

    // Where the first visible message starts in wrapped visual line space.
    let first_offset = vscroll.visual_offset_of(start);
    // How many lines to skip inside the first visible message.
    let skip_in_first = scroll.saturating_sub(first_offset);

    let mut y = 0usize; // current row in the viewport

    for idx in start..end.min(messages.len()) {
        let mut msg_lines = render_single_message_wrapped(&messages[idx], theme, area.width);
        if streaming
            && idx == messages.len().saturating_sub(1)
            && matches!(&messages[idx], Message::Assistant(_))
        {
            if let Some(last_line) = msg_lines.last_mut() {
                last_line.spans.push(Span::styled(" ▌", theme.dim));
            }
        }

        // Separator blank line (between messages, not after last)
        let has_sep = idx < messages.len() - 1;
        let total_for_msg = msg_lines.len() + if has_sep { 1 } else { 0 };

        let skip = if idx == start { skip_in_first } else { 0 };

        for li in skip..total_for_msg {
            if y >= viewport_h {
                return;
            }
            let line = if li < msg_lines.len() {
                &msg_lines[li]
            } else {
                // separator blank line
                &Line::default()
            };
            buf.set_line(area.x, area.y + y as u16, line, area.width);
            y += 1;
        }
    }
}

fn render_single_message_wrapped<'a>(msg: &Message, theme: &Theme, width: u16) -> Vec<Line<'a>> {
    render_single_message(msg, theme)
        .into_iter()
        .flat_map(|line| wrap_line_to_width(&line, width))
        .collect()
}

/// Render a single message into one or more `Line`s.
///
/// `pub(super)` so that `virtual_scroll` can call it for height measurement.
pub(in crate::ui) fn render_single_message<'a>(msg: &Message, theme: &Theme) -> Vec<Line<'a>> {
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
        MessageContent::Blocks(blocks) => {
            if let Some(tool_lines) = render_tool_result_user_message(msg, blocks, theme) {
                return tool_lines;
            }
            blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    // First line includes the "You: " prefix.
    let content_lines: Vec<&str> = content_text.lines().collect();
    if content_lines.is_empty() {
        lines.push(Line::from(vec![Span::styled("You: ", theme.user_name)]));
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

fn render_tool_result_user_message<'a>(
    msg: &crate::types::message::UserMessage,
    blocks: &[ContentBlock],
    theme: &Theme,
) -> Option<Vec<Line<'a>>> {
    let tool_result = blocks.iter().find_map(|block| match block {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => Some((content, *is_error)),
        _ => None,
    })?;

    if let Some(preview) = msg.tool_use_result.as_deref() {
        if !tool_result.1 {
            if let Some(lines) = render_file_edit_preview(preview, theme) {
                return Some(lines);
            }
        }
        return Some(styled_text_lines(
            &pretty_json_or_raw(preview),
            theme.tool_result,
        ));
    }

    let text = tool_result_content_text(tool_result.0);
    let style = if tool_result.1 {
        theme.error
    } else {
        theme.tool_result
    };
    Some(styled_text_lines(&text, style))
}

fn render_file_edit_preview<'a>(preview: &str, theme: &Theme) -> Option<Vec<Line<'a>>> {
    let value = serde_json::from_str::<serde_json::Value>(preview).ok()?;
    if value.get("kind").and_then(|v| v.as_str()) != Some("file_edit") {
        return None;
    }
    let file_path = value.get("path").and_then(|v| v.as_str())?.to_string();
    let hunk_lines = value
        .get("hunk_lines")?
        .as_array()?
        .iter()
        .filter_map(|line| line.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    if hunk_lines.is_empty() {
        return None;
    }

    let rendered = render_file_edit_tool_updated_message(&FileEditToolUpdatedView {
        file_path,
        hunk_lines,
        style: FileEditMessageStyle::Regular,
        verbose: true,
        preview_hint: None,
        width: 100,
        max_lines: 48,
    });
    Some(
        rendered
            .lines()
            .enumerate()
            .map(|(idx, line)| {
                let style = if idx == 0 {
                    theme.tool_name
                } else {
                    theme.tool_result
                };
                Line::from(Span::styled(line.to_string(), style))
            })
            .collect(),
    )
}

fn tool_result_content_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => text.clone(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Image { source } => Some(format!("[image: {}]", source.media_type)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn pretty_json_or_raw(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| raw.to_string())
}

fn styled_text_lines<'a>(text: &str, style: Style) -> Vec<Line<'a>> {
    if text.is_empty() {
        return vec![Line::from(Span::styled("<empty tool output>", style))];
    }
    text.lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
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

            ContentBlock::ConnectorText { connector_text, .. } => {
                let md_lines = markdown_to_lines(connector_text, theme);
                if md_lines.is_empty() {
                    if first_block {
                        lines.push(Line::from(vec![prefix.clone()]));
                    }
                } else {
                    for (i, md_line) in md_lines.into_iter().enumerate() {
                        if i == 0 && first_block {
                            let mut spans = vec![prefix.clone()];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        } else {
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

            ContentBlock::ServerToolUse { id: _, name, input } => {
                let input_summary = abbreviate_json(input, 80);
                let tool_line = Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled(format!("[server:{}] ", name), theme.tool_name),
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
                        .map(|b| match b {
                            ContentBlock::Text { text } => text.clone(),
                            ContentBlock::ConnectorText { connector_text, .. } => {
                                connector_text.clone()
                            }
                            ContentBlock::Image { source } => {
                                format!("[image: {}]", source.media_type)
                            }
                            _ => "[...]".to_string(),
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
                        Span::styled(format!("... {} more lines", total_lines - 5), theme.dim),
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
        SystemSubtype::MicrocompactBoundary { .. } => ("--- context microcompacted ---", theme.dim),
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

    if matches!(
        &msg.subtype,
        SystemSubtype::CompactBoundary { .. } | SystemSubtype::MicrocompactBoundary { .. }
    ) {
        lines.push(Line::from(vec![Span::styled(prefix.to_string(), style)]));
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
                lines.push(Line::from(Span::styled(format!("  {}", extra), style)));
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

#[cfg(test)]
mod tests {
    use super::render_single_message;
    use crate::types::message::{
        ContentBlock, Message, MessageContent, ToolResultContent, UserMessage,
    };
    use crate::ui::diff::file_edit_diff::unified_hunk_lines_from_edit;
    use crate::ui::theme::Theme;
    use serde_json::json;

    #[test]
    fn renders_file_edit_tool_preview_from_tool_use_result() {
        let hunk_lines = unified_hunk_lines_from_edit(
            "src/lib.rs",
            "fn main() {\n    old_call();\n}\n",
            "fn main() {\n    new_call();\n}\n",
        );
        let preview = json!({
            "kind": "file_edit",
            "path": "src/lib.rs",
            "output": "Successfully replaced 1 occurrence(s) in src/lib.rs",
            "replacements": 1,
            "hunk_lines": hunk_lines,
        })
        .to_string();
        let message = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_edit".to_string(),
                content: ToolResultContent::Text("The file was updated.".to_string()),
                is_error: false,
            }]),
            is_meta: true,
            tool_use_result: Some(preview),
            source_tool_assistant_uuid: None,
        });

        let rendered = render_single_message(&message, &Theme::default())
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Added 1 line, removed 1 line"));
        assert!(rendered.contains("file: src/lib.rs"));
        assert!(rendered.contains("old_call();"));
        assert!(rendered.contains("new_call();"));
    }
}
