//! Typed chat history cells.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryCell {
    User(String),
    Assistant(String),
    System(String),
    Tool {
        name: String,
        summary: String,
    },
    Diff {
        path: String,
        added: usize,
        removed: usize,
    },
    Status(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRenderMode {
    Prompt,
    Transcript,
}

impl HistoryCell {
    pub fn role(&self) -> &'static str {
        match self {
            HistoryCell::User(_) => "user",
            HistoryCell::Assistant(_) => "assistant",
            HistoryCell::System(_) => "system",
            HistoryCell::Tool { .. } => "tool",
            HistoryCell::Diff { .. } => "diff",
            HistoryCell::Status(_) => "status",
        }
    }

    pub fn render(&self, mode: HistoryRenderMode) -> String {
        match (self, mode) {
            (HistoryCell::User(text), _) => format!("> {text}"),
            (HistoryCell::Assistant(text), HistoryRenderMode::Prompt) => text.clone(),
            (HistoryCell::Assistant(text), HistoryRenderMode::Transcript) => {
                format!("assistant: {text}")
            }
            (HistoryCell::System(text), HistoryRenderMode::Prompt) => format!("sys: {text}"),
            (HistoryCell::System(text), HistoryRenderMode::Transcript) => {
                format!("system: {text}")
            }
            (HistoryCell::Tool { name, summary }, HistoryRenderMode::Prompt) => {
                format!("tool {name}: {summary}")
            }
            (HistoryCell::Tool { name, summary }, HistoryRenderMode::Transcript) => {
                format!("tool-call {name}\n  {summary}")
            }
            (
                HistoryCell::Diff {
                    path,
                    added,
                    removed,
                },
                _,
            ) => {
                format!("diff {path} +{added} -{removed}")
            }
            (HistoryCell::Status(text), _) => format!("status: {text}"),
        }
    }
}

pub fn render_history(cells: &[HistoryCell], mode: HistoryRenderMode) -> String {
    cells
        .iter()
        .map(|cell| cell.render(mode))
        .collect::<Vec<_>>()
        .join("\n")
}
