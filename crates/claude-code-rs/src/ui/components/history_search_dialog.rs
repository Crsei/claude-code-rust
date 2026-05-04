use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use unicode_width::UnicodeWidthChar;

use crate::ui::fuzzy_match::fuzzy_match;
use crate::ui::search_box::SearchBox;

const AGE_WIDTH: usize = 8;
const PREVIEW_ROWS: usize = 6;
const RIGHT_PREVIEW_MIN_WIDTH: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySearchEntry {
    pub display: String,
    pub timestamp_secs: i64,
}

impl HistorySearchEntry {
    pub fn new(display: impl Into<String>, timestamp_secs: i64) -> Self {
        Self {
            display: display.into(),
            timestamp_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistorySearchDialogEvent {
    None,
    Cancelled,
    Selected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySearchDialog {
    items: Option<Vec<HistorySearchItem>>,
    query: String,
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistorySearchItem {
    entry: HistorySearchEntry,
    lower: String,
    first_line: String,
    age: String,
}

impl HistorySearchDialog {
    pub fn loading(initial_query: impl Into<String>, _now_secs: i64) -> Self {
        Self {
            items: None,
            query: initial_query.into(),
            selected: 0,
        }
    }

    pub fn from_entries(
        entries: impl IntoIterator<Item = HistorySearchEntry>,
        initial_query: impl Into<String>,
        now_secs: i64,
    ) -> Self {
        let mut dialog = Self {
            items: Some(
                entries
                    .into_iter()
                    .map(|entry| HistorySearchItem::new(entry, now_secs))
                    .collect(),
            ),
            query: initial_query.into(),
            selected: 0,
        };
        dialog.clamp_selection();
        dialog
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_prompt(&self) -> Option<&str> {
        let idx = self.selected_item_index()?;
        self.items
            .as_ref()
            .and_then(|items| items.get(idx))
            .map(|item| item.entry.display.as_str())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> HistorySearchDialogEvent {
        if key.kind != KeyEventKind::Press {
            return HistorySearchDialogEvent::None;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
                self.move_next();
                HistorySearchDialogEvent::None
            }
            (_, KeyCode::Up) => {
                self.move_prev();
                HistorySearchDialogEvent::None
            }
            (_, KeyCode::Down) | (_, KeyCode::Tab) => {
                self.move_next();
                HistorySearchDialogEvent::None
            }
            (_, KeyCode::Enter) => self
                .selected_prompt()
                .map(|prompt| HistorySearchDialogEvent::Selected(prompt.to_string()))
                .unwrap_or(HistorySearchDialogEvent::None),
            (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                HistorySearchDialogEvent::Cancelled
            }
            (_, KeyCode::Backspace) => {
                self.query.pop();
                self.selected = 0;
                HistorySearchDialogEvent::None
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.query.clear();
                self.selected = 0;
                HistorySearchDialogEvent::None
            }
            (modifiers, KeyCode::Char(ch))
                if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT =>
            {
                self.query.push(ch);
                self.selected = 0;
                HistorySearchDialogEvent::None
            }
            _ => HistorySearchDialogEvent::None,
        }
    }

    pub fn render(&self, width: usize, height: usize) -> String {
        let width = width.max(20);
        let height = height.max(4);
        let preview_on_right = width >= RIGHT_PREVIEW_MIN_WIDTH;

        let search = SearchBox::new(&self.query)
            .placeholder("Filter history...")
            .prefix("filter")
            .borderless(true)
            .terminal_focused(true)
            .width(width.saturating_sub(2))
            .render();
        let mut lines = vec![format!("Search prompts  {search}")];

        let visible = self.visible_indices();
        let empty_message = self.empty_message();
        if let Some(message) = empty_message {
            lines.push(message.to_string());
            lines.push("Enter use | Esc close | Ctrl+R next".to_string());
            return lines
                .into_iter()
                .take(height)
                .collect::<Vec<_>>()
                .join("\n");
        }

        if preview_on_right {
            lines.extend(self.render_wide_rows(width, height, &visible));
        } else {
            lines.extend(self.render_narrow_rows(width, height, &visible));
        }
        lines.push("Enter use | Esc close | Ctrl+R next".to_string());
        lines
            .into_iter()
            .take(height)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_wide_rows(&self, width: usize, height: usize, visible: &[usize]) -> Vec<String> {
        let list_width = ((width.saturating_sub(6)) / 2).max(32);
        let preview_width = width.saturating_sub(list_width + 3).max(20);
        let list_rows = self.list_rows(visible, list_width, height.saturating_sub(2));
        let preview = self.preview_rows(preview_width);
        let row_count = list_rows
            .len()
            .max(preview.len())
            .min(height.saturating_sub(2));
        (0..row_count)
            .map(|idx| {
                let left = list_rows.get(idx).map(String::as_str).unwrap_or("");
                let right = preview.get(idx).map(String::as_str).unwrap_or("");
                let left = pad_to_width(left, list_width);
                if right.is_empty() {
                    format!("{left} |")
                } else {
                    format!("{left} | {}", truncate_to_width(right, preview_width))
                }
            })
            .collect()
    }

    fn render_narrow_rows(&self, width: usize, height: usize, visible: &[usize]) -> Vec<String> {
        let preview_budget = PREVIEW_ROWS + 1;
        let list_budget = height.saturating_sub(preview_budget + 3).max(1);
        let mut lines = self.list_rows(visible, width, list_budget);
        lines.push("Preview".to_string());
        lines.extend(self.preview_rows(width.saturating_sub(2).max(20)));
        lines
    }

    fn list_rows(&self, visible: &[usize], width: usize, limit: usize) -> Vec<String> {
        let Some(items) = &self.items else {
            return Vec::new();
        };
        let row_width = width.saturating_sub(AGE_WIDTH + 3).max(10);
        visible
            .iter()
            .take(limit)
            .enumerate()
            .filter_map(|(visible_idx, item_idx)| {
                let item = items.get(*item_idx)?;
                let marker = if visible_idx == self.selected {
                    ">"
                } else {
                    " "
                };
                Some(format!(
                    "{marker} {} {}",
                    item.age,
                    truncate_to_width(&item.first_line, row_width)
                ))
            })
            .collect()
    }

    fn preview_rows(&self, width: usize) -> Vec<String> {
        let Some(prompt) = self.selected_prompt() else {
            return vec!["No prompt selected".to_string()];
        };

        let mut wrapped = Vec::new();
        for line in prompt.lines().filter(|line| !line.trim().is_empty()) {
            for row in textwrap::wrap(line, width.max(1)) {
                wrapped.push(row.into_owned());
            }
        }
        if wrapped.is_empty() {
            wrapped.push("(empty prompt)".to_string());
        }

        let overflow = wrapped.len() > PREVIEW_ROWS;
        let shown_count = if overflow {
            PREVIEW_ROWS.saturating_sub(1)
        } else {
            PREVIEW_ROWS
        };
        let mut lines = wrapped
            .iter()
            .take(shown_count)
            .map(|row| truncate_to_width(row, width))
            .collect::<Vec<_>>();
        if overflow {
            lines.push(format!(
                "+{} more lines",
                wrapped.len().saturating_sub(shown_count)
            ));
        }
        lines
    }

    fn empty_message(&self) -> Option<&'static str> {
        match &self.items {
            None => Some("Loading..."),
            Some(items) if items.is_empty() => Some("No history yet"),
            Some(_) if self.visible_indices().is_empty() => Some("No matching prompts"),
            Some(_) => None,
        }
    }

    fn move_next(&mut self) {
        let len = self.visible_indices().len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1) % len;
        }
    }

    fn move_prev(&mut self) {
        let len = self.visible_indices().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected == 0 {
            self.selected = len - 1;
        } else {
            self.selected -= 1;
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_indices().len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(len - 1);
        }
    }

    fn selected_item_index(&self) -> Option<usize> {
        self.visible_indices().get(self.selected).copied()
    }

    fn visible_indices(&self) -> Vec<usize> {
        let Some(items) = &self.items else {
            return Vec::new();
        };
        let query = self.query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return (0..items.len()).collect();
        }

        let mut exact = Vec::new();
        let mut fuzzy = Vec::new();
        for (idx, item) in items.iter().enumerate() {
            if item.lower.contains(&query) {
                exact.push(idx);
            } else if fuzzy_match(&item.lower, &query).is_some() {
                fuzzy.push(idx);
            }
        }
        exact.extend(fuzzy);
        exact
    }
}

impl HistorySearchItem {
    fn new(entry: HistorySearchEntry, now_secs: i64) -> Self {
        let first_line = entry
            .display
            .lines()
            .next()
            .unwrap_or(entry.display.as_str())
            .to_string();
        let lower = entry.display.to_ascii_lowercase();
        let age = pad_to_width(&format_age(entry.timestamp_secs, now_secs), AGE_WIDTH);
        Self {
            entry,
            lower,
            first_line,
            age,
        }
    }
}

fn format_age(timestamp_secs: i64, now_secs: i64) -> String {
    let delta = now_secs.saturating_sub(timestamp_secs).max(0);
    if delta < 60 {
        "now".to_string()
    } else if delta < 3_600 {
        format!("{}m", delta / 60)
    } else if delta < 86_400 {
        format!("{}h", delta / 3_600)
    } else {
        format!("{}d", delta / 86_400)
    }
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut width = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }

    if out.len() < text.len() && max_width > 3 {
        while display_width(&out) + 3 > max_width {
            out.pop();
        }
        out.push_str("...");
    }
    out
}

fn pad_to_width(text: &str, width: usize) -> String {
    let current = display_width(text);
    if current >= width {
        truncate_to_width(text, width)
    } else {
        format!("{}{}", text, " ".repeat(width - current))
    }
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};
    use insta::assert_snapshot;

    const NOW: i64 = 1_700_000_000;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl_key(ch: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn entries() -> Vec<HistorySearchEntry> {
        vec![
            HistorySearchEntry::new("git status --short", NOW - 45),
            HistorySearchEntry::new("cargo test -p claude-code-rs history_search", NOW - 240),
            HistorySearchEntry::new("open model context protocol docs", NOW - 7_200),
            HistorySearchEntry::new(
                "write a multi-line prompt\nwith enough detail to preview\nand extra rows\nthat should overflow\nline five\nline six\nline seven",
                NOW - 172_800,
            ),
        ]
    }

    #[test]
    fn filtering_keeps_contains_before_subsequence() {
        let mut dialog = HistorySearchDialog::from_entries(entries(), "mcp", NOW);

        assert_eq!(
            dialog.selected_prompt(),
            Some("open model context protocol docs")
        );

        dialog.handle_key(ctrl_key('u'));
        dialog.handle_key(key(KeyCode::Char('z')));
        dialog.handle_key(key(KeyCode::Char('z')));
        dialog.handle_key(key(KeyCode::Char('z')));
        assert_eq!(dialog.query(), "zzz");
        assert_eq!(dialog.selected_prompt(), None);
    }

    #[test]
    fn key_handling_edits_and_selects() {
        let mut dialog = HistorySearchDialog::from_entries(entries(), "", NOW);

        dialog.handle_key(key(KeyCode::Char('c')));
        dialog.handle_key(key(KeyCode::Char('a')));
        assert!(dialog
            .selected_prompt()
            .is_some_and(|prompt| prompt.starts_with("cargo test")));

        dialog.handle_key(key(KeyCode::Backspace));
        assert_eq!(dialog.query(), "c");

        dialog.handle_key(ctrl_key('u'));
        assert_eq!(dialog.query(), "");

        assert_eq!(
            dialog.handle_key(key(KeyCode::Enter)),
            HistorySearchDialogEvent::Selected("git status --short".to_string())
        );
        assert_eq!(
            dialog.handle_key(key(KeyCode::Esc)),
            HistorySearchDialogEvent::Cancelled
        );
    }

    #[test]
    fn snapshot_history_search_states() {
        let wide = HistorySearchDialog::from_entries(entries(), "", NOW).render(120, 12);
        let narrow = HistorySearchDialog::from_entries(entries(), "write", NOW).render(64, 14);
        let loading = HistorySearchDialog::loading("", NOW).render(64, 6);
        let no_history = HistorySearchDialog::from_entries(Vec::new(), "", NOW).render(64, 6);
        let no_matches = HistorySearchDialog::from_entries(entries(), "zzz", NOW).render(64, 6);

        assert_snapshot!(format!(
            "## wide\n{wide}\n\n## narrow\n{narrow}\n\n## loading\n{loading}\n\n## no history\n{no_history}\n\n## no matches\n{no_matches}"
        ));
    }
}
