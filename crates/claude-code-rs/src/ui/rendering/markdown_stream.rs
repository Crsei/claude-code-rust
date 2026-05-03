//! Streaming Markdown collector.

use ratatui::text::Text;

use super::markdown_render::render_markdown_text;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkdownStreamCollector {
    buffer: String,
    committed_len: usize,
}

impl MarkdownStreamCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_str(&mut self, delta: &str) {
        self.buffer.push_str(delta);
    }

    pub fn replace(&mut self, text: impl Into<String>) {
        self.buffer = text.into();
        self.committed_len = self.committed_len.min(self.buffer.len());
    }

    pub fn raw(&self) -> &str {
        &self.buffer
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn render(&self) -> Text<'static> {
        render_markdown_text(&self.buffer)
    }

    pub fn commit(&mut self) -> String {
        let delta = self.buffer[self.committed_len..].to_string();
        self.committed_len = self.buffer.len();
        delta
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.committed_len = 0;
    }
}

pub(crate) fn simulate_stream_markdown_for_tests(chunks: &[&str]) -> Vec<Text<'static>> {
    let mut collector = MarkdownStreamCollector::new();
    chunks
        .iter()
        .map(|chunk| {
            collector.push_str(chunk);
            collector.render()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_returns_only_new_delta() {
        let mut collector = MarkdownStreamCollector::new();
        collector.push_str("a");
        assert_eq!(collector.commit(), "a");
        collector.push_str("b");
        assert_eq!(collector.commit(), "b");
    }
}
