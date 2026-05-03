use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::ui::selection_surface::SelectionSurface;
use crate::ui::theme::Theme;

use super::edit_targets::{has_edit_target_picker, EditTarget};
use super::filter::command_from_argument_input;
use super::{
    CommandItem, CommandPalette, DETAIL_ROWS, MAX_EDIT_ROWS, MAX_EDIT_TARGET_ROWS, MAX_ROWS,
};

impl CommandPalette {
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || area.width == 0 || !self.active {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(if self.edit_target_picker.is_some() {
                " Commands - Edit targets "
            } else {
                " Commands "
            })
            .border_style(theme.dim);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.filtered.is_empty() {
            let line = Line::from(Span::styled("No matching commands", theme.dim));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        let selected_item = self.filtered.get(self.selected);
        let detail_rows = selected_item
            .map(|item| palette_detail_rows(item, inner.width as usize))
            .unwrap_or(DETAIL_ROWS);
        let picker_rows = self
            .edit_target_picker
            .as_ref()
            .map(|picker| picker.render_lines(MAX_EDIT_TARGET_ROWS).len())
            .unwrap_or(0);
        let detail_and_picker_rows =
            detail_rows as usize + picker_rows + usize::from(picker_rows > 0);
        let visible_rows = (inner.height as usize)
            .saturating_sub(detail_and_picker_rows)
            .min(MAX_ROWS);
        let mut lines = Vec::new();
        let visible_start = visible_window_start(self.filtered.len(), self.selected, visible_rows);
        for (idx, item) in self
            .filtered
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(visible_rows)
        {
            let selected = idx == self.selected;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.unselected
            };
            let aliases = if item.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.aliases.join(", "))
            };
            lines.push(Line::from(vec![
                Span::styled(format!("/{:<18}", item.name), style),
                Span::styled(aliases, theme.dim),
                Span::styled("  ", theme.dim),
                Span::styled(item.description.clone(), style),
            ]));
        }

        if let Some(selected) = selected_item {
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("Usage: ", theme.dim),
                Span::styled(selected.usage.clone(), theme.info),
            ]));
            if let Some(example) = selected.examples.first() {
                lines.push(Line::from(vec![
                    Span::styled("Example: ", theme.dim),
                    Span::styled(example.clone(), theme.unselected),
                ]));
            }
            if !selected.edit_targets.is_empty() {
                for (idx, text) in
                    edit_target_lines(&selected.edit_targets, inner.width as usize, MAX_EDIT_ROWS)
                        .into_iter()
                        .enumerate()
                {
                    let label = if idx == 0 { "Edit: " } else { "      " };
                    lines.push(Line::from(vec![
                        Span::styled(label, theme.dim),
                        Span::styled(text, theme.info),
                    ]));
                }
                if has_edit_target_picker(selected) {
                    lines.push(Line::from(vec![
                        Span::styled("Hint: ", theme.dim),
                        Span::styled("Press Ctrl+E to choose a target in-terminal.", theme.info),
                    ]));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "Enter selects the command; type arguments after the inserted space.",
                    theme.dim,
                )));
            }
        }

        if let Some(picker) = &self.edit_target_picker {
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("Target picker: ", theme.dim),
                Span::styled("Ctrl+E closes the picker", theme.dim),
            ]));
            lines.extend(render_picker_lines(picker, MAX_EDIT_TARGET_ROWS, theme));
        }

        Paragraph::new(lines).render(inner, buf);
    }

    pub fn render_argument_help(
        input: &str,
        cwd: &Path,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let Some(item) = command_from_argument_input(input, cwd) else {
            return;
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" /{} arguments ", item.name))
            .border_style(theme.dim);
        let inner = block.inner(area);
        block.render(area, buf);

        let usage = Line::from(vec![
            Span::styled("Usage: ", theme.dim),
            Span::styled(truncate(&item.usage, inner.width as usize), theme.info),
        ]);
        set_inner_line(buf, inner, 0, &usage);

        let example = item
            .examples
            .first()
            .map(|example| format!("Example: {example}"))
            .unwrap_or_else(|| "Type arguments after the command, then press Enter.".to_string());
        let example_line = Line::from(Span::styled(
            truncate(&example, inner.width as usize),
            theme.unselected,
        ));
        set_inner_line(buf, inner, 1, &example_line);

        let about_line = Line::from(vec![
            Span::styled("About: ", theme.dim),
            Span::styled(
                truncate(&item.description, inner.width as usize),
                theme.unselected,
            ),
        ]);
        set_inner_line(buf, inner, 2, &about_line);

        if !item.edit_targets.is_empty() {
            for (idx, text) in
                edit_target_lines(&item.edit_targets, inner.width as usize, MAX_EDIT_ROWS)
                    .into_iter()
                    .enumerate()
            {
                let row_idx = 3 + idx;
                if row_idx as u16 >= inner.height {
                    break;
                }
                let label = if idx == 0 { "Edit: " } else { "      " };
                let edit_line = Line::from(vec![
                    Span::styled(label, theme.dim),
                    Span::styled(text, theme.link),
                ]);
                set_inner_line(buf, inner, row_idx as u16, &edit_line);
            }
            if has_edit_target_picker(&item) {
                let hint = Line::from(vec![
                    Span::styled("Hint: ", theme.dim),
                    Span::styled("Press Ctrl+E to pick a target.", theme.info),
                ]);
                let row_idx = 3 + item.edit_targets.len().min(MAX_EDIT_ROWS);
                if (row_idx as u16) < inner.height {
                    set_inner_line(buf, inner, row_idx as u16, &hint);
                }
            }
        }
    }
}

pub(super) fn visible_window_start(total: usize, selected: usize, max_rows: usize) -> usize {
    if max_rows == 0 || total <= max_rows {
        return 0;
    }

    selected.saturating_add(1).saturating_sub(max_rows)
}

fn set_inner_line(buf: &mut Buffer, inner: Rect, row: u16, line: &Line<'_>) {
    if row < inner.height {
        buf.set_line(inner.x, inner.y + row, line, inner.width);
    }
}

pub(super) fn argument_edit_row_count(item: &CommandItem) -> usize {
    if item.edit_targets.is_empty() {
        0
    } else {
        item.edit_targets.len().min(MAX_EDIT_ROWS)
    }
}

pub(super) fn palette_detail_rows(item: &CommandItem, width: usize) -> u16 {
    let edit_rows = if item.edit_targets.is_empty() {
        1
    } else {
        edit_target_lines(&item.edit_targets, width, MAX_EDIT_ROWS)
            .len()
            .max(1)
    };
    3 + edit_rows as u16
}

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

fn edit_target_lines(targets: &[EditTarget], max_width: usize, max_lines: usize) -> Vec<String> {
    if targets.is_empty() || max_width == 0 || max_lines == 0 {
        return Vec::new();
    }

    let fragments = targets
        .iter()
        .map(|target| format!("{}: {}", target.label, target.display))
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut current = String::new();

    for fragment in fragments {
        let prefix_width = if lines.is_empty() {
            "Edit: ".len()
        } else {
            "      ".len()
        };
        let available = max_width.saturating_sub(prefix_width).max(1);
        let separator = if current.is_empty() { "" } else { " | " };
        let candidate_width = current.len() + separator.len() + fragment.len();

        if candidate_width <= available {
            if !current.is_empty() {
                current.push_str(separator);
            }
            current.push_str(&fragment);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            if lines.len() == max_lines {
                return lines;
            }
        }

        let available = max_width.saturating_sub("      ".len()).max(1);
        current = truncate(&fragment, available);
    }

    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }

    lines
}

fn render_picker_lines(
    picker: &SelectionSurface,
    max_rows: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    picker
        .render_lines(max_rows)
        .into_iter()
        .enumerate()
        .map(|(idx, text)| {
            let style = if idx == 0 {
                theme.dim
            } else if text.starts_with('>') {
                theme.selected
            } else {
                theme.unselected
            };
            Line::from(Span::styled(text, style))
        })
        .collect()
}
