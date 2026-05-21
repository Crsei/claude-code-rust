use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::HashMap;

use crate::ui::markdown::markdown_to_lines;
use crate::ui::messages::assistant_text_message::{classify_assistant_text, render_api_error};
use crate::ui::messages::assistant_tool_use_message::{
    render_assistant_tool_use_message, ToolUseState,
};
use crate::ui::messages::attachment_message::render_attachment_message as render_attachment_helper;
use crate::ui::messages::system_text_message::render_system_text_message;
use crate::ui::messages::user_bash_output_message::{
    render_user_bash_output_message_with_options, ShellOutputRenderOptions,
};
use crate::ui::messages::user_text_message::render_user_text_message;
use crate::ui::theme::Theme;
use crate::ui::virtual_scroll::VirtualScroll;
use cc_types::message::{
    Attachment, ContentBlock, InfoLevel, Message, MessageContent, SystemSubtype, ToolResultContent,
};

use super::file_edit_tool_updated_message::{
    render_file_edit_tool_updated_message, FileEditMessageStyle, FileEditToolUpdatedView,
};
use super::wrap::wrap_line_to_width;

#[derive(Debug, Clone, Default)]
pub(crate) struct MessageRenderContext {
    tool_uses: HashMap<String, ToolUseRenderRecord>,
    latest_shell_tool_result_id: Option<String>,
    selected_message: Option<usize>,
    selected_expanded: bool,
    cache_key: String,
}

#[derive(Debug, Clone)]
struct ToolUseRenderRecord {
    tool_name: String,
    input: serde_json::Value,
}

impl MessageRenderContext {
    pub(crate) fn cache_key(&self) -> &str {
        &self.cache_key
    }

    fn tool_use(&self, tool_use_id: &str) -> Option<&ToolUseRenderRecord> {
        self.tool_uses.get(tool_use_id)
    }

    fn shell_expanded(&self, msg_index: usize, tool_use_id: &str) -> bool {
        self.latest_shell_tool_result_id.as_deref() == Some(tool_use_id)
            || (self.selected_message == Some(msg_index) && self.selected_expanded)
    }
}

pub(crate) fn build_message_render_context(
    messages: &[Message],
    selected_message: Option<usize>,
    selected_expanded: bool,
) -> MessageRenderContext {
    let mut ctx = MessageRenderContext {
        selected_message,
        selected_expanded,
        ..MessageRenderContext::default()
    };

    for message in messages {
        match message {
            Message::Assistant(assistant) => {
                for block in &assistant.content {
                    match block {
                        ContentBlock::ToolUse { id, name, input }
                        | ContentBlock::ServerToolUse { id, name, input } => {
                            ctx.tool_uses.insert(
                                id.clone(),
                                ToolUseRenderRecord {
                                    tool_name: name.clone(),
                                    input: input.clone(),
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::User(user) => {
                if let MessageContent::Blocks(blocks) = &user.content {
                    for block in blocks {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                            if ctx
                                .tool_uses
                                .get(tool_use_id)
                                .is_some_and(ToolUseRenderRecord::is_shell)
                            {
                                ctx.latest_shell_tool_result_id = Some(tool_use_id.clone());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    ctx.cache_key = format!(
        "shell={}|selected={:?}|expanded={}",
        ctx.latest_shell_tool_result_id.as_deref().unwrap_or(""),
        ctx.selected_message,
        ctx.selected_expanded
    );
    ctx
}

impl ToolUseRenderRecord {
    fn is_shell(&self) -> bool {
        matches!(self.tool_name.as_str(), "Bash" | "PowerShell")
    }

    fn command(&self) -> String {
        self.input
            .get("command")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.tool_name.clone())
    }
}

/// Render only the visible messages into the given buffer area using virtual
/// scrolling.
///
/// `vscroll` must have been updated via `ensure_up_to_date()` before calling.
/// `scroll` is the number of rendered lines to skip from the top.
#[expect(
    clippy::too_many_arguments,
    reason = "message renderer takes explicit ratatui render state to avoid per-frame allocations"
)]
pub fn render_messages(
    messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    streaming: bool,
    scroll: usize,
    vscroll: &VirtualScroll,
    render_context: &MessageRenderContext,
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
        let mut msg_lines =
            render_single_message_wrapped(&messages[idx], idx, theme, area.width, render_context);
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

fn render_single_message_wrapped<'a>(
    msg: &Message,
    index: usize,
    theme: &Theme,
    width: u16,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    render_single_message_for_layout(msg, index, theme, width as usize, render_context)
        .into_iter()
        .flat_map(|line| wrap_line_to_width(&line, width))
        .collect()
}

pub(crate) fn render_single_message_for_layout<'a>(
    msg: &Message,
    index: usize,
    theme: &Theme,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    let mut lines = render_single_message_with_context(msg, index, theme, width, render_context);
    if render_context.selected_message == Some(index) {
        decorate_selected_message(
            &mut lines,
            msg,
            theme,
            render_context.selected_expanded,
            width,
        );
    }
    lines
}

/// Render a single message into one or more `Line`s.
///
/// `pub(super)` so that `virtual_scroll` can call it for height measurement.
pub(in crate::ui) fn render_single_message<'a>(msg: &Message, theme: &Theme) -> Vec<Line<'a>> {
    render_single_message_with_context(msg, 0, theme, 80, &MessageRenderContext::default())
}

pub(in crate::ui) fn render_single_message_with_context<'a>(
    msg: &Message,
    index: usize,
    theme: &Theme,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    match msg {
        Message::User(user_msg) => {
            render_user_message(user_msg, theme, index, width, render_context)
        }
        Message::Assistant(assistant_msg) => render_assistant_message(assistant_msg, theme),
        Message::System(system_msg) => render_system_message(system_msg, theme),
        Message::Progress(progress_msg) => render_progress_message(progress_msg, theme),
        Message::Attachment(attachment_msg) => render_attachment_message(attachment_msg, theme),
    }
}

pub(in crate::ui) fn message_copy_text(msg: &Message) -> String {
    match msg {
        Message::User(user) => message_content_copy_text(&user.content),
        Message::Assistant(assistant) => assistant
            .content
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Message::System(system) => system.content.clone(),
        Message::Progress(progress) => progress
            .data
            .get("message")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| progress.data.to_string()),
        Message::Attachment(attachment) => attachment_copy_text(&attachment.attachment),
    }
}

pub(in crate::ui) fn message_primary_reference(msg: &Message) -> Option<String> {
    match msg {
        Message::User(user) => message_content_reference(&user.content),
        Message::Assistant(assistant) => assistant.content.iter().find_map(content_block_reference),
        Message::Attachment(attachment) => attachment_reference(&attachment.attachment),
        Message::System(_) | Message::Progress(_) => None,
    }
}

fn decorate_selected_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &Message,
    theme: &Theme,
    expanded: bool,
    width: usize,
) {
    let mut header = vec![
        Span::styled(
            if expanded {
                "▼ selected"
            } else {
                "▶ selected"
            },
            theme.selected,
        ),
        Span::styled(" · c copy", theme.dim),
        Span::styled(" · enter detail", theme.dim),
    ];
    if let Some(meta) = selected_message_meta(msg) {
        header.push(Span::styled(format!(" · {meta}"), theme.dim));
    }
    lines.insert(0, Line::from(header));

    if expanded {
        lines.extend(
            message_detail_lines(msg, width)
                .into_iter()
                .map(|line| Line::from(Span::styled(format!("  {line}"), theme.dim))),
        );
    }
}

fn selected_message_meta(msg: &Message) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(ts) = concise_timestamp(msg.timestamp()) {
        parts.push(ts);
    }
    if let Some(reference) = message_primary_reference(msg) {
        parts.push(reference);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn concise_timestamp(timestamp: i64) -> Option<String> {
    if timestamp <= 0 {
        return None;
    }
    let millis = if timestamp > 10_000_000_000 {
        timestamp
    } else {
        timestamp.saturating_mul(1000)
    };
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
}

fn message_detail_lines(msg: &Message, width: usize) -> Vec<String> {
    let mut lines = vec![format!("uuid: {}", msg.uuid())];
    if let Some(ts) = concise_timestamp(msg.timestamp()) {
        lines.push(format!("time: {ts}"));
    }
    if let Some(reference) = message_primary_reference(msg) {
        lines.push(format!("ref: {reference}"));
    }
    let preview = cc_utils::messages::truncate_text(
        &message_copy_text(msg).replace('\n', " ⏎ "),
        width.saturating_sub(4).max(20),
    );
    if !preview.is_empty() {
        lines.push(format!("copy: {preview}"));
    }
    lines
}

// ── User messages ───────────────────────────────────────────────────────

fn render_user_message<'a>(
    msg: &cc_types::message::UserMessage,
    theme: &Theme,
    msg_index: usize,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let content_text = match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => {
            if let Some(tool_lines) = render_tool_result_user_message(
                msg,
                blocks,
                theme,
                msg_index,
                width,
                render_context,
            ) {
                return tool_lines;
            }
            blocks
                .iter()
                .filter_map(content_block_copy_text)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    if content_text.trim() == "[Request interrupted by user]" {
        return vec![Line::from(Span::styled(
            "Interrupted by user",
            theme.warning,
        ))];
    }

    let routed = render_user_text_message(&content_text, theme);
    if routed.is_empty() {
        return Vec::new();
    }
    let content_text = routed
        .strip_prefix("You: ")
        .unwrap_or(routed.as_str())
        .to_string();

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
    msg: &cc_types::message::UserMessage,
    blocks: &[ContentBlock],
    theme: &Theme,
    msg_index: usize,
    width: usize,
    render_context: &MessageRenderContext,
) -> Option<Vec<Line<'a>>> {
    let tool_result = blocks.iter().find_map(|block| match block {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Some((tool_use_id, content, *is_error)),
        _ => None,
    })?;

    if let Some(tool_use) = render_context.tool_use(tool_result.0) {
        if tool_use.is_shell() {
            let output = msg
                .tool_use_result
                .as_deref()
                .map(str::to_string)
                .unwrap_or_else(|| tool_result_content_text(tool_result.1));
            let expanded = render_context.shell_expanded(msg_index, tool_result.0);
            let rendered = render_user_bash_output_message_with_options(
                &tool_use.command(),
                &output,
                ShellOutputRenderOptions {
                    width: width.max(20),
                    expanded,
                    total_lines: Some(output.lines().count()),
                    total_bytes: Some(output.len()),
                    ..ShellOutputRenderOptions::default()
                },
            );
            let style = if tool_result.2 {
                theme.error
            } else {
                theme.tool_result
            };
            return Some(styled_text_lines(&rendered, style));
        }
    }

    if let Some(preview) = msg.tool_use_result.as_deref() {
        if !tool_result.2 {
            if let Some(lines) = render_file_edit_preview(preview, theme) {
                return Some(lines);
            }
        }
        return Some(styled_text_lines(
            &pretty_json_or_raw(preview),
            theme.tool_result,
        ));
    }

    let text = tool_result_content_text(tool_result.1);
    let style = if tool_result.2 {
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
                ContentBlock::Image { source } => Some(image_reference(source)),
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

fn plain_text_to_lines<'a>(text: &str, style: Style) -> Vec<Line<'a>> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

fn api_error_display_text(text: &str) -> String {
    if let Some(error) = classify_assistant_text(text) {
        render_api_error(&error)
    } else {
        format!("Error occurred: {text}")
    }
}

// ── Assistant messages ──────────────────────────────────────────────────

fn render_assistant_message<'a>(
    msg: &cc_types::message::AssistantMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Name prefix on the first line.
    let prefix = Span::styled("Claude: ", theme.assistant_name);
    let mut first_block = true;

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                // If this is an API error message, use error classification
                // instead of standard markdown rendering.
                if msg.is_api_error_message {
                    let error_text = api_error_display_text(text);
                    let style = theme.error;
                    if first_block {
                        let mut spans = vec![prefix.clone()];
                        spans.push(Span::styled(error_text, style));
                        lines.push(Line::from(spans));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("        "),
                            Span::styled(error_text, style),
                        ]));
                    }
                    first_block = false;
                    continue;
                }
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
                let input_json = serde_json::to_string(input).unwrap_or_else(|_| input.to_string());
                let rendered = render_assistant_tool_use_message(
                    name,
                    &input_json,
                    ToolUseState::InProgress,
                    false,
                    theme,
                );
                for (i, line) in rendered.lines().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(if first_block && i == 0 {
                            ""
                        } else {
                            "        "
                        }),
                        Span::styled(line.to_string(), theme.tool_name),
                    ]));
                }
                first_block = false;
            }

            ContentBlock::ServerToolUse { id: _, name, input } => {
                let input_json = serde_json::to_string(input).unwrap_or_else(|_| input.to_string());
                let rendered = render_assistant_tool_use_message(
                    name,
                    &input_json,
                    ToolUseState::InProgress,
                    false,
                    theme,
                );
                for (i, line) in rendered.lines().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(if first_block && i == 0 {
                            ""
                        } else {
                            "        "
                        }),
                        Span::styled(format!("server: {line}"), theme.tool_name),
                    ]));
                }
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
                            ContentBlock::Image { source } => image_reference(source),
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

            ContentBlock::Image { source } => {
                lines.push(Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled(image_reference(source), theme.dim),
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
    msg: &cc_types::message::SystemMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let (prefix, style) = match &msg.subtype {
        SystemSubtype::CompactBoundary { .. } => ("context compacted", theme.dim),
        SystemSubtype::MicrocompactBoundary { .. } => ("context microcompacted", theme.dim),
        SystemSubtype::ApiError { .. } => ("", theme.error),
        SystemSubtype::Informational { level } => match level {
            InfoLevel::Info => ("Info: ", theme.info),
            InfoLevel::Warning => ("Warning: ", theme.warning),
            InfoLevel::Error => ("Error: ", theme.error),
        },
        SystemSubtype::LocalCommand { .. } => ("$ ", theme.system_name),
        SystemSubtype::Warning => ("Warning: ", theme.warning),
    };

    if let SystemSubtype::ApiError {
        retry_attempt,
        max_retries: _,
        retry_in_ms,
        error,
    } = &msg.subtype
    {
        let detail = if msg.content.trim().is_empty() {
            error.message.as_str()
        } else {
            msg.content.trim()
        };
        let rendered = if *retry_in_ms > 0 {
            render_system_text_message(
                "api_error",
                &format!("retry_attempt={} {}", retry_attempt, detail),
                theme,
            )
        } else {
            render_system_text_message("api_error", detail, theme)
        };
        return plain_text_to_lines(&rendered, theme.error);
    }

    if matches!(
        &msg.subtype,
        SystemSubtype::CompactBoundary { .. } | SystemSubtype::MicrocompactBoundary { .. }
    ) {
        lines.push(Line::from(vec![Span::styled(
            compact_boundary_summary(&msg.subtype, prefix),
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
                lines.push(Line::from(Span::styled(format!("  {}", extra), style)));
            }
        }
    }

    lines
}

// ── Progress messages ───────────────────────────────────────────────────

fn render_progress_message<'a>(
    msg: &cc_types::message::ProgressMessage,
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
    msg: &cc_types::message::AttachmentMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    use cc_types::message::Attachment;
    let text = match &msg.attachment {
        Attachment::EditedTextFile { path } => format!("[edited: {}]", path),
        Attachment::QueuedCommand { prompt, .. } => render_attachment_helper(
            "queued_command",
            &serde_json::json!({ "prompt": prompt }).to_string(),
            theme,
        ),
        Attachment::MaxTurnsReached {
            max_turns,
            turn_count,
        } => format!("[max turns reached: {}/{}]", turn_count, max_turns),
        Attachment::StructuredOutput { .. } => "[structured output]".to_string(),
        Attachment::HookStoppedContinuation => "[hook stopped continuation]".to_string(),
        Attachment::NestedMemory { path, .. } => {
            render_attachment_helper("nested_memory", path, theme)
        }
        Attachment::SkillDiscovery { skills } => render_attachment_helper(
            "skill_discovery",
            &serde_json::to_string(skills).unwrap_or_else(|_| "[]".to_string()),
            theme,
        ),
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

fn tool_input_summary(name: &str, input: &serde_json::Value, max_chars: usize) -> String {
    if let Some(primary) = tool_primary_input(name, input) {
        let json = abbreviate_json(input, max_chars);
        return format!("{primary} {json}").trim().to_string();
    }
    abbreviate_json(input, max_chars)
}

fn tool_primary_input(name: &str, input: &serde_json::Value) -> Option<String> {
    let key = match name {
        "Read" | "Edit" | "Write" => "file_path",
        "NotebookEdit" => "notebook_path",
        "Bash" => "command",
        "Grep" | "Glob" => "pattern",
        "WebFetch" => "url",
        "WebSearch" => "query",
        "Task" | "Agent" => "prompt",
        _ => return None,
    };
    input
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| {
            let label = if key.ends_with("path") || key == "file_path" {
                "path"
            } else {
                key
            };
            format!("{label}={value}")
        })
}

fn message_content_copy_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => strip_system_reminders(text),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn message_content_reference(content: &MessageContent) -> Option<String> {
    match content {
        MessageContent::Text(_) => None,
        MessageContent::Blocks(blocks) => blocks.iter().find_map(content_block_reference),
    }
}

fn content_block_copy_text(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::Text { text } => Some(strip_system_reminders(text)),
        ContentBlock::ConnectorText { connector_text, .. } => Some(connector_text.clone()),
        ContentBlock::ToolUse { name, input, .. }
        | ContentBlock::ServerToolUse { name, input, .. } => {
            tool_primary_input(name, input).or_else(|| Some(input.to_string()))
        }
        ContentBlock::ToolResult { content, .. } => Some(tool_result_content_text(content)),
        ContentBlock::Image { source } => Some(image_reference(source)),
        ContentBlock::Thinking { thinking, .. } => (!thinking.is_empty()).then(|| thinking.clone()),
        ContentBlock::RedactedThinking { .. } => Some("[redacted thinking]".to_string()),
    }
}

fn content_block_reference(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::ToolUse { name, input, .. }
        | ContentBlock::ServerToolUse { name, input, .. } => tool_primary_input(name, input),
        ContentBlock::Image { source } => Some(image_reference(source)),
        ContentBlock::ToolResult { content, .. } => match content {
            ToolResultContent::Text(_) => None,
            ToolResultContent::Blocks(blocks) => blocks.iter().find_map(content_block_reference),
        },
        _ => None,
    }
}

fn image_reference(source: &cc_types::message::ImageSource) -> String {
    format!(
        "[image: {}, {} chars]",
        source.media_type,
        source.data.len()
    )
}

fn attachment_copy_text(attachment: &Attachment) -> String {
    match attachment {
        Attachment::EditedTextFile { path } => path.clone(),
        Attachment::QueuedCommand { prompt, .. } => prompt.clone(),
        Attachment::MaxTurnsReached {
            max_turns,
            turn_count,
        } => format!("Max turns reached: {turn_count}/{max_turns}"),
        Attachment::StructuredOutput { data } => data.to_string(),
        Attachment::HookStoppedContinuation => "Hook stopped continuation".to_string(),
        Attachment::NestedMemory { path, content } => format!("{path}\n{content}"),
        Attachment::SkillDiscovery { skills } => skills.join(", "),
    }
}

fn attachment_reference(attachment: &Attachment) -> Option<String> {
    match attachment {
        Attachment::EditedTextFile { path } | Attachment::NestedMemory { path, .. } => {
            Some(format!("path={path}"))
        }
        Attachment::QueuedCommand { prompt, .. } => Some(format!(
            "prompt={}",
            cc_utils::messages::truncate_text(prompt, 48)
        )),
        _ => None,
    }
}

fn strip_system_reminders(text: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut rest = text.trim_start();
    while let Some(after_open) = rest.strip_prefix(OPEN) {
        let Some(end) = after_open.find(CLOSE) else {
            break;
        };
        rest = after_open[end + CLOSE.len()..].trim_start();
    }
    rest.to_string()
}

fn compact_boundary_summary(subtype: &SystemSubtype, prefix: &str) -> String {
    match subtype {
        SystemSubtype::CompactBoundary {
            compact_metadata: Some(meta),
        } => format!(
            "--- {prefix}: {} → {} tokens ---",
            meta.pre_compact_token_count, meta.post_compact_token_count
        ),
        SystemSubtype::MicrocompactBoundary {
            microcompact_metadata: Some(meta),
        } => format!(
            "--- {prefix}: saved {} tokens from {} tool results ---",
            meta.tokens_saved,
            meta.compacted_tool_ids.len()
        ),
        _ => format!("--- {prefix} ---"),
    }
}

#[cfg(test)]
mod tests {
    use super::{message_copy_text, message_primary_reference, render_single_message};
    use crate::ui::diff::file_edit_diff::unified_hunk_lines_from_edit;
    use crate::ui::theme::Theme;
    use cc_types::message::{
        ApiErrorInfo, AssistantMessage, CompactMetadata, ContentBlock, ImageSource, Message,
        MessageContent, SystemMessage, SystemSubtype, ToolResultContent, UserMessage,
    };
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

    #[test]
    fn messages_render_path_image_compact_and_interrupt_summaries() {
        let theme = Theme::default();
        let image = ImageSource {
            source_type: "base64".to_string(),
            media_type: "image/png".to_string(),
            data: "abcdef".to_string(),
        };
        let assistant = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 1_700_000_000,
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::ToolUse {
                    id: "toolu_read".to_string(),
                    name: "Read".to_string(),
                    input: json!({ "file_path": "src/main.rs" }),
                },
                ContentBlock::Image {
                    source: image.clone(),
                },
            ],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        });
        let rendered = lines_to_text(render_single_message(&assistant, &theme));

        assert!(rendered.contains("path=src/main.rs"));
        assert!(rendered.contains("[image: image/png, 6 chars]"));
        assert_eq!(
            message_primary_reference(&assistant).as_deref(),
            Some("path=src/main.rs")
        );
        assert!(message_copy_text(&assistant).contains("[image: image/png, 6 chars]"));

        let compact = Message::System(SystemMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 1_700_000_000,
            subtype: SystemSubtype::CompactBoundary {
                compact_metadata: Some(CompactMetadata {
                    pre_compact_token_count: 12_000,
                    post_compact_token_count: 4_000,
                    preserved_segment: None,
                }),
            },
            content: String::new(),
        });
        assert!(lines_to_text(render_single_message(&compact, &theme))
            .contains("context compacted: 12000 → 4000 tokens"));

        let interrupted = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text("[Request interrupted by user]".to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });
        assert_eq!(
            lines_to_text(render_single_message(&interrupted, &theme)),
            "Interrupted by user"
        );
    }

    #[test]
    fn assistant_api_error_runtime_path_uses_error_occurred_prefix() {
        let message = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: "API error: Provider openrouter error (HTTP 429): rate limit exceeded"
                    .to_string(),
            }],
            usage: None,
            stop_reason: Some("error".to_string()),
            is_api_error_message: true,
            api_error: Some(
                "Provider openrouter error (HTTP 429): rate limit exceeded".to_string(),
            ),
            cost_usd: 0.0,
        });

        let rendered = lines_to_text(render_single_message(&message, &Theme::default()));

        assert!(rendered.contains(
            "Claude: Error occurred: API error: Provider openrouter error (HTTP 429): rate limit exceeded"
        ));
    }

    #[test]
    fn system_api_error_runtime_path_uses_error_occurred_prefix() {
        let message = Message::System(SystemMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            subtype: SystemSubtype::ApiError {
                retry_attempt: 1,
                max_retries: 3,
                retry_in_ms: 0,
                error: ApiErrorInfo {
                    status: Some(403),
                    message: "Provider proxy error (HTTP 403): forbidden".to_string(),
                },
            },
            content: String::new(),
        });

        let rendered = lines_to_text(render_single_message(&message, &Theme::default()));

        assert_eq!(
            rendered,
            "Error occurred: Provider proxy error (HTTP 403): forbidden"
        );
    }

    fn lines_to_text(lines: Vec<ratatui::text::Line<'_>>) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
