//! Direct TUI dialog for AskUserQuestion tool prompts.

use cc_types::callbacks::AskUserRequestPayload;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use crate::ui::permissions::ask_user_question_permission_request::preview_question_view::render_preview_question_view;
use crate::ui::permissions::ask_user_question_permission_request::submit_questions_view::render_submit_questions_view;
use crate::ui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionDialog {
    pub id: String,
    pub request: AskUserRequestPayload,
    answer: String,
    cursor: usize,
    selected_choice: usize,
}

impl QuestionDialog {
    pub fn new(id: impl Into<String>, request: AskUserRequestPayload) -> Self {
        Self {
            id: id.into(),
            request,
            answer: String::new(),
            cursor: 0,
            selected_choice: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match (key.modifiers, key.code) {
            (_, KeyCode::Enter) => return Some(self.submit_answer()),
            (_, KeyCode::Esc) => return Some(String::new()),
            (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                self.selected_choice = self.selected_choice.saturating_sub(1);
            }
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                let choice_count = self.request.choices.len();
                if choice_count > 0 {
                    self.selected_choice = (self.selected_choice + 1).min(choice_count - 1);
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.answer.clear();
                self.cursor = 0;
            }
            (_, KeyCode::Backspace) => self.backspace(),
            (_, KeyCode::Delete) => self.delete(),
            (_, KeyCode::Left) => self.cursor = self.cursor.saturating_sub(1),
            (_, KeyCode::Right) => {
                self.cursor = (self.cursor + 1).min(self.answer.chars().count());
            }
            (_, KeyCode::Home) => self.cursor = 0,
            (_, KeyCode::End) => self.cursor = self.answer.chars().count(),
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(ch)) => {
                self.insert(ch);
            }
            _ => {}
        }
        None
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let dialog_width = (area.width * 68 / 100).max(48).min(area.width);
        let dialog_height = 13u16.min(area.height).max(8);
        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
        let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

        Widget::render(Clear, dialog_area, buf);

        let block = Block::default()
            .title(" Need Input ")
            .borders(Borders::ALL)
            .border_style(theme.info)
            .style(Style::default());
        let inner = block.inner(dialog_area);
        Widget::render(block, dialog_area, buf);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);

        let header = vec![
            Line::from(vec![
                Span::styled("Question: ", theme.dim),
                Span::styled(
                    truncate(&self.request.question, chunks[0].width as usize),
                    theme.info,
                ),
            ]),
            Line::from(vec![
                Span::styled("Answer: ", theme.dim),
                Span::styled(
                    truncate(&self.answer_preview(), chunks[0].width as usize),
                    theme.warning,
                ),
            ]),
        ];
        Widget::render(Paragraph::new(header), chunks[0], buf);

        let body = self.rendered_body_lines(chunks[1].width as usize);
        Widget::render(
            Paragraph::new(body).wrap(Wrap { trim: true }),
            chunks[1],
            buf,
        );

        let footer_width = chunks[2].width.saturating_sub(2) as usize;
        let hint = Line::from(Span::styled(
            truncate(
                "Type an answer. Enter submits. Esc sends an empty answer.",
                footer_width,
            ),
            theme.dim,
        ));
        buf.set_line(chunks[2].x + 1, chunks[2].y, &hint, footer_width as u16);
    }

    fn rendered_body_lines(&self, width: usize) -> Vec<Line<'static>> {
        let answer_preview = if self.answer.trim().is_empty() {
            self.selected_choice_text()
                .unwrap_or_else(|| "<empty answer>".to_string())
        } else {
            self.answer.clone()
        };
        let rendered = format!(
            "{}\n\n{}",
            render_preview_question_view(&self.request.question, &[answer_preview.as_str()]),
            render_submit_questions_view(usize::from(!self.answer.trim().is_empty()), 1),
        );
        let mut lines = self.choice_lines(width);
        lines.extend(
            rendered
                .lines()
                .filter(|line| !line.trim().is_empty())
                .take(6)
                .map(|line| Line::from(truncate(line, width))),
        );
        lines
    }

    fn answer_with_cursor(&self) -> String {
        let mut chars = self.answer.chars().collect::<Vec<_>>();
        let cursor = self.cursor.min(chars.len());
        chars.insert(cursor, '|');
        chars.into_iter().collect()
    }

    fn answer_preview(&self) -> String {
        if self.request.allow_free_text {
            self.answer_with_cursor()
        } else {
            self.selected_choice_text()
                .unwrap_or_else(|| "<select a choice>".to_string())
        }
    }

    fn selected_choice_text(&self) -> Option<String> {
        self.request
            .choices
            .get(
                self.selected_choice
                    .min(self.request.choices.len().saturating_sub(1)),
            )
            .cloned()
    }

    fn choice_lines(&self, width: usize) -> Vec<Line<'static>> {
        self.request
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                let marker = if idx == self.selected_choice {
                    ">"
                } else {
                    " "
                };
                Line::from(truncate(&format!("{marker} {choice}"), width))
            })
            .collect()
    }

    fn submit_answer(&self) -> String {
        if !self.answer.trim().is_empty() {
            self.answer.clone()
        } else {
            self.selected_choice_text().unwrap_or_default()
        }
    }

    fn insert(&mut self, ch: char) {
        let mut chars = self.answer.chars().collect::<Vec<_>>();
        let cursor = self.cursor.min(chars.len());
        chars.insert(cursor, ch);
        self.answer = chars.into_iter().collect();
        self.cursor = cursor + 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars = self.answer.chars().collect::<Vec<_>>();
        let idx = self.cursor.saturating_sub(1);
        if idx < chars.len() {
            chars.remove(idx);
            self.answer = chars.into_iter().collect();
            self.cursor = idx;
        }
    }

    fn delete(&mut self) {
        let mut chars = self.answer.chars().collect::<Vec<_>>();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.answer = chars.into_iter().collect();
        }
    }
}

fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        input.to_string()
    } else if max_chars <= 3 {
        input.chars().take(max_chars).collect()
    } else {
        input
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
            + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::QuestionDialog;
    use cc_types::callbacks::AskUserRequestPayload;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn question_dialog_collects_answer() {
        let mut dialog = QuestionDialog::new(
            "q-1",
            AskUserRequestPayload {
                question: "Continue?".to_string(),
                choices: vec![],
                allow_free_text: true,
            },
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some("yes".to_string())
        );
    }
}
