// BEGIN generated upstream messages modules
// Rust-side message modules mirrored from upstream React components.
#[allow(dead_code)]
pub mod advisor_message;
#[allow(dead_code)]
pub mod assistant_redacted_thinking_message;
#[allow(dead_code)]
pub mod assistant_text_message;
#[allow(dead_code)]
pub mod assistant_thinking_message;
#[allow(dead_code)]
pub mod assistant_tool_use_message;
#[allow(dead_code)]
pub mod attachment_message;
#[allow(dead_code)]
pub mod collapsed_read_search_content;
#[allow(dead_code)]
pub mod compact_boundary_message;
#[allow(dead_code)]
pub mod grouped_tool_use_content;
#[allow(dead_code)]
pub mod highlighted_thinking_text;
#[allow(dead_code)]
pub mod hook_progress_message;
#[allow(dead_code)]
pub mod null_rendering_attachments;
#[allow(dead_code)]
pub mod plan_approval_message;
#[allow(dead_code)]
pub mod rate_limit_message;
#[allow(dead_code)]
pub mod shutdown_message;
#[allow(dead_code)]
pub mod system_api_error_message;
#[allow(dead_code)]
pub mod system_text_message;
#[allow(dead_code)]
pub mod task_assignment_message;
#[allow(dead_code)]
pub mod team_mem_collapsed;
#[allow(dead_code)]
pub mod team_mem_saved;
#[allow(dead_code)]
pub mod user_agent_notification_message;
#[allow(dead_code)]
pub mod user_bash_input_message;
#[allow(dead_code)]
pub mod user_bash_output_message;
#[allow(dead_code)]
pub mod user_channel_message;
#[allow(dead_code)]
pub mod user_command_message;
#[allow(dead_code)]
pub mod user_image_message;
#[allow(dead_code)]
pub mod user_local_command_output_message;
#[allow(dead_code)]
pub mod user_memory_input_message;
#[allow(dead_code)]
pub mod user_plan_message;
#[allow(dead_code)]
pub mod user_prompt_message;
#[allow(dead_code)]
pub mod user_resource_update_message;
#[allow(dead_code)]
pub mod user_teammate_message;
#[allow(dead_code)]
pub mod user_text_message;
#[allow(dead_code)]
pub mod user_tool_result_message;
// END generated upstream messages modules

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use crate::types::message::{
    ContentBlock, InfoLevel, Message, MessageContent, SystemSubtype, ToolResultContent,
};

use super::markdown::markdown_to_lines;
use super::theme::Theme;
use super::virtual_scroll::VirtualScroll;

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

fn wrap_line_to_width(line: &Line<'_>, width: u16) -> Vec<Line<'static>> {
    let max_width = usize::from(width.max(1));
    if max_width == 0 {
        return vec![Line::default()];
    }

    let mut wrapped = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for span in &line.spans {
        let style = span.style;
        let mut segment = String::new();

        for ch in span.content.chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width > 0 && current_width + ch_width > max_width {
                if !segment.is_empty() {
                    current_spans.push(Span::styled(std::mem::take(&mut segment), style));
                }
                wrapped.push(Line::from(std::mem::take(&mut current_spans)));
                current_width = 0;
            }

            segment.push(ch);
            current_width += ch_width;

            if current_width >= max_width {
                current_spans.push(Span::styled(std::mem::take(&mut segment), style));
                wrapped.push(Line::from(std::mem::take(&mut current_spans)));
                current_width = 0;
            }
        }

        if !segment.is_empty() {
            current_spans.push(Span::styled(segment, style));
        }
    }

    if current_spans.is_empty() {
        vec![Line::default()]
    } else {
        wrapped.push(Line::from(current_spans));
        wrapped
    }
}

/// Render a single message into one or more `Line`s.
///
/// `pub(super)` so that `virtual_scroll` can call it for height measurement.
pub(super) fn render_single_message<'a>(msg: &Message, theme: &Theme) -> Vec<Line<'a>> {
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
                        .map(|b| match b {
                            ContentBlock::Text { text } => text.clone(),
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
    use super::advisor_message::render_advisor_message;
    use super::assistant_redacted_thinking_message::render_assistant_redacted_thinking_message;
    use super::assistant_text_message::render_assistant_text_message;
    use super::assistant_thinking_message::render_assistant_thinking_message;
    use super::assistant_tool_use_message::render_assistant_tool_use_message;
    use super::attachment_message::render_attachment_message;
    use super::collapsed_read_search_content::render_collapsed_read_search_content;
    use super::compact_boundary_message::render_compact_boundary_message;
    use super::grouped_tool_use_content::render_grouped_tool_use_content;
    use super::highlighted_thinking_text::render_highlighted_thinking_text;
    use super::hook_progress_message::render_hook_progress_message;
    use super::null_rendering_attachments::render_null_rendering_attachments;
    use super::plan_approval_message::render_plan_approval_message;
    use super::rate_limit_message::render_rate_limit_message;
    use super::shutdown_message::render_shutdown_message;
    use super::system_api_error_message::render_system_api_error_message;
    use super::system_text_message::render_system_text_message;
    use super::task_assignment_message::render_task_assignment_message;
    use super::team_mem_collapsed::render_team_mem_collapsed;
    use super::team_mem_saved::render_team_mem_saved;
    use super::user_agent_notification_message::render_user_agent_notification_message;
    use super::user_bash_input_message::render_user_bash_input_message;
    use super::user_bash_output_message::render_user_bash_output_message;
    use super::user_channel_message::render_user_channel_message;
    use super::user_command_message::render_user_command_message;
    use super::user_image_message::render_user_image_message;
    use super::user_local_command_output_message::render_user_local_command_output_message;
    use super::user_memory_input_message::render_user_memory_input_message;
    use super::user_plan_message::render_user_plan_message;
    use super::user_prompt_message::render_user_prompt_message;
    use super::user_resource_update_message::render_user_resource_update_message;
    use super::user_teammate_message::render_user_teammate_message;
    use super::user_text_message::render_user_text_message;
    use crate::ui::theme::Theme;

    #[test]
    fn snapshot_message_component_helpers() {
        let theme = Theme::default();
        let rendered = [
            section(
                "advisor",
                render_advisor_message(Some("gpt"), "Plan next steps", "No issues", &theme),
            ),
            section(
                "assistant-redacted-thinking",
                render_assistant_redacted_thinking_message(&theme),
            ),
            section(
                "assistant-text",
                render_assistant_text_message("Hello", &theme),
            ),
            section(
                "assistant-thinking",
                render_assistant_thinking_message("I will inspect the files", &theme),
            ),
            section(
                "assistant-tool-use",
                render_assistant_tool_use_message("read_file", "path=src/main.rs", &theme),
            ),
            section(
                "attachment",
                render_attachment_message("log", "appended", &theme),
            ),
            section(
                "collapsed-read-search",
                render_collapsed_read_search_content("notes.txt", 3, &theme),
            ),
            section(
                "compact-boundary",
                render_compact_boundary_message(120, 80, &theme),
            ),
            section(
                "grouped-tool-use",
                render_grouped_tool_use_content(&["read_file", "edit_file"], &theme),
            ),
            section(
                "highlighted-thinking",
                render_highlighted_thinking_text("cache warmup", &theme),
            ),
            section(
                "hook-progress",
                render_hook_progress_message("PostToolUse", "running", &theme),
            ),
            section(
                "null-rendering-attachments",
                render_null_rendering_attachments("filtered", &theme),
            ),
            section(
                "plan-approval",
                render_plan_approval_message("refactor ui", true, &theme),
            ),
            section(
                "rate-limit",
                render_rate_limit_message("messages", 250, &theme),
            ),
            section(
                "shutdown",
                render_shutdown_message("user requested", &theme),
            ),
            section(
                "system-api-error",
                render_system_api_error_message(429, "rate limit exceeded", &theme),
            ),
            section(
                "system-text",
                render_system_text_message("note", "Context switch", &theme),
            ),
            section(
                "task-assignment",
                render_task_assignment_message("build", "alice", &theme),
            ),
            section(
                "team-mem-collapsed",
                render_team_mem_collapsed("team-a", 4, &theme),
            ),
            section(
                "team-mem-saved",
                render_team_mem_saved("/tmp/team.md", &theme),
            ),
            section(
                "user-agent-notification",
                render_user_agent_notification_message("Agent connected", &theme),
            ),
            section(
                "user-bash-input",
                render_user_bash_input_message("ls -la", &theme),
            ),
            section(
                "user-bash-output",
                render_user_bash_output_message("ls -la", "README.md", &theme),
            ),
            section(
                "user-channel",
                render_user_channel_message("default", "status ok", &theme),
            ),
            section(
                "user-command",
                render_user_command_message("build", Some("/repo"), &theme),
            ),
            section(
                "user-image",
                render_user_image_message("/tmp/a.png", "png", &theme),
            ),
            section(
                "user-local-output",
                render_user_local_command_output_message("echo hi", 0, "hi", &theme),
            ),
            section(
                "user-memory-input",
                render_user_memory_input_message("prompt", "value", &theme),
            ),
            section("user-plan", render_user_plan_message("run tests", &theme)),
            section(
                "user-prompt",
                render_user_prompt_message("Continue", &theme),
            ),
            section(
                "user-resource-update",
                render_user_resource_update_message("memory", "+1GB", &theme),
            ),
            section(
                "user-teammate",
                render_user_teammate_message("charlie", "online", &theme),
            ),
            section("user-text", render_user_text_message("Hello world", &theme)),
        ]
        .join("\n\n");

        insta::assert_snapshot!("message_component_helpers", rendered);
    }

    fn section(name: &str, body: String) -> String {
        format!("## {name}\n{body}")
    }
}
