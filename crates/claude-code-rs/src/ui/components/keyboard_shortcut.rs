//! Shared render primitive for one-line keyboard shortcut hints.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutHint<'a> {
    pub key: &'a str,
    pub action: &'a str,
}

impl<'a> ShortcutHint<'a> {
    pub const fn new(key: &'a str, action: &'a str) -> Self {
        Self { key, action }
    }

    fn render(self) -> String {
        format!("{} {}", self.key, self.action)
    }
}

pub fn render_shortcut_hints(hints: &[ShortcutHint<'_>]) -> String {
    hints
        .iter()
        .copied()
        .map(ShortcutHint::render)
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::{render_shortcut_hints, ShortcutHint};

    #[test]
    fn render_shortcut_hints_matches_existing_help_pattern() {
        assert_eq!(
            render_shortcut_hints(&[
                ShortcutHint::new("Enter", "select"),
                ShortcutHint::new("Esc", "close"),
            ]),
            "Enter select | Esc close"
        );
    }
}
