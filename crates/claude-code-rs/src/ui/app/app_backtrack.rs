//! Backtrack/rewind state helpers.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppBacktrackState {
    cursor: Option<usize>,
}

impl AppBacktrackState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn begin_at_end(&mut self, len: usize) {
        self.cursor = len.checked_sub(1);
    }

    pub fn move_previous(&mut self) {
        self.cursor = Some(self.cursor.unwrap_or(0).saturating_sub(1));
    }

    pub fn move_next(&mut self, len: usize) {
        let max = len.saturating_sub(1);
        self.cursor = Some(self.cursor.unwrap_or(0).saturating_add(1).min(max));
    }

    pub fn clear(&mut self) {
        self.cursor = None;
    }
}

pub fn truncate_after_index<T: Clone>(items: &[T], keep_last_index: usize) -> Vec<T> {
    items
        .iter()
        .take(keep_last_index.saturating_add(1))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_after_selected_index() {
        assert_eq!(truncate_after_index(&[1, 2, 3], 1), vec![1, 2]);
    }
}
