//! Reusable selection, command, and picker surface.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub enabled: bool,
}

impl SelectionItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSurface {
    pub title: String,
    pub items: Vec<SelectionItem>,
    pub selected: usize,
    pub filter: String,
}

impl SelectionSurface {
    pub fn new(title: impl Into<String>, items: Vec<SelectionItem>) -> Self {
        Self {
            title: title.into(),
            items,
            selected: 0,
            filter: String::new(),
        }
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.selected = 0;
    }

    pub fn move_next(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1).min(visible.len() - 1);
        }
    }

    pub fn selected_item(&self) -> Option<&SelectionItem> {
        let visible = self.visible_indices();
        visible
            .get(self.selected)
            .and_then(|idx| self.items.get(*idx))
    }

    pub fn render_lines(&self, height: usize) -> Vec<String> {
        let mut lines = vec![format!("{} filter='{}'", self.title, self.filter)];
        for (visible_idx, item_idx) in self.visible_indices().into_iter().take(height).enumerate() {
            let item = &self.items[item_idx];
            let marker = if visible_idx == self.selected {
                ">"
            } else {
                " "
            };
            let state = if item.enabled { "" } else { " disabled" };
            lines.push(format!(
                "{marker} {} - {}{state}",
                item.label, item.description
            ));
        }
        lines
    }

    fn visible_indices(&self) -> Vec<usize> {
        let needle = self.filter.to_ascii_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                if needle.is_empty()
                    || item.label.to_ascii_lowercase().contains(&needle)
                    || item.id.to_ascii_lowercase().contains(&needle)
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }
}
