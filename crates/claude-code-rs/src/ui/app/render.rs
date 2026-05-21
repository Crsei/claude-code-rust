use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Widget;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::App;
use crate::ui::agents::agents_menu::AgentsMenuState;
use crate::ui::bottom_pane::BottomPaneHeights;
use crate::ui::command_palette::CommandPalette;
use crate::ui::command_surface::CommandSurface;
use crate::ui::history_search_dialog::HistorySearchDialog;
use crate::ui::keyboard_shortcut::{render_shortcut_hints, ShortcutHint};
use crate::ui::messages::render_messages;
use crate::ui::notifications::in_app::{NotificationPriority, NotificationTone};
use crate::ui::overlays::{render_centered_dialog_lines, CenteredOverlayFrame};
use crate::ui::prompt_input::PromptInputRenderContext;
use crate::ui::theme::{Theme, ThemeColors};
use crate::ui::transcript::{self, TranscriptInputMode, ViewMode};
use crate::ui::welcome;

/// Upper cap on the number of stdout lines the status-line runner is
/// allowed to take up. Arbitrary but small so a runaway script can't
/// eat the messages pane.
const STATUS_LINE_MAX_LINES: usize = 3;

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        if size.width < 10 || size.height < 4 {
            return;
        }

        if self.workspace_trust_pending {
            render_workspace_trust_prompt(
                size,
                frame.buffer_mut(),
                &self.cwd,
                self.workspace_trust_selection,
            );
            return;
        }

        // Transcript / focus modes have a different chrome; dispatch
        // before we compute the prompt-mode layout.
        if self.view_mode.is_transcript_like() {
            self.render_transcript(frame, size);
            return;
        }

        // Kick the status-line runner before computing layout so this
        // frame already has a chance to show a refreshed output. The
        // runner throttles refreshes internally.
        self.trigger_status_refresh();
        let status_output = self.status_line_runner.latest();
        let custom_lines: Vec<String> =
            if status_output.is_usable() && self.status_line_settings.is_command_mode() {
                status_output.lines(STATUS_LINE_MAX_LINES)
            } else {
                Vec::new()
            };

        let current_notification = self.current_notification();
        let immediate_notification = current_notification
            .is_some_and(|notification| notification.priority == NotificationPriority::Immediate);
        let spinner_height = if self.is_streaming && !immediate_notification {
            1u16
        } else {
            0
        };
        let suggestion_height =
            if !self.is_streaming && self.suggestions.is_some() && !immediate_notification {
                1u16
            } else {
                0
            };
        let command_palette_height = self
            .command_palette
            .preferred_height()
            .min(size.height.saturating_sub(4));
        let completion_popup_height = self.completion_popup_height();
        let cwd_path = std::path::Path::new(&self.cwd);
        let command_arg_help_height = 0;
        let paste_notice_height =
            u16::from(self.prompt.large_paste_notice().is_some() && !immediate_notification);
        let notification_height = u16::from(current_notification.is_some());
        let agent_footer_height = u16::from(self.agent_footer_visible() && !immediate_notification);
        let input_height = 1u16;
        let status_height = if custom_lines.is_empty() {
            1u16
        } else {
            custom_lines.len().min(STATUS_LINE_MAX_LINES) as u16
        };
        let bottom_pane = BottomPaneHeights {
            spinner: spinner_height,
            suggestions: suggestion_height,
            paste_notice: paste_notice_height,
            input: input_height,
            completion_popup: completion_popup_height,
            command_palette: command_palette_height,
            command_arg_help: command_arg_help_height,
            notification: notification_height,
            agent_footer: agent_footer_height,
            status: status_height,
        };
        let bottom_height = bottom_pane.total();
        let max_content_height = size.height.saturating_sub(bottom_height);
        let content_height = if self.show_welcome {
            welcome::welcome_height_for(size.width).min(max_content_height)
        } else {
            let message_render_context =
                super::super::messages::build_message_render_context_with_options(
                    &self.messages,
                    self.selected_message,
                    self.selected_message_expanded,
                    super::super::messages::MessageRenderOptions {
                        verbose: self.verbose,
                        is_transcript_mode: false,
                        show_all_in_transcript: false,
                    },
                );
            self.vscroll.ensure_up_to_date(
                &self.messages,
                size.width,
                &self.theme,
                &message_render_context,
            );
            self.vscroll
                .total_visual_lines()
                .min(max_content_height as usize) as u16
        };

        let chunks = Layout::vertical([
            Constraint::Length(content_height),
            Constraint::Length(bottom_height),
            Constraint::Min(0),
        ])
        .split(size);

        let message_area = chunks[0];
        let bottom_area = chunks[1];

        if self.show_welcome {
            // Welcome screen
            welcome::render_welcome(
                message_area,
                frame.buffer_mut(),
                env!("CARGO_PKG_VERSION"),
                &self.model_name,
                &self.session_id,
                &self.cwd,
            );
        } else {
            // Messages (virtual scroll)
            let message_render_context =
                super::super::messages::build_message_render_context_with_options(
                    &self.messages,
                    self.selected_message,
                    self.selected_message_expanded,
                    super::super::messages::MessageRenderOptions {
                        verbose: self.verbose,
                        is_transcript_mode: false,
                        show_all_in_transcript: false,
                    },
                );
            self.vscroll.ensure_up_to_date(
                &self.messages,
                message_area.width,
                &self.theme,
                &message_render_context,
            );
            let total = self.vscroll.total_visual_lines();
            let max_scroll = total.saturating_sub(message_area.height as usize);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }

            render_messages(
                &self.messages,
                message_area,
                frame.buffer_mut(),
                &self.theme,
                self.is_streaming,
                self.scroll_offset,
                &self.vscroll,
                &message_render_context,
            );
        }

        // Bottom area: spinner + suggestions + paste_notice + input + completion_popup + palette + arg_help + notification + agent_footer + status
        let has_suggestions = suggestion_height > 0;
        let bottom_chunks = bottom_pane.split(bottom_area);

        if self.is_streaming && bottom_chunks.spinner.height > 0 {
            self.spinner_state
                .render(bottom_chunks.spinner, frame.buffer_mut(), &self.theme);
        }

        if has_suggestions {
            self.render_suggestions(bottom_chunks.suggestions, frame.buffer_mut());
        }

        if paste_notice_height > 0 {
            self.render_paste_notice(bottom_chunks.paste_notice, frame.buffer_mut());
        }

        let argument_hint = CommandPalette::argument_hint(&self.prompt.input, cwd_path);
        let placeholder = self.prompt_placeholder();
        let mode_indicator = self.prompt_mode_indicator();
        self.prompt.render_with_context(
            bottom_chunks.input,
            frame.buffer_mut(),
            &self.theme,
            PromptInputRenderContext {
                hint: argument_hint.as_deref(),
                placeholder: Some(placeholder),
                mode_indicator: Some(mode_indicator),
            },
        );

        // Render completion popup (when active and command palette is not active)
        if self.completion_state.active && !self.command_palette.active() {
            self.render_completion_popup(bottom_chunks.completion_popup, frame.buffer_mut());
        }

        self.command_palette.render(
            bottom_chunks.command_palette,
            frame.buffer_mut(),
            &self.theme,
        );

        if notification_height > 0 {
            self.render_notification(bottom_chunks.notification, frame.buffer_mut());
        }

        if agent_footer_height > 0 {
            self.render_agent_footer(bottom_chunks.agent_footer, frame.buffer_mut());
        }

        self.render_status_bar(bottom_chunks.status, frame.buffer_mut(), &custom_lines);

        if let Some(ref surface) = self.command_surface {
            render_command_surface_overlay(
                surface,
                size,
                frame.buffer_mut(),
                &self.theme,
                self.design_theme_provider.colors(),
            );
        }

        if let Some(ref dialog) = self.history_search_dialog {
            render_history_search_overlay(
                dialog,
                size,
                frame.buffer_mut(),
                &self.theme,
                self.design_theme_provider.colors(),
            );
        }

        let current_thread_id = self.current_agent_thread_id().to_string();
        if let Some(ref mut dialog) = self.agent_tree_dialog {
            render_agent_tree_overlay(
                dialog,
                &self.agent_nav,
                &current_thread_id,
                size,
                frame.buffer_mut(),
                &self.theme,
                self.design_theme_provider.colors(),
            );
        }

        if let Some(ref dialog) = self.permission_dialog {
            dialog.render(size, frame.buffer_mut(), &self.theme);
        }

        if let Some(ref dialog) = self.question_dialog {
            dialog.render(size, frame.buffer_mut(), &self.theme);
        }

        if let Some(ref dialog) = self.bypass_permissions_mode_dialog {
            dialog.render(size, frame.buffer_mut(), &self.theme);
        }
    }

    fn render_suggestions(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }
        if let Some(suggestions) = &self.suggestions {
            let hint: String = suggestions
                .iter()
                .take(3)
                .enumerate()
                .map(|(i, s)| format!("[{}{}] {}", s.category.icon(), i + 1, s.text))
                .collect::<Vec<_>>()
                .join("  ");
            let line = Line::from(Span::styled(
                hint,
                ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
            ));
            buf.set_line(area.x, area.y, &line, area.width);
        }
    }

    fn render_paste_notice(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }
        if let Some(notice) = self.prompt.large_paste_notice() {
            let line = Line::from(vec![
                Span::styled(" paste ", self.theme.info),
                Span::styled(notice.to_string(), self.theme.dim),
            ]);
            buf.set_line(area.x, area.y, &line, area.width);
        }
    }

    fn render_notification(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 {
            return;
        }
        let Some(notification) = self.current_notification() else {
            return;
        };
        let line = if let Some(spans) = notification.rendered.as_ref() {
            Line::from(spans.clone())
        } else {
            Line::from(vec![
                Span::styled(" notice ", self.theme.info),
                Span::styled(
                    notification.text.as_str(),
                    notification_style(notification.tone, &self.theme),
                ),
            ])
        };
        buf.set_line(area.x, area.y, &line, area.width);
    }

    fn render_agent_footer(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height == 0 || !self.agent_footer_visible() {
            return;
        }

        let active_label = self
            .current_agent_label()
            .unwrap_or_else(|| "Primary".to_string());
        let line = Line::from(vec![
            Span::styled(" agents ", self.theme.info),
            Span::styled(format!("{active_label} | "), self.theme.dim),
            Span::styled("Ctrl+X Ctrl+A open tree", self.theme.dim),
        ]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    fn prompt_placeholder(&self) -> &'static str {
        if self.is_streaming {
            "Waiting for response..."
        } else if self.prompt.input.starts_with('/') || self.command_palette.active() {
            "Type a command"
        } else if self.vim.enabled {
            "Press i to insert, / for commands, Ctrl+R for history"
        } else {
            "Message cc-rust, / for commands"
        }
    }

    fn prompt_mode_indicator(&self) -> &'static str {
        if self.is_streaming {
            "BUSY"
        } else if self.prompt.input.starts_with('/') || self.command_palette.active() {
            "CMD"
        } else if self.vim.enabled {
            self.vim.mode.indicator()
        } else {
            "INS"
        }
    }

    fn render_status_bar(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        custom_lines: &[String],
    ) {
        if area.height == 0 {
            return;
        }

        // 1. Custom scriptable status-line (issue #11); when present, take
        //    full priority over the built-in footer. Padding from settings.
        if !custom_lines.is_empty() {
            let padding = self.status_line_settings.padding.unwrap_or(0) as usize;
            let pad_str: String = " ".repeat(padding);
            for (i, text) in custom_lines.iter().enumerate() {
                if (i as u16) >= area.height {
                    break;
                }
                let line = Line::from(vec![Span::styled(
                    format!("{}{}", pad_str, text),
                    self.theme.dim,
                )]);
                buf.set_line(area.x, area.y + i as u16, &line, area.width);
            }
            return;
        }

        // 2. Built-in default footer; also the fallback when the runner
        //    errors or the script is disabled.
        let msg_count = self.messages.len();
        let mode = if self.is_streaming {
            "streaming"
        } else {
            "ready"
        };

        let mut parts = Vec::new();
        if !self.model_name.is_empty() {
            let short_model = self
                .model_name
                .strip_prefix("claude-")
                .unwrap_or(&self.model_name);
            let short_model = short_model.split('-').take(2).collect::<Vec<_>>().join("-");
            parts.push(short_model);
        }
        if let Some(active_agent) = self.current_agent_label() {
            parts.push(format!("agent:{active_agent}"));
        }
        parts.push(format!("{} msgs", msg_count));
        if self.session_cost_usd > 0.0 {
            parts.push(format!("${:.4}", self.session_cost_usd));
        }
        if self.vim.enabled {
            parts.push(format!("vim:{}", self.vim.mode.indicator()));
        }
        if !self.permission_mode_label.is_empty() {
            parts.push(format!("perm:{}", self.permission_mode_label));
        }
        if !self.sandbox_label.is_empty() {
            parts.push(format!("sandbox:{}", self.sandbox_label));
        }
        if let Some(effort) = &self.effort_label {
            parts.push(format!("effort:{effort}"));
        }
        if let Some(remote) = &self.remote_indicator_label {
            parts.push(format!("remote:{remote}"));
        }
        parts.push(mode.to_string());
        let hints = if self.is_streaming {
            render_shortcut_hints(&[ShortcutHint::new("Ctrl+C", "abort")])
        } else {
            render_shortcut_hints(&[
                ShortcutHint::new("/", "commands"),
                ShortcutHint::new("Ctrl+R", "history"),
                ShortcutHint::new("Ctrl+O", "views"),
                ShortcutHint::new("Ctrl+C", "quit"),
            ])
        };
        parts.push(hints);

        // If the runner reported an error, surface a quiet marker so the
        // user knows to run `/statusline status` to see why.
        let latest = self.status_line_runner.latest();
        if latest.error.is_some() && self.status_line_settings.is_command_mode() {
            parts.push("statusline:err".to_string());
        }

        // Voice push-to-talk status (issue #13). Shown as the tail entry
        // so it's the most prominent thing while recording.
        if self.voice_enabled && !self.voice_supported {
            parts.push("voice:unsupported".to_string());
        } else if let Some(v) = &self.voice {
            if let Some(label) = v.status_line() {
                parts.push(label);
            }
        }

        let status_text = format!(" {}", parts.join(" | "));
        let line = Line::from(vec![Span::styled(status_text, self.theme.dim)]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    // Transcript rendering (issue #12)

    fn render_transcript(&mut self, frame: &mut Frame, size: Rect) {
        if matches!(self.view_mode, ViewMode::Focus) {
            self.render_focus_view(frame, size);
            return;
        }

        // Focus mode hides all chrome and uses the full height for body;
        // Transcript mode reserves 1 line for header + 1 for footer.
        let chrome = 1u16;
        let header_height = chrome;
        let footer_height = chrome;
        let body_height = size.height.saturating_sub(header_height + footer_height);

        let rows = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(body_height),
            Constraint::Length(footer_height),
        ])
        .split(size);

        let body_area = rows[1];

        // Ensure the virtual-scroll cache matches the body width. Sharing
        // `vscroll` with prompt mode is fine because both invalidate on
        // width change.
        let message_render_context =
            super::super::messages::build_message_render_context_with_options(
                &self.messages,
                self.selected_message,
                self.selected_message_expanded,
                super::super::messages::MessageRenderOptions {
                    verbose: self.verbose,
                    is_transcript_mode: true,
                    show_all_in_transcript: true,
                },
            );
        self.vscroll.ensure_up_to_date(
            &self.messages,
            body_area.width,
            &self.theme,
            &message_render_context,
        );
        let total = self.vscroll.total_visual_lines();
        let max_scroll = total.saturating_sub(body_area.height as usize);
        if self.transcript_state.scroll_offset > max_scroll {
            self.transcript_state.scroll_offset = max_scroll;
        }

        render_messages(
            &self.messages,
            body_area,
            frame.buffer_mut(),
            &self.theme,
            self.is_streaming,
            self.transcript_state.scroll_offset,
            &self.vscroll,
            &message_render_context,
        );

        if header_height > 0 {
            self.render_transcript_header(rows[0], frame.buffer_mut());
        }
        if footer_height > 0 {
            self.render_transcript_footer(rows[2], frame.buffer_mut());
        }
    }

    fn render_focus_view(&mut self, frame: &mut Frame, size: Rect) {
        let focus = transcript::build_focus_view(&self.messages);
        let mut lines = Vec::new();

        if let Some(prompt) = focus.prompt {
            lines.push(Line::from(vec![Span::styled(
                "Prompt",
                self.theme.assistant_name,
            )]));
            for body_line in prompt.body.lines() {
                lines.push(Line::from(body_line.to_string()));
            }
        }

        if let Some(summary) = focus.tool_summary {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::from(vec![Span::styled(
                "Tool Summary",
                self.theme.tool_name,
            )]));
            for body_line in summary.lines() {
                lines.push(Line::from(body_line.to_string()));
            }
        }

        if let Some(response) = focus.response {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::from(vec![Span::styled(
                "Response",
                self.theme.user_name,
            )]));
            for body_line in response.body.lines() {
                lines.push(Line::from(body_line.to_string()));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "No focused transcript content yet.",
                self.theme.dim,
            )]));
        }

        for (row, line) in lines.iter().enumerate().take(size.height as usize) {
            frame
                .buffer_mut()
                .set_line(size.x, size.y + row as u16, line, size.width);
        }
    }

    fn render_transcript_header(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let total = self.messages.len();
        let mut parts = vec![format!(
            "\u{2500}\u{2500} {} \u{00b7} {} messages",
            self.view_mode.label(),
            total
        )];
        if matches!(
            self.transcript_state.input_mode,
            TranscriptInputMode::Search
        ) {
            parts.push(format!("search: \"{}\"", self.transcript_state.query));
        } else if !self.transcript_state.query.is_empty() {
            let total = self.transcript_state.matches.len();
            let idx = self.transcript_state.focused.map(|i| i + 1).unwrap_or(0);
            parts.push(format!(
                "search: \"{}\" ({}/{})",
                self.transcript_state.query, idx, total
            ));
        }
        let text = parts.join(" \u{00b7} ");
        let line = Line::from(vec![Span::styled(text, self.theme.dim)]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    fn render_transcript_footer(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let text = match self.transcript_state.input_mode {
            TranscriptInputMode::Search => {
                " [Enter] commit \u{00b7} [Esc] cancel \u{00b7} type to extend query".to_string()
            }
            TranscriptInputMode::Normal => concat!(
                " [Esc/q] prompt \u{00b7} [Ctrl+O] cycle \u{00b7} [/] search ",
                "\u{00b7} [n]/[N] next/prev \u{00b7} [e] editor ",
                "\u{00b7} [g]/[G] top/bottom"
            )
            .to_string(),
        };
        let line = Line::from(vec![Span::styled(text, self.theme.dim)]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

fn notification_style(tone: NotificationTone, theme: &Theme) -> Style {
    match tone {
        NotificationTone::Info => theme.info,
        NotificationTone::Warning => theme.warning,
        NotificationTone::Error => theme.error,
        NotificationTone::Dim => theme.dim,
    }
}
fn render_workspace_trust_prompt(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    cwd: &str,
    selected: usize,
) {
    let line_width = area.width.clamp(40, 120) as usize;
    let separator = "\u{2500}".repeat(line_width);
    let yes_marker = if selected == 0 { "\u{276f}" } else { " " };
    let no_marker = if selected == 1 { "\u{276f}" } else { " " };

    let lines = vec![
        Line::from(Span::styled(
            separator,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Accessing workspace:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {}", cwd),
            Style::default().fg(Color::LightBlue),
        )),
        Line::from(""),
        Line::from(
            " Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source",
        ),
        Line::from(
            " project, or work from your team). If not, take a moment to review what's in this folder first.",
        ),
        Line::from(""),
        Line::from(" cc-rust can read, edit, and execute files here."),
        Line::from(""),
        Line::from(Span::styled(
            " Security guide",
            Style::default().fg(Color::LightBlue),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" {} ", yes_marker),
                Style::default().fg(if selected == 0 {
                    Color::Green
                } else {
                    Color::White
                }),
            ),
            Span::raw("1. Yes, I trust this folder"),
        ]),
        Line::from(vec![
            Span::styled(
                format!(" {} ", no_marker),
                Style::default().fg(if selected == 1 {
                    Color::Red
                } else {
                    Color::White
                }),
            ),
            Span::raw("2. No, exit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Enter to confirm \u{00b7} Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, buf);
}

fn render_command_surface_overlay(
    surface: &CommandSurface,
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
    colors: &ThemeColors,
) {
    let text = surface.render();
    let body = text
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect::<Vec<_>>();
    render_centered_dialog_lines(
        CenteredOverlayFrame::new(surface.title())
            .color("accent")
            .width(32, 96)
            .height(5, 28),
        body,
        area,
        buf,
        colors,
        theme.dim,
    );
}

fn render_history_search_overlay(
    dialog: &HistorySearchDialog,
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
    colors: &ThemeColors,
) {
    if area.width < 20 || area.height < 8 {
        return;
    }

    let width = area.width.saturating_sub(4).clamp(20, 120);
    let height = area.height.saturating_sub(4).clamp(8, 18);
    let text = dialog.render(
        width.saturating_sub(4) as usize,
        height.saturating_sub(3) as usize,
    );
    let body = text
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect::<Vec<_>>();
    render_centered_dialog_lines(
        CenteredOverlayFrame::new("History Search")
            .color("accent")
            .width(20, 120)
            .height(8, 18),
        body,
        area,
        buf,
        colors,
        theme.dim,
    );
}

fn render_agent_tree_overlay(
    dialog: &mut super::agent_tree_dialog::AgentTreeDialog,
    state: &super::agent_navigation::AgentNavigationState,
    current_thread_id: &str,
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
    colors: &ThemeColors,
) {
    if area.width < 24 || area.height < 8 {
        return;
    }

    let mut lines = dialog.render_lines(state, current_thread_id, theme);
    let total = state.thread_count();
    let menu =
        AgentsMenuState::default_with_counts(total, 1.min(total), 0, total.saturating_sub(1))
            .render();
    if let Some(summary_row) = menu.lines().nth(1) {
        lines.insert(
            1,
            Line::from(Span::styled(format!("Menu: {summary_row}"), theme.dim)),
        );
    }
    render_centered_dialog_lines(
        CenteredOverlayFrame::new("Agent Threads")
            .color("accent")
            .width(24, 100)
            .height(8, 20),
        lines,
        area,
        buf,
        colors,
        theme.dim,
    );
}

// ---------------------------------------------------------------------------
// Completion popup rendering helpers
// ---------------------------------------------------------------------------

/// Calculate visible window start for a scrollable list.
fn visible_window_start(total: usize, selected: usize, max_rows: usize) -> usize {
    if max_rows == 0 || total <= max_rows {
        return 0;
    }

    selected.saturating_add(1).saturating_sub(max_rows)
}

/// Truncate a string to a max character width, adding "..." if truncated.
fn truncate(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        format!("{}...", chars[..max_width - 3].iter().collect::<String>())
    }
}

impl App {
    /// Preferred height for the completion popup (0 if not active).
    pub(super) fn completion_popup_height(&self) -> u16 {
        if !self.completion_state.active || self.command_palette.active() {
            return 0;
        }
        let count = self.completion_state.items.len().min(8);
        if count == 0 {
            return 0;
        }
        // Header + separator line + item rows + footer hint
        (count as u16).min(8) + 2
    }

    /// Render the active completion popup.
    fn render_completion_popup(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height < 3 || area.width < 20 {
            return;
        }

        let state = &self.completion_state;
        if state.items.is_empty() {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Completions ")
            .border_style(self.theme.dim);
        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line<'_>> = Vec::new();

        // Header line
        lines.push(Line::from(vec![Span::styled(
            format!(
                " {} items | Tab:accept | Shift+Tab:prev | Enter:fill ",
                state.items.len()
            ),
            self.theme.dim,
        )]));

        // Determine visible window
        let max_visible = (inner.height as usize).saturating_sub(2).max(1);
        let visible_start = visible_window_start(state.items.len(), state.selected, max_visible);

        for (idx, item) in state
            .items
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(max_visible)
        {
            let selected = idx == state.selected;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let kind_label = item.kind.label();
            let source = item.source_group.unwrap_or(kind_label);

            let detail = if selected {
                item.detail.as_deref().unwrap_or("")
            } else {
                ""
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" \u{276f} "),
                    if selected {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(format!("{:<30}", truncate(&item.label, 28)), style),
                Span::styled(format!(" {} ", source), self.theme.dim),
                if !detail.is_empty() {
                    Span::styled(detail, self.theme.dim)
                } else {
                    Span::raw("")
                },
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }
}
