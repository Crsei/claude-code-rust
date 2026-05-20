//! Scrollable pager overlays for transcript and static text views.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::text::Line;

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Static(StaticOverlay),
    Transcript(TranscriptOverlay),
}

impl Overlay {
    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn title(&self) -> &str {
        match self {
            Self::Static(overlay) => &overlay.title,
            Self::Transcript(overlay) => &overlay.title,
        }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn handle_key(&mut self, key: KeyEvent, viewport_height: usize) -> bool {
        match self {
            Self::Static(overlay) => overlay.handle_key(key, viewport_height),
            Self::Transcript(overlay) => overlay.inner.handle_key(key, viewport_height),
        }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn visible_lines(&self, viewport_height: usize) -> Vec<Line<'static>> {
        match self {
            Self::Static(overlay) => overlay.visible_lines(viewport_height),
            Self::Transcript(overlay) => overlay.inner.visible_lines(viewport_height),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticOverlay {
    pub title: String,
    lines: Vec<Line<'static>>,
    scroll: usize,
}

impl StaticOverlay {
    pub fn new(title: impl Into<String>, lines: Vec<Line<'static>>) -> Self {
        Self {
            title: title.into(),
            lines,
            scroll: 0,
        }
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn handle_key(&mut self, key: KeyEvent, viewport_height: usize) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(1, viewport_height),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(viewport_height.max(1)),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_down(viewport_height.max(1), viewport_height)
            }
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll_to_end(viewport_height),
            KeyCode::Esc | KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }

    pub fn visible_lines(&self, viewport_height: usize) -> Vec<Line<'static>> {
        render_offset_content(&self.lines, self.scroll, viewport_height)
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    fn scroll_down(&mut self, amount: usize, viewport_height: usize) {
        let max_scroll = self.lines.len().saturating_sub(viewport_height);
        self.scroll = self.scroll.saturating_add(amount).min(max_scroll);
    }

    fn scroll_to_end(&mut self, viewport_height: usize) {
        self.scroll = self.lines.len().saturating_sub(viewport_height);
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptOverlay {
    pub title: String,
    inner: StaticOverlay,
}

impl TranscriptOverlay {
    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn new(lines: Vec<Line<'static>>) -> Self {
        Self {
            title: "Transcript".to_string(),
            inner: StaticOverlay::new("Transcript", lines),
        }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn scroll(&self) -> usize {
        self.inner.scroll()
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
pub fn render_offset_content(
    lines: &[Line<'static>],
    offset: usize,
    height: usize,
) -> Vec<Line<'static>> {
    lines.iter().skip(offset).take(height).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyModifiers};

    #[test]
    fn page_down_moves_scroll() {
        let mut overlay = StaticOverlay::new(
            "test",
            (0..10).map(|i| Line::from(format!("{i}"))).collect(),
        );
        overlay.handle_key(
            KeyEvent::new_with_kind(KeyCode::PageDown, KeyModifiers::NONE, KeyEventKind::Press),
            3,
        );
        assert_eq!(overlay.scroll(), 3);
    }
}
