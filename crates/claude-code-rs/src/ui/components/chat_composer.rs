//! Chat composer state used by the bottom pane.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerMode {
    Insert,
    #[allow(dead_code)] // Phase 1: upstream parity surface
    VimNormal,
    Slash,
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerEffect {
    Submitted(String),
    Queued(String),
    IgnoredEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatComposerState {
    pub input: String,
    pub mode: ComposerMode,
    pub busy: bool,
    queued: VecDeque<String>,
}

impl Default for ChatComposerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatComposerState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            mode: ComposerMode::Insert,
            busy: false,
            queued: VecDeque::new(),
        }
    }

    pub fn set_input(&mut self, input: impl Into<String>) {
        self.input = input.into();
        self.mode = if self.input.starts_with('/') {
            ComposerMode::Slash
        } else if self.busy {
            ComposerMode::Busy
        } else {
            ComposerMode::Insert
        };
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn paste_text(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if !self.input.is_empty() && !self.input.ends_with('\n') {
            self.input.push('\n');
        }
        self.input.push_str(normalized.trim_end());
    }

    pub fn submit_or_queue(&mut self) -> ComposerEffect {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return ComposerEffect::IgnoredEmpty;
        }
        self.input.clear();
        if self.busy {
            self.queued.push_back(text.clone());
            ComposerEffect::Queued(text)
        } else {
            ComposerEffect::Submitted(text)
        }
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
        if busy {
            self.mode = ComposerMode::Busy;
        } else if self.mode == ComposerMode::Busy {
            self.mode = ComposerMode::Insert;
        }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn pop_queued(&mut self) -> Option<String> {
        self.queued.pop_front()
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn status_hint(&self) -> String {
        if self.busy {
            format!("busy; {} queued", self.queued.len())
        } else if self.input.starts_with('/') {
            "slash command".to_string()
        } else {
            "ready".to_string()
        }
    }

    pub fn render_lines(&self, width: usize) -> Vec<String> {
        let input = self.input.replace('\n', "\\n");
        let prompt_line = if input.is_empty() {
            ">".to_string()
        } else {
            format!("> {input}")
        };
        vec![
            format!("composer mode={:?} {}", self.mode, self.status_hint()),
            fit_line(&prompt_line, width),
        ]
    }
}

fn fit_line(text: &str, width: usize) -> String {
    if width == 0 || text.chars().count() <= width {
        return text.to_string();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}
